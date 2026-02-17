use crate::errors::ConcentratedPoolError as Error;
use crate::storage::{MAX_TICK, MIN_TICK};
use crate::u512::mul_div_u256 as u512_mul_div;
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::{Bytes, Env, U256};

const Q96_SHIFT: u32 = 96;
const Q128_SHIFT: u32 = 128;

const MIN_SQRT_RATIO_U128: u128 = 4_295_128_739;
const MAX_SQRT_RATIO_BYTES: [u8; 32] = [
    0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, //
    0x00, 0x00, 0x00, 0x00, //
    0xff, 0xfd, 0x89, 0x63, 0xef, 0xd1, 0xfc, 0x6a, 0x50, 0x64, 0x88, 0x49, 0x5d, 0x95, 0x1d, 0x52,
    0x63, 0x98, 0x8d, 0x26,
];

const TICK_MULTIPLIERS: [u128; 20] = [
    0xfffcb933bd6fad37aa2d162d1a594001,
    0xfff97272373d413259a46990580e213a,
    0xfff2e50f5f656932ef12357cf3c7fdcc,
    0xffe5caca7e10e4e61c3624eaa0941cd0,
    0xffcb9843d60f6159c9db58835c926644,
    0xff973b41fa98c081472e6896dfb254c0,
    0xff2ea16466c96a3843ec78b326b52861,
    0xfe5dee046a99a2a811c461f1969c3053,
    0xfcbe86c7900a88aedcffc83b479aa3a4,
    0xf987a7253ac413176f2b074cf7815e54,
    0xf3392b0822b70005940c7a398e4b70f3,
    0xe7159475a2c29b7443b29c7fa6e889d9,
    0xd097f3bdfd2022b8845ad8f792aa5825,
    0xa9f746462d870fdf8a65dc1f90e061e5,
    0x70d869a156d2a1b890bb3df62baf32f7,
    0x31be135f97d08fd981231505542fcfa6,
    0x09aa508b5b7a84e1c677de54f3e99bc9,
    0x005d6af8dedb81196699c329225ee604,
    0x0002216e584f5fa1ea926041bedfe98,
    0x000048a170391f7dc42444e8fa2,
];

fn u256_from_u128(e: &Env, value: u128) -> U256 {
    U256::from_u128(e, value)
}

fn u256_zero(e: &Env) -> U256 {
    U256::from_u32(e, 0)
}

fn u256_one(e: &Env) -> U256 {
    U256::from_u32(e, 1)
}

fn u256_q96(e: &Env) -> U256 {
    u256_one(e).shl(Q96_SHIFT)
}

fn u256_q128(e: &Env) -> U256 {
    u256_one(e).shl(Q128_SHIFT)
}

fn u256_q32(e: &Env) -> U256 {
    u256_one(e).shl(32)
}

fn u256_max(e: &Env) -> U256 {
    U256::from_be_bytes(e, &Bytes::from_array(e, &[0xFF; 32]))
}

// Wrapping subtraction for fee_growth accumulators.
// Returns (a - b) mod 2^256, matching Uniswap V3 semantics where
// fee_growth counters intentionally overflow.
pub fn wrapping_sub_u256(e: &Env, a: &U256, b: &U256) -> U256 {
    if a >= b {
        a.sub(b)
    } else {
        // (2^256 - 1) - b + a + 1  =  (a - b) mod 2^256
        u256_max(e).sub(b).add(a).add(&u256_one(e))
    }
}

// Wrapping addition for fee_growth accumulators.
// Returns (a + b) mod 2^256, matching Uniswap V3 semantics where
// fee_growth counters intentionally overflow.
pub fn wrapping_add_u256(e: &Env, a: &U256, b: &U256) -> U256 {
    let max = u256_max(e);
    // If a + b would overflow: max - a < b
    let remaining = max.sub(a);
    if remaining >= *b {
        a.add(b)
    } else {
        // (a + b) mod 2^256 = b - (max - a) - 1
        b.sub(&remaining).sub(&u256_one(e))
    }
}

fn u256_to_u128(value: &U256) -> Result<u128, Error> {
    value.to_u128().ok_or(Error::LiquidityOverflow)
}

