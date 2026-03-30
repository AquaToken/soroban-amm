use soroban_sdk::{Env, U256};

// 512-bit unsigned integer for intermediate products in mul-div operations.
// Stored as two U256 halves: value = hi * 2^256 + lo.
struct U512 {
    hi: U256,
    lo: U256,
}

// Extract the lower 128 bits of a U256: a mod 2^128.
fn lo128(_e: &Env, a: &U256) -> U256 {
    // a - (a >> 128) << 128
    let hi_part = a.shr(128).shl(128);
    a.sub(&hi_part)
}

/// Wrapping U256 addition: returns (sum mod 2^256, carried).
/// Unlike U256::add(), this does not panic on overflow.
fn wrapping_add(e: &Env, a: &U256, b: &U256) -> (U256, bool) {
    let max = U256::from_be_bytes(e, &soroban_sdk::Bytes::from_array(e, &[0xFF; 32]));
    let remaining = max.sub(a); // max - a (always valid since a <= max)
    if remaining >= *b {
        (a.add(b), false)
    } else {
        // (a + b) mod 2^256 = b - remaining - 1
        (b.sub(&remaining).sub(&U256::from_u32(e, 1)), true)
    }
}

// Full 512-bit product of two U256 values using schoolbook multiplication.
//
// Split each operand into two 128-bit halves:
//   a = a_hi * 2^128 + a_lo,   b = b_hi * 2^128 + b_lo
//
// Product = a_hi*b_hi * 2^256 + (a_hi*b_lo + a_lo*b_hi) * 2^128 + a_lo*b_lo
//
// Each partial product fits in U256 since inputs are at most 128-bit.
fn u256_full_mul(e: &Env, a: &U256, b: &U256) -> U512 {
    let a_lo = lo128(e, a);
    let a_hi = a.shr(128);
    let b_lo = lo128(e, b);
    let b_hi = b.shr(128);

    let lo_lo = a_lo.mul(&b_lo); // max 256 bits
    let hi_lo = a_hi.mul(&b_lo); // max 256 bits
    let lo_hi = a_lo.mul(&b_hi); // max 256 bits
    let hi_hi = a_hi.mul(&b_hi); // max 256 bits

    // cross = hi_lo + lo_hi (may carry into bit 256)
    let (cross, cross_carried) = wrapping_add(e, &hi_lo, &lo_hi);
    let cross_carry = if cross_carried {
        U256::from_u32(e, 1)
    } else {
        U256::from_u32(e, 0)
    };

    // lo = lo_lo + lo128(cross) << 128
    let cross_lo_shifted = lo128(e, &cross).shl(128);
    let (lo, lo_carried) = wrapping_add(e, &lo_lo, &cross_lo_shifted);
    let lo_carry = if lo_carried {
        U256::from_u32(e, 1)
    } else {
        U256::from_u32(e, 0)
    };

    // hi = hi_hi + cross >> 128 + cross_carry * 2^128 + lo_carry
    let hi = hi_hi
        .add(&cross.shr(128))
        .add(&cross_carry.shl(128))
        .add(&lo_carry);

    U512 { hi, lo }
}

/// Compute `floor(a * b / d)` with 512-bit intermediate precision.
///
/// Uses U256 direct arithmetic when possible, falls back to schoolbook
/// multiplication + long division in 64-bit chunks.
///
/// Panics if `d == 0`.
pub fn mul_div_floor(e: &Env, a: &U256, b: &U256, d: &U256) -> U256 {
    mul_div_internal(e, a, b, d, false)
}

/// Compute `ceil(a * b / d)` with 512-bit intermediate precision.
///
/// Uses U256 direct arithmetic when possible, falls back to schoolbook
/// multiplication + long division in 64-bit chunks.
///
/// Panics if `d == 0`.
pub fn mul_div_ceil(e: &Env, a: &U256, b: &U256, d: &U256) -> U256 {
    mul_div_internal(e, a, b, d, true)
}

