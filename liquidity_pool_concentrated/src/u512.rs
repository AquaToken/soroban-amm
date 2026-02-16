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

// Full 512-bit product of two U256 values using schoolbook multiplication.
///
// Split each operand into two 128-bit halves:
//   a = a_hi * 2^128 + a_lo,   b = b_hi * 2^128 + b_lo
///
// Product = a_hi*b_hi * 2^256 + (a_hi*b_lo + a_lo*b_hi) * 2^128 + a_lo*b_lo
///
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
    let cross = hi_lo.add(&lo_hi);
    let cross_carry = if cross < hi_lo {
        U256::from_u32(e, 1)
    } else {
        U256::from_u32(e, 0)
    };

    // lo = lo_lo + lo128(cross) << 128
    let cross_lo_shifted = lo128(e, &cross).shl(128);
    let lo = lo_lo.add(&cross_lo_shifted);
    let lo_carry = if lo < lo_lo {
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

// Compute `floor(a * b / d)` or `ceil(a * b / d)` with 512-bit intermediate precision.
///
// Uses schoolbook multiplication for the 512-bit product, then long division
// processing the dividend in 64-bit chunks (8 iterations).
///
// Panics if `d == 0`.
pub fn mul_div_u256(e: &Env, a: &U256, b: &U256, d: &U256, round_up: bool) -> U256 {
    let zero = U256::from_u32(e, 0);
    let one = U256::from_u32(e, 1);

    if *a == zero || *b == zero {
        return zero;
    }

    // Fast path: if a * b fits in 256 bits, use direct arithmetic.
    let max_u256 = U256::from_be_bytes(e, &soroban_sdk::Bytes::from_array(e, &[0xFF; 32]));
    let threshold = max_u256.div(b);
    if *a <= threshold {
        let product = a.mul(b);
        let quotient = product.div(d);
        let remainder = product.rem_euclid(d);
        return if round_up && remainder != zero {
            quotient.add(&one)
        } else {
            quotient
        };
    }

    // Slow path: full 512-bit product + long division in 64-bit chunks.
    let U512 { hi, lo } = u256_full_mul(e, a, b);

    // Extract eight 64-bit chunks via to_be_bytes.
    let hi_bytes = hi.to_be_bytes();
    let lo_bytes = lo.to_be_bytes();

    let mut remainder = zero.clone();
    let mut quotient = zero.clone();

    // Process 8 chunks: 4 from hi (most significant), 4 from lo.
    for chunk_idx in 0u32..8 {
        let chunk_val = if chunk_idx < 4 {
            extract_u64_from_bytes(&hi_bytes, chunk_idx)
        } else {
            extract_u64_from_bytes(&lo_bytes, chunk_idx - 4)
        };

        // remainder = remainder * 2^64 + chunk
        remainder = remainder.shl(64).add(&U256::from_u128(e, chunk_val as u128));

        // q_chunk = remainder / d
        let q_chunk = remainder.div(d);
        remainder = remainder.rem_euclid(d);

        // quotient = quotient * 2^64 + q_chunk
        quotient = quotient.shl(64).add(&q_chunk);
    }

    if round_up && remainder != U256::from_u32(e, 0) {
        quotient = quotient.add(&one);
    }

    quotient
}

// Extract a 64-bit big-endian value from a 32-byte Bytes at the given chunk index.
// chunk_idx 0 = bytes [0..8] (most significant), chunk_idx 3 = bytes [24..32].
fn extract_u64_from_bytes(bytes: &soroban_sdk::Bytes, chunk_idx: u32) -> u64 {
    let offset = chunk_idx * 8;
    let mut val: u64 = 0;
    for i in 0..8u32 {
        let byte = bytes.get(offset + i).unwrap_or(0);
        val = (val << 8) | (byte as u64);
    }
    val
}

#[cfg(test)]
mod test {
    use super::mul_div_u256;
    use soroban_sdk::{Env, U256};

    #[test]
    fn u512_small_exact() {
        let e = Env::default();
        let a = U256::from_u128(&e, 1_000_000);
        let b = U256::from_u128(&e, 2_000_000);
        let d = U256::from_u128(&e, 500_000);
        assert_eq!(
            mul_div_u256(&e, &a, &b, &d, false),
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
            mul_div_u256(&e, &a, &b, &d, false),
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
        assert_eq!(mul_div_u256(&e, &a, &b, &d, false), a);
    }

    #[test]
    fn u512_rounding_down() {
        let e = Env::default();
        // 10 * 3 / 7 = 4.285... → floor = 4
        let a = U256::from_u128(&e, 10);
        let b = U256::from_u128(&e, 3);
        let d = U256::from_u128(&e, 7);
        assert_eq!(mul_div_u256(&e, &a, &b, &d, false), U256::from_u128(&e, 4));
    }

    #[test]
    fn u512_rounding_up() {
        let e = Env::default();
        // 10 * 3 / 7 = 4.285... → ceil = 5
        let a = U256::from_u128(&e, 10);
        let b = U256::from_u128(&e, 3);
        let d = U256::from_u128(&e, 7);
        assert_eq!(mul_div_u256(&e, &a, &b, &d, true), U256::from_u128(&e, 5));
    }

    #[test]
    fn u512_large_realistic() {
        let e = Env::default();
        // liquidity (u128::MAX) * sqrt_price (2^160) / Q96 (2^96)
        // Intermediate ~2^288 (overflows U256), result = u128::MAX * 2^64
        let liq = U256::from_u128(&e, u128::MAX);
        let sqrt = U256::from_u32(&e, 1).shl(160);
        let q96 = U256::from_u32(&e, 1).shl(96);
        let result = mul_div_u256(&e, &liq, &sqrt, &q96, false);
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
            mul_div_u256(&e, &a, &b, &d, false),
            U256::from_u128(&e, 1097393681)
        );
    }

    #[test]
    fn u512_divisor_one() {
        let e = Env::default();
        let a = U256::from_u128(&e, 12345);
        let b = U256::from_u128(&e, 67890);
        let d = U256::from_u32(&e, 1);
        assert_eq!(
            mul_div_u256(&e, &a, &b, &d, false),
            U256::from_u128(&e, 12345 * 67890)
        );
    }
}