fn u256_div_round_up(e: &Env, numerator: &U256, denominator: &U256) -> Result<U256, Error> {
    let zero = u256_zero(e);
    if *denominator == zero {
        return Err(Error::InvalidAmount);
    }
    if *numerator == zero {
        return Ok(zero);
    }

    let quotient = numerator.div(denominator);
    let remainder = numerator.rem_euclid(denominator);
    if remainder != zero {
        Ok(quotient.add(&u256_one(e)))
    } else {
        Ok(quotient)
    }
}

fn u256_mul_shift_128(e: &Env, a: &U256, b: &U256) -> Result<U256, Error> {
    Ok(a.fixed_mul_floor(e, b, &u256_q128(e)))
}

fn u256_mul_div(
    e: &Env,
    a: &U256,
    b: &U256,
    denominator: &U256,
    round_up: bool,
) -> Result<U256, Error> {
    let zero = u256_zero(e);
    if *denominator == zero {
        return Err(Error::InvalidAmount);
    }

    Ok(if round_up {
        a.fixed_mul_ceil(e, b, denominator)
    } else {
        a.fixed_mul_floor(e, b, denominator)
    })
}

pub fn min_sqrt_ratio(e: &Env) -> U256 {
    U256::from_u128(e, MIN_SQRT_RATIO_U128)
}

pub fn max_sqrt_ratio(e: &Env) -> U256 {
    U256::from_be_bytes(e, &Bytes::from_array(e, &MAX_SQRT_RATIO_BYTES))
}

pub fn sqrt_ratio_at_tick(e: &Env, tick: i32) -> Result<U256, Error> {
    if !(MIN_TICK..=MAX_TICK).contains(&tick) {
        return Err(Error::TickOutOfBounds);
    }

    let abs_tick = if tick < 0 {
        (-tick) as u32
    } else {
        tick as u32
    };

    let mut ratio = u256_q128(e);
    for (i, mul) in TICK_MULTIPLIERS.iter().enumerate() {
        if abs_tick & (1u32 << i) != 0 {
            ratio = u256_mul_shift_128(e, &ratio, &u256_from_u128(e, *mul))?;
        }
    }

    if tick > 0 {
        ratio = u256_max(e).div(&ratio);
    }

    let q32 = u256_q32(e);
    let mut sqrt_price_x96 = ratio.shr(32);
    if ratio.rem_euclid(&q32) != u256_zero(e) {
        sqrt_price_x96 = sqrt_price_x96.add(&u256_one(e));
    }

    Ok(sqrt_price_x96)
}

pub fn tick_at_sqrt_ratio(e: &Env, sqrt_price_x96: &U256) -> Result<i32, Error> {
    let min = min_sqrt_ratio(e);
    let max = max_sqrt_ratio(e);
    if *sqrt_price_x96 < min || *sqrt_price_x96 >= max {
        return Err(Error::PriceOutOfBounds);
    }

    let mut low = MIN_TICK;
    let mut high = MAX_TICK;
    while low < high {
        let mid = low + ((high - low + 1) / 2);
        let sqrt_mid = sqrt_ratio_at_tick(e, mid)?;
        if sqrt_mid <= *sqrt_price_x96 {
            low = mid;
        } else {
            high = mid - 1;
        }
    }

    Ok(low)
}

pub fn amount0_delta(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    liquidity: u128,
    round_up: bool,
) -> Result<u128, Error> {
    if liquidity == 0 {
        return Ok(0);
    }

    let mut sa = sqrt_price_a_x96.clone();
    let mut sb = sqrt_price_b_x96.clone();
    if sa > sb {
        core::mem::swap(&mut sa, &mut sb);
    }
    if sa == u256_zero(e) {
        return Err(Error::InvalidSqrtPrice);
    }

    // amount0 = L * Q96 * (sb - sa) / (sa * sb)
    // Matches Uniswap V3's two-step approach:
    //   temp = mulDiv(L * Q96, sb - sa, sb)   [U512 intermediate for ~384-bit product]
    //   amount0 = temp / sa                    [or divRoundingUp]
    let diff = sb.sub(&sa);
    let liquidity_u256 = u256_from_u128(e, liquidity);
    let numerator1 = liquidity_u256.mul(&u256_q96(e)); // L * Q96, ~224 bits, fits U256

    // Step 1: numerator1 * diff / sb (uses U512 for ~384-bit intermediate)
    let temp = u512_mul_div(e, &numerator1, &diff, &sb, round_up);

    // Step 2: temp / sa
    let amount_u256 = if round_up {
        u256_div_round_up(e, &temp, &sa)?
    } else {
        temp.div(&sa)
    };

    u256_to_u128(&amount_u256)
}