fn mul_div_internal(e: &Env, a: &U256, b: &U256, d: &U256, round_up: bool) -> U256 {
    let zero = U256::from_u32(e, 0);
    let one = U256::from_u32(e, 1);

    if *a == zero || *b == zero {
        return zero;
    }

    // Fast path: if both operands fit in 128 bits, product fits in 256 bits.
    if a.shr(128) == zero && b.shr(128) == zero {
        let product = a.mul(b);
        let quotient = product.div(d);
        let remainder = product.rem_euclid(d);
        return if round_up && remainder != zero {
            quotient.add(&one)
        } else {
            quotient
        };
    }

    // Slow path: full 512-bit product + long division.
    let U512 { hi, lo } = u256_full_mul(e, a, b);

    let (quotient, remainder) = if d.shr(248) == zero {
        // Byte-level long division (64 iterations).
        // Safe because remainder < d < 2^248, so remainder.shl(8) < 2^256.
        u512_div_bytes(e, &hi, &lo, d)
    } else {
        // Bit-level long division (256 iterations).
        // Required when d >= 2^248 (e.g. getNextSqrtPriceFromAmount0 denominator)
        // because remainder.shl(8) would overflow U256.
        u512_div_bits(e, &hi, &lo, d)
    };

    if round_up && remainder != zero {
        quotient.add(&one)
    } else {
        quotient
    }
}

// Long division of (hi * 2^256 + lo) / d, processing 8 bits at a time.
// Requires d < 2^248 so that remainder.shl(8) fits in U256.
fn u512_div_bytes(e: &Env, hi: &U256, lo: &U256, d: &U256) -> (U256, U256) {
    let zero = U256::from_u32(e, 0);

    let hi_bytes = hi.to_be_bytes();
    let lo_bytes = lo.to_be_bytes();

    let mut remainder = zero.clone();
    let mut quotient = zero.clone();

    // Process 64 bytes: 32 from hi (most significant), 32 from lo.
    for byte_idx in 0u32..64 {
        let byte_val = if byte_idx < 32 {
            hi_bytes.get(byte_idx).unwrap_or(0)
        } else {
            lo_bytes.get(byte_idx - 32).unwrap_or(0)
        };

        // remainder = remainder * 256 + byte
        remainder = remainder.shl(8).add(&U256::from_u32(e, byte_val as u32));

        // q_chunk = remainder / d
        let q_chunk = remainder.div(d);
        remainder = remainder.rem_euclid(d);

        // quotient = quotient * 256 + q_chunk
        quotient = quotient.shl(8).add(&q_chunk);
    }

    (quotient, remainder)
}

// Long division of (hi * 2^256 + lo) / d, processing 1 bit at a time.
// Uses wrapping_add for doubling to handle d >= 2^248 without overflow.
// Since the result must fit in U256, hi < d, so we start with remainder = hi
// and only process the 256 bits of lo.
fn u512_div_bits(e: &Env, hi: &U256, lo: &U256, d: &U256) -> (U256, U256) {
    let zero = U256::from_u32(e, 0);
    let one = U256::from_u32(e, 1);
    let max = U256::from_be_bytes(e, &soroban_sdk::Bytes::from_array(e, &[0xFF; 32]));

    // Since result fits in U256, hi < d. Start with remainder = hi,
    // skipping 256 iterations of zero quotient bits.
    let mut remainder = hi.clone();
    let mut quotient = zero.clone();

    let lo_bytes = lo.to_be_bytes();

    // Process 256 bits of lo from MSB to LSB.
    for i in 0u32..256 {
        // remainder = remainder * 2 + bit (using wrapping_add for the doubling)
        let (doubled, carry) = wrapping_add(e, &remainder, &remainder);

        // Extract bit from lo bytes: bit (255-i) = byte i/8, bit 7-(i%8)
        let byte_idx = i / 8;
        let bit_in_byte = 7 - (i % 8);
        let byte = lo_bytes.get(byte_idx).unwrap_or(0);
        let bit = (byte >> bit_in_byte) & 1;

        let rem_with_bit = if bit != 0 { doubled.add(&one) } else { doubled };

        if carry || rem_with_bit >= *d {
            // Quotient bit is 1. Subtract d from the actual value.
            if carry {
                // Actual value = rem_with_bit + 2^256.
                // adjusted = rem_with_bit + (2^256 - d) = rem_with_bit + (MAX - d + 1)
                // This is < d < 2^256, so add() is safe.
                let complement = max.sub(d).add(&one);
                remainder = rem_with_bit.add(&complement);
            } else {
                remainder = rem_with_bit.sub(d);
            }
            quotient = quotient.shl(1).add(&one);
        } else {
            remainder = rem_with_bit;
            quotient = quotient.shl(1);
        }
    }

    (quotient, remainder)
}