pub fn amount1_delta(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    liquidity: u128,
    round_up: bool,
) -> Result<u128, Error> {
    if liquidity == 0 {
        return Ok(0);
    }

    let mut sa = sqrt_price_a_x96.clone();
    let mut sb = sqrt_price_b_x96.clone();
    if sa > sb {
        core::mem::swap(&mut sa, &mut sb);
    }

    // amount1 = L * (sb - sa) / Q96
    // Uses U512 intermediate for L * diff product (~128 + 160 = 288 bits).
    // Matches Uniswap V3: FullMath.mulDiv(liquidity, sqrtRatioBX96 - sqrtRatioAX96, Q96)
    let diff = sb.sub(&sa);
    let liquidity_u256 = u256_from_u128(e, liquidity);
    let amount_u256 = u512_mul_div(e, &liquidity_u256, &diff, &u256_q96(e), round_up);

    u256_to_u128(&amount_u256)
}

pub fn mul_div_u128(
    e: &Env,
    amount: u128,
    numerator: u128,
    denominator: u128,
    round_up: bool,
) -> Result<u128, Error> {
    if denominator == 0 {
        return Err(Error::InvalidAmount);
    }

    Ok(if round_up {
        amount.fixed_mul_ceil(e, &numerator, &denominator)
    } else {
        amount.fixed_mul_floor(e, &numerator, &denominator)
    })
}

pub fn fee_growth_delta_x128(e: &Env, fee_amount: u128, liquidity: u128) -> Result<U256, Error> {
    if fee_amount == 0 || liquidity == 0 {
        return Ok(U256::from_u32(e, 0));
    }

    u256_mul_div(
        e,
        &u256_from_u128(e, fee_amount),
        &u256_q128(e),
        &u256_from_u128(e, liquidity),
        false,
    )
}

pub fn mul_div_fee_growth(
    e: &Env,
    growth_delta_x128: &U256,
    liquidity: u128,
) -> Result<u128, Error> {
    if liquidity == 0 {
        return Ok(0);
    }

    let value = u256_mul_div(
        e,
        growth_delta_x128,
        &u256_from_u128(e, liquidity),
        &u256_q128(e),
        false,
    )?;

    u256_to_u128(&value)
}

// Compute maximum liquidity mintable from a given amount of token0.
//
// Inverse of amount0_delta:
//   L = amount0 * sqrtA * sqrtB / (Q96 * (sqrtB - sqrtA))
//
// Computed as: L = amount0 * mulDiv(sqrtA, sqrtB, Q96) / (sqrtB - sqrtA)
// Uses U512 intermediates to avoid overflow.
pub fn liquidity_for_amount0(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    amount0: u128,
) -> Result<u128, Error> {
    if amount0 == 0 {
        return Ok(0);
    }

    let mut sa = sqrt_price_a_x96.clone();
    let mut sb = sqrt_price_b_x96.clone();
    if sa > sb {
        core::mem::swap(&mut sa, &mut sb);
    }

    let diff = sb.sub(&sa);
    if diff == u256_zero(e) {
        return Ok(0);
    }

    // intermediate = sqrtA * sqrtB / Q96 (via U512 to handle ~320-bit product)
    let intermediate = u512_mul_div(e, &sa, &sb, &u256_q96(e), false);

    // L = amount0 * intermediate / diff (round down)
    let amount_u256 = u256_from_u128(e, amount0);
    let result = u512_mul_div(e, &amount_u256, &intermediate, &diff, false);

    u256_to_u128(&result)
}