#[cfg(test)]
mod test {
    use super::{mul_div_ceil, mul_div_floor};
    use soroban_sdk::{Env, U256};

    #[test]
    fn u512_small_exact() {
        let e = Env::default();
        let a = U256::from_u128(&e, 1_000_000);
        let b = U256::from_u128(&e, 2_000_000);
        let d = U256::from_u128(&e, 500_000);
        assert_eq!(
            mul_div_floor(&e, &a, &b, &d),
            U256::from_u128(&e, 4_000_000)
        );
    }

    #[test]
    fn u512_identity() {
        let e = Env::default();
        let a = U256::from_u128(&e, u128::MAX);
        let b = U256::from_u128(&e, u128::MAX);
        let d = U256::from_u128(&e, u128::MAX);
        assert_eq!(
            mul_div_floor(&e, &a, &b, &d),
            U256::from_u128(&e, u128::MAX)
        );
    }

    #[test]
    fn u512_overflow_case() {
        let e = Env::default();
        // 2^200 * 2^200 / 2^200 = 2^200
        let a = U256::from_u32(&e, 1).shl(200);
        let b = U256::from_u32(&e, 1).shl(200);
        let d = U256::from_u32(&e, 1).shl(200);
        assert_eq!(mul_div_floor(&e, &a, &b, &d), a);
    }

    #[test]
    fn u512_rounding_down() {
        let e = Env::default();
        // 10 * 3 / 7 = 4.285... → floor = 4
        let a = U256::from_u128(&e, 10);
        let b = U256::from_u128(&e, 3);
        let d = U256::from_u128(&e, 7);
        assert_eq!(mul_div_floor(&e, &a, &b, &d), U256::from_u128(&e, 4));
    }

    #[test]
    fn u512_rounding_up() {
        let e = Env::default();
        // 10 * 3 / 7 = 4.285... → ceil = 5
        let a = U256::from_u128(&e, 10);
        let b = U256::from_u128(&e, 3);
        let d = U256::from_u128(&e, 7);
        assert_eq!(mul_div_ceil(&e, &a, &b, &d), U256::from_u128(&e, 5));
    }

    #[test]
    fn u512_large_realistic() {
        let e = Env::default();
        // liquidity (u128::MAX) * sqrt_price (2^160) / Q96 (2^96)
        // Intermediate ~2^288 (overflows U256), result = u128::MAX * 2^64
        let liq = U256::from_u128(&e, u128::MAX);
        let sqrt = U256::from_u32(&e, 1).shl(160);
        let q96 = U256::from_u32(&e, 1).shl(96);
        let result = mul_div_floor(&e, &liq, &sqrt, &q96);
        let expected = U256::from_u128(&e, u128::MAX).shl(64);
        assert_eq!(result, expected);
    }

    #[test]
    fn u512_no_overflow_fast_path() {
        let e = Env::default();
        let a = U256::from_u128(&e, 123456789);
        let b = U256::from_u128(&e, 987654321);
        let d = U256::from_u128(&e, 111111111);
        // 123456789 * 987654321 / 111111111 = 1097393681
        assert_eq!(
            mul_div_floor(&e, &a, &b, &d),
            U256::from_u128(&e, 1097393681)
        );
    }

    #[test]
    fn u512_lo_accumulation_overflow() {
        // Both operands = 2^129 - 1, with a_hi = b_hi = 1, a_lo = b_lo = 2^128 - 1.
        // lo_lo = (2^128-1)^2 ≈ 2^256, cross_lo_shifted ≈ 2^256 - 2^129.
        // Their sum exceeds 2^256, triggering the lo accumulation overflow
        // that crashed full-range concentrated pool deposits.
        let e = Env::default();
        let one = U256::from_u32(&e, 1);
        let a = one.shl(129).sub(&one); // 2^129 - 1
                                        // a * a / a = a
        let result = mul_div_floor(&e, &a, &a, &a);
        assert_eq!(result, a);
    }