// Compute maximum liquidity mintable from a given amount of token1.
//
// Inverse of amount1_delta:
//   L = amount1 * Q96 / (sqrtB - sqrtA)
//
// Uses U512 intermediate for amount1 * Q96 product (~224 bits).
pub fn liquidity_for_amount1(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    amount1: u128,
) -> Result<u128, Error> {
    if amount1 == 0 {
        return Ok(0);
    }

    let mut sa = sqrt_price_a_x96.clone();
    let mut sb = sqrt_price_b_x96.clone();
    if sa > sb {
        core::mem::swap(&mut sa, &mut sb);
    }

    let diff = sb.sub(&sa);
    if diff == u256_zero(e) {
        return Ok(0);
    }

    // L = amount1 * Q96 / diff (round down)
    let amount_u256 = u256_from_u128(e, amount1);
    let result = u512_mul_div(e, &amount_u256, &u256_q96(e), &diff, false);

    u256_to_u128(&result)
}

// Compute next sqrt price given token0 input/output amount.
//
// When adding token0 (price decreases):
//   sqrt_next = L * sqrt_current * Q96 / (L * Q96 + amount * sqrt_current)
//
// When removing token0 (price increases):
//   sqrt_next = L * sqrt_current * Q96 / (L * Q96 - amount * sqrt_current)
//
// Uses U512 for the `amount * sqrt_current` product which can exceed 256 bits.
pub fn get_next_sqrt_price_from_amount0(
    e: &Env,
    sqrt_price_x96: &U256,
    liquidity: u128,
    amount: u128,
    add: bool,
) -> Result<U256, Error> {
    if amount == 0 {
        return Ok(sqrt_price_x96.clone());
    }

    let liq = u256_from_u128(e, liquidity);
    let q96 = u256_q96(e);

    // numerator1 = L * Q96 (fits in ~224 bits for realistic values)
    let numerator1 = liq.mul(&q96);

    // product = amount * sqrt_price (can overflow U256: ~128 + 160 = 288 bits)
    // Use U512 mul-div: amount * sqrt / 1 for the product value,
    // but we only need it for addition/subtraction with numerator1.
    //
    // Restructure to avoid overflow where possible:
    // denominator = numerator1 +/- amount * sqrt
    //
    // If amount * sqrt fits in U256, use direct arithmetic.
    // Otherwise, use the Uniswap V3 fallback:
    //   sqrt_next = numerator1 / (numerator1 / sqrt + amount)  [for add case]
    let amt = u256_from_u128(e, amount);
    let zero = u256_zero(e);
    let max_u256 = U256::from_be_bytes(e, &Bytes::from_array(e, &[0xFF; 32]));

    if add {
        // Check if amount * sqrt overflows U256
        let threshold = max_u256.div(sqrt_price_x96);
        if amt <= threshold {
            // No overflow: direct formula
            let product = amt.mul(sqrt_price_x96);
            let denominator = numerator1.add(&product);
            // sqrt_next = numerator1 * sqrt / denominator (round up)
            Ok(u512_mul_div(
                e,
                &numerator1,
                sqrt_price_x96,
                &denominator,
                true,
            ))
        } else {
            // Overflow fallback: sqrt_next = numerator1 / (numerator1 / sqrt + amount)
            let term = numerator1.div(sqrt_price_x96);
            let denominator = term.add(&amt);
            Ok(u256_div_round_up(e, &numerator1, &denominator)?)
        }
    } else {
        // Removing token0 (price increases)
        let threshold = max_u256.div(sqrt_price_x96);
        if amt <= threshold {
            let product = amt.mul(sqrt_price_x96);
            if numerator1 <= product {
                return Err(Error::PriceOutOfBounds);
            }
            let denominator = numerator1.sub(&product);
            if denominator == zero {
                return Err(Error::PriceOutOfBounds);
            }
            Ok(u512_mul_div(
                e,
                &numerator1,
                sqrt_price_x96,
                &denominator,
                true,
            ))
        } else {
            // Overflow means product > numerator1 for realistic values
            return Err(Error::PriceOutOfBounds);
        }
    }
}

// Compute next sqrt price given token1 input/output amount.
//
// When adding token1 (price increases):
//   sqrt_next = sqrt_current + amount * Q96 / L
//
// When removing token1 (price decreases):
//   sqrt_next = sqrt_current - amount * Q96 / L
//
// Uses U512 for `amount * Q96` which can exceed 256 bits.
pub fn get_next_sqrt_price_from_amount1(
    e: &Env,
    sqrt_price_x96: &U256,
    liquidity: u128,
    amount: u128,
    add: bool,
) -> Result<U256, Error> {
    let liq = u256_from_u128(e, liquidity);
    let q96 = u256_q96(e);
    let amt = u256_from_u128(e, amount);

    // Matches Uniswap V3's getNextSqrtPriceFromAmount1RoundingDown:
    // - add:      quotient rounds DOWN → result (sqrt + q) rounds DOWN
    // - subtract: quotient rounds UP   → result (sqrt - q) rounds DOWN
    if add {
        let quotient = u512_mul_div(e, &amt, &q96, &liq, false);
        Ok(sqrt_price_x96.add(&quotient))
    } else {
        let quotient = u512_mul_div(e, &amt, &q96, &liq, true);
        if *sqrt_price_x96 <= quotient {
            return Err(Error::PriceOutOfBounds);
        }
        Ok(sqrt_price_x96.sub(&quotient))
    }
}

// Compute next sqrt price given an input amount.
// Dispatches based on swap direction (zero_for_one).
pub fn get_next_sqrt_price_from_input(
    e: &Env,
    sqrt_price_x96: &U256,
    liquidity: u128,
    amount_in: u128,
    zero_for_one: bool,
) -> Result<U256, Error> {
    if zero_for_one {
        // Selling token0 → price goes down → use amount0 formula (add)
        get_next_sqrt_price_from_amount0(e, sqrt_price_x96, liquidity, amount_in, true)
    } else {
        // Selling token1 → price goes up → use amount1 formula (add)
        get_next_sqrt_price_from_amount1(e, sqrt_price_x96, liquidity, amount_in, true)
    }
}

// Compute next sqrt price given an output amount.
// Dispatches based on swap direction (zero_for_one).
pub fn get_next_sqrt_price_from_output(
    e: &Env,
    sqrt_price_x96: &U256,
    liquidity: u128,
    amount_out: u128,
    zero_for_one: bool,
) -> Result<U256, Error> {
    if zero_for_one {
        // Buying token1 (output) → use amount1 formula (remove)
        get_next_sqrt_price_from_amount1(e, sqrt_price_x96, liquidity, amount_out, false)
    } else {
        // Buying token0 (output) → use amount0 formula (remove)
        get_next_sqrt_price_from_amount0(e, sqrt_price_x96, liquidity, amount_out, false)
    }
}

#[cfg(test)]
mod test {
    use super::{
        max_sqrt_ratio, min_sqrt_ratio, sqrt_ratio_at_tick, tick_at_sqrt_ratio, u256_max, u256_one,
        wrapping_add_u256, wrapping_sub_u256,
    };
    use soroban_sdk::{Env, U256};

    #[test]
    fn tick_math_roundtrip() {
        let e = Env::default();

        for tick in [-887_272, -100_000, -1, 0, 1, 100_000, 887_271] {
            let sqrt = sqrt_ratio_at_tick(&e, tick).unwrap();
            let actual_tick = tick_at_sqrt_ratio(&e, &sqrt).unwrap();
            assert_eq!(actual_tick, tick);
        }
    }

    #[test]
    fn min_max_bounds_are_valid() {
        let e = Env::default();
        assert!(min_sqrt_ratio(&e) < max_sqrt_ratio(&e));
    }

    #[test]
    fn wrapping_sub_no_underflow() {
        let e = Env::default();
        let a = U256::from_u128(&e, 100);
        let b = U256::from_u128(&e, 30);
        assert_eq!(wrapping_sub_u256(&e, &a, &b), U256::from_u128(&e, 70));
    }

    #[test]
    fn wrapping_sub_equal() {
        let e = Env::default();
        let a = U256::from_u128(&e, 42);
        assert_eq!(wrapping_sub_u256(&e, &a, &a), U256::from_u32(&e, 0));
    }

    #[test]
    fn wrapping_sub_with_underflow() {
        let e = Env::default();
        let a = U256::from_u128(&e, 10);
        let b = U256::from_u128(&e, 30);
        // (2^256 - 1) - 30 + 10 + 1 = 2^256 - 20
        let expected = u256_max(&e).sub(&U256::from_u128(&e, 19));
        assert_eq!(wrapping_sub_u256(&e, &a, &b), expected);
    }