    #[test]
    fn u512_full_range_deposit_realistic() {
        // Simulates amount0_delta: numerator1 * diff / sqrt_price
        // with max liquidity and wide tick range.
        // numerator1 = u128::MAX * Q96 ≈ 2^224, diff = sqrt_price ≈ 2^160.
        // Product ≈ 2^384, requires U512 path.
        let e = Env::default();
        let q96 = U256::from_u32(&e, 1).shl(96);
        let numerator1 = U256::from_u128(&e, u128::MAX).mul(&q96);
        let sqrt_price = U256::from_u32(&e, 1).shl(160);
        // numerator1 * sqrt_price / sqrt_price = numerator1
        let result = mul_div_floor(&e, &numerator1, &sqrt_price, &sqrt_price);
        assert_eq!(result, numerator1);
    }

    #[test]
    fn u512_divisor_one() {
        let e = Env::default();
        let a = U256::from_u128(&e, 12345);
        let b = U256::from_u128(&e, 67890);
        let d = U256::from_u32(&e, 1);
        assert_eq!(
            mul_div_floor(&e, &a, &b, &d),
            U256::from_u128(&e, 12345 * 67890)
        );
    }

    #[test]
    fn u512_large_divisor() {
        // Divisor > 2^192 — would overflow with 64-bit chunk long division.
        // Simulates getNextSqrtPriceFromAmount0 where denominator = L*Q96 + amt*sqrt.
        let e = Env::default();
        // 2^128 * 2^128 / 2^200 = 2^56
        let a = U256::from_u32(&e, 1).shl(128);
        let b = U256::from_u32(&e, 1).shl(128);
        let d = U256::from_u32(&e, 1).shl(200);
        let result = mul_div_floor(&e, &a, &b, &d);
        assert_eq!(result, U256::from_u32(&e, 1).shl(56));
    }

    #[test]
    fn u512_large_divisor_ceil() {
        // Same as above but with rounding up and a non-exact division.
        let e = Env::default();
        let a = U256::from_u32(&e, 1).shl(128).add(&U256::from_u32(&e, 1));
        let b = U256::from_u32(&e, 1).shl(128);
        let d = U256::from_u32(&e, 1).shl(200);
        // (2^128 + 1) * 2^128 / 2^200 = 2^56 + 2^(-72) → floor = 2^56, ceil = 2^56 + 1
        assert_eq!(mul_div_floor(&e, &a, &b, &d), U256::from_u32(&e, 1).shl(56));
        assert_eq!(
            mul_div_ceil(&e, &a, &b, &d),
            U256::from_u32(&e, 1).shl(56).add(&U256::from_u32(&e, 1))
        );
    }

    #[test]
    fn u512_very_large_divisor() {
        // Divisor >= 2^248 — exercises the bit-level division fallback.
        // Simulates getNextSqrtPriceFromAmount0 with large denominator.
        let e = Env::default();
        let a = U256::from_u32(&e, 1).shl(128);
        let b = U256::from_u32(&e, 1).shl(128);
        let d = U256::from_u32(&e, 1).shl(250);
        // 2^128 * 2^128 / 2^250 = 2^6 = 64
        let result = mul_div_floor(&e, &a, &b, &d);
        assert_eq!(result, U256::from_u32(&e, 64));
    }

    #[test]
    fn u512_very_large_divisor_ceil() {
        let e = Env::default();
        let a = U256::from_u32(&e, 1).shl(128).add(&U256::from_u32(&e, 1));
        let b = U256::from_u32(&e, 1).shl(128);
        let d = U256::from_u32(&e, 1).shl(250);
        // (2^128 + 1) * 2^128 / 2^250 = 64 + 2^(-122) → floor = 64, ceil = 65
        assert_eq!(mul_div_floor(&e, &a, &b, &d), U256::from_u32(&e, 64));
        assert_eq!(mul_div_ceil(&e, &a, &b, &d), U256::from_u32(&e, 65));
    }

    #[test]
    fn u512_max_divisor() {
        // Divisor = U256::MAX — extreme case for bit-level division.
        let e = Env::default();
        let max = U256::from_be_bytes(&e, &soroban_sdk::Bytes::from_array(&e, &[0xFF; 32]));
        // MAX * MAX / MAX = MAX
        let result = mul_div_floor(&e, &max, &max, &max);
        assert_eq!(result, max);
    }
}