    #[test]
    fn wrapping_sub_double_wrap_identity() {
        let e = Env::default();
        let a = U256::from_u128(&e, 50);
        let b = U256::from_u128(&e, 200);
        // (a - b) + (b - a) should equal 0 mod 2^256
        // i.e. wrapping_sub(a, b) + wrapping_sub(b, a) = 0
        let diff1 = wrapping_sub_u256(&e, &a, &b);
        let diff2 = wrapping_sub_u256(&e, &b, &a);
        // diff2 = 150, diff1 = 2^256 - 150, sum = 2^256 which wraps to 0
        // Verify via: wrapping_sub(diff1, wrapping_sub(0, diff2)) == 0
        // Simpler: diff2 is just b - a = 150, and wrapping_sub(diff1, 0) should invert
        // Actually verify the concrete values:
        assert_eq!(diff2, U256::from_u128(&e, 150));
        let expected_diff1 =
            wrapping_sub_u256(&e, &U256::from_u32(&e, 0), &U256::from_u128(&e, 150));
        assert_eq!(diff1, expected_diff1);
    }

    #[test]
    fn wrapping_sub_zero_minus_one() {
        let e = Env::default();
        let zero = U256::from_u32(&e, 0);
        let one = u256_one(&e);
        // 0 - 1 mod 2^256 = 2^256 - 1 = U256::MAX
        assert_eq!(wrapping_sub_u256(&e, &zero, &one), u256_max(&e));
    }

    #[test]
    fn wrapping_sub_max_values() {
        let e = Env::default();
        let max = u256_max(&e);
        let zero = U256::from_u32(&e, 0);
        // MAX - 0 = MAX
        assert_eq!(wrapping_sub_u256(&e, &max, &zero), max);
        // MAX - MAX = 0
        assert_eq!(wrapping_sub_u256(&e, &max, &max), zero);
        // 0 - MAX = 1
        assert_eq!(wrapping_sub_u256(&e, &zero, &max), U256::from_u32(&e, 1));
    }

    #[test]
    fn wrapping_add_no_overflow() {
        let e = Env::default();
        let a = U256::from_u128(&e, 100);
        let b = U256::from_u128(&e, 200);
        assert_eq!(wrapping_add_u256(&e, &a, &b), U256::from_u128(&e, 300));
    }

    #[test]
    fn wrapping_add_zero() {
        let e = Env::default();
        let a = U256::from_u128(&e, 42);
        let zero = U256::from_u32(&e, 0);
        assert_eq!(wrapping_add_u256(&e, &a, &zero), a);
        assert_eq!(wrapping_add_u256(&e, &zero, &a), a);
    }

    #[test]
    fn wrapping_add_overflow() {
        let e = Env::default();
        let max = u256_max(&e);
        let one = u256_one(&e);
        // MAX + 1 mod 2^256 = 0
        assert_eq!(wrapping_add_u256(&e, &max, &one), U256::from_u32(&e, 0));
    }

    #[test]
    fn wrapping_add_overflow_both_large() {
        let e = Env::default();
        let max = u256_max(&e);
        // MAX + MAX mod 2^256 = (2^256 - 1) + (2^256 - 1) mod 2^256 = 2^257 - 2 mod 2^256 = 2^256 - 2
        let expected = max.sub(&u256_one(&e)); // MAX - 1
        assert_eq!(wrapping_add_u256(&e, &max, &max), expected);
    }

    #[test]
    fn wrapping_add_sub_roundtrip() {
        let e = Env::default();
        let a = U256::from_u128(&e, 12345);
        let b = U256::from_u128(&e, 67890);
        // (a + b) - b = a
        let sum = wrapping_add_u256(&e, &a, &b);
        assert_eq!(wrapping_sub_u256(&e, &sum, &b), a);
    }

    #[test]
    fn wrapping_add_sub_roundtrip_overflow() {
        let e = Env::default();
        let max = u256_max(&e);
        let val = U256::from_u128(&e, 100);
        // (MAX + val) wraps, then subtract val should give MAX
        let sum = wrapping_add_u256(&e, &max, &val);
        assert_eq!(wrapping_sub_u256(&e, &sum, &val), max);
    }
}
