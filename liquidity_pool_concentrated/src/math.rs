use crate::constants::{MAX_TICK, MIN_TICK};
use crate::errors::ConcentratedPoolError as Error;
use crate::u512::{mul_div_ceil, mul_div_floor};
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::{panic_with_error, Bytes, Env, U256};
use utils::u256_math::ExtraMath;

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

fn u256_to_u128(e: &Env, value: &U256) -> u128 {
    value
        .to_u128()
        .unwrap_or_else(|| panic_with_error!(e, Error::LiquidityOverflow))
}

fn u256_div_round_up(e: &Env, numerator: &U256, denominator: &U256) -> U256 {
    let zero = u256_zero(e);
    if *denominator == zero {
        panic_with_error!(e, Error::InvalidAmount);
    }
    if *numerator == zero {
        return zero;
    }

    let quotient = numerator.div(denominator);
    let remainder = numerator.rem_euclid(denominator);
    if remainder != zero {
        quotient.add(&u256_one(e))
    } else {
        quotient
    }
}

fn u256_mul_shift_128(e: &Env, a: &U256, b: &U256) -> U256 {
    a.fixed_mul_floor(e, b, &u256_q128(e))
}

fn u256_mul_div(e: &Env, a: &U256, b: &U256, denominator: &U256, round_up: bool) -> U256 {
    let zero = u256_zero(e);
    if *denominator == zero {
        panic_with_error!(e, Error::InvalidAmount);
    }

    if round_up {
        a.fixed_mul_ceil(e, b, denominator)
    } else {
        a.fixed_mul_floor(e, b, denominator)
    }
}

pub fn min_sqrt_ratio(e: &Env) -> U256 {
    U256::from_u128(e, MIN_SQRT_RATIO_U128)
}

pub fn max_sqrt_ratio(e: &Env) -> U256 {
    U256::from_be_bytes(e, &Bytes::from_array(e, &MAX_SQRT_RATIO_BYTES))
}

pub fn sqrt_ratio_at_tick(e: &Env, tick: i32) -> U256 {
    if !(MIN_TICK..=MAX_TICK).contains(&tick) {
        panic_with_error!(e, Error::TickOutOfBounds);
    }

    let abs_tick = if tick < 0 {
        (-tick) as u32
    } else {
        tick as u32
    };

    let mut ratio = u256_q128(e);
    for (i, mul) in TICK_MULTIPLIERS.iter().enumerate() {
        if abs_tick & (1u32 << i) != 0 {
            ratio = u256_mul_shift_128(e, &ratio, &u256_from_u128(e, *mul));
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

    sqrt_price_x96
}

pub fn tick_at_sqrt_ratio(e: &Env, sqrt_price_x96: &U256) -> i32 {
    let min = min_sqrt_ratio(e);
    let max = max_sqrt_ratio(e);
    if *sqrt_price_x96 < min || *sqrt_price_x96 >= max {
        panic_with_error!(e, Error::PriceOutOfBounds);
    }

    const LOG_SQRT10001: u128 = 255_738_958_999_603_826_347_141;
    const TICK_LOW_ERROR: u128 = 3_402_992_956_809_132_418_596_140_100_660_247_210;
    const TICK_HI_ERROR: u128 = 291_339_464_771_989_622_907_027_621_153_398_088_495;

    // Convert Q64.96 -> Q128.128.
    let ratio = sqrt_price_x96.shl(32);
    let ratio_hi_u256 = ratio.shr(128);
    let ratio_hi = u256_to_u128(e, &ratio_hi_u256);
    let ratio_lo_u256 = ratio.sub(&ratio_hi_u256.shl(128));
    let ratio_lo = u256_to_u128(e, &ratio_lo_u256);

    let msb: u32 = if ratio_hi > 0 {
        128 + (127 - ratio_hi.leading_zeros())
    } else {
        127 - ratio_lo.leading_zeros()
    };

    // Normalize so r is in [2^127, 2^128).
    let mut r: u128 = if msb >= 128 {
        let s = msb - 127;
        (ratio_hi << (128 - s)) | (ratio_lo >> s)
    } else {
        ratio_lo << (127 - msb)
    };

    // Fixed-point log2 in Q64.64.
    let mut log_2: i128 = ((msb as i128) - 128) << 64;
    for bit_pos in (50u32..=63u32).rev() {
        let (sq_hi, sq_lo) = widening_mul(r, r);
        let f = sq_hi >> 127; // 0 or 1
        log_2 |= (f as i128) << bit_pos;
        r = if f == 0 {
            // (r * r) >> 127
            (sq_hi << 1) | (sq_lo >> 127)
        } else {
            // ((r * r) >> 127) >> 1 == (r * r) >> 128
            sq_hi
        };
    }

    // Convert log2 -> log_sqrt(1.0001), represented as signed 128.128.
    let neg = log_2 < 0;
    let abs_log_2 = log_2.unsigned_abs();
    let (mul_hi, mul_lo) = widening_mul(abs_log_2, LOG_SQRT10001);

    // Signed 256-bit representation split into (hi, lo):
    // value = hi * 2^128 + lo, where hi is signed and lo is unsigned.
    let (log_hi, log_lo): (i128, u128) = if !neg {
        (mul_hi as i128, mul_lo)
    } else if mul_lo == 0 {
        (-(mul_hi as i128), 0)
    } else {
        (-(mul_hi as i128) - 1, mul_lo.wrapping_neg())
    };

    // Arithmetic shifts by 128 after applying low/high error bounds.
    let tick_low = (log_hi - if log_lo < TICK_LOW_ERROR { 1 } else { 0 }) as i32;
    let tick_hi = (log_hi
        + if log_lo.overflowing_add(TICK_HI_ERROR).1 {
            1
        } else {
            0
        }) as i32;

    if tick_low == tick_hi {
        tick_low
    } else if sqrt_ratio_at_tick(e, tick_hi) <= *sqrt_price_x96 {
        tick_hi
    } else {
        tick_low
    }
}

// 128-bit x 128-bit -> 256-bit unsigned multiply.
// Returns (hi, lo) where result = hi * 2^128 + lo.
fn widening_mul(a: u128, b: u128) -> (u128, u128) {
    let a0 = a & 0xFFFF_FFFF_FFFF_FFFF;
    let a1 = a >> 64;
    let b0 = b & 0xFFFF_FFFF_FFFF_FFFF;
    let b1 = b >> 64;

    let p00 = a0 * b0;
    let p01 = a0 * b1;
    let p10 = a1 * b0;
    let p11 = a1 * b1;

    let mid = (p00 >> 64) + (p01 & 0xFFFF_FFFF_FFFF_FFFF) + (p10 & 0xFFFF_FFFF_FFFF_FFFF);

    let lo = (p00 & 0xFFFF_FFFF_FFFF_FFFF) | ((mid & 0xFFFF_FFFF_FFFF_FFFF) << 64);
    let hi = p11 + (p01 >> 64) + (p10 >> 64) + (mid >> 64);

    (hi, lo)
}

// ## Overflow safety for amount deltas
//
// Concentrated liquidity can produce very large L values at extreme ticks
// (e.g. L=1e30 from just 2600 USDC at ticks [879400, 887200]).  When computing
// token amounts across wide tick ranges with such liquidity, the mathematical
// result can exceed u128::MAX.
//
// Saturation to u128::MAX is safe because:
// 1. Soroban token transfers use i128 (max ~1.7e38).  No real token amount can
//    exceed i128::MAX, so any value > i128::MAX is unrealizable.
// 2. In the swap loop, `amount_remaining` (user input) bounds actual consumption.
//    A saturated target-amount just means "input can't reach the target tick" —
//    the swap correctly falls back to `get_next_sqrt_price_from_input`.
// 3. On withdraw, reserve checks (`InsufficientBalance`) catch impossible values.
// 4. max_liquidity_per_tick (u128::MAX / num_ticks ≈ 3.83e34 for spacing=200)
//    prevents liquidity_gross/net overflow.  Even for 18-decimal tokens at
//    price ratios up to ~1e12, L stays well within this cap.
//
// The `try_` variants return `Option<u128>` for swap-step target checks where
// overflow means "remaining amount cannot reach the target tick".

// Internal helper: compute amount0 delta as U256 (no truncation).
fn amount0_delta_u256(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    liquidity: u128,
    round_up: bool,
) -> Option<U256> {
    if liquidity == 0 {
        return Some(u256_zero(e));
    }

    let mut sa = sqrt_price_a_x96.clone();
    let mut sb = sqrt_price_b_x96.clone();
    if sa > sb {
        core::mem::swap(&mut sa, &mut sb);
    }
    if sa == u256_zero(e) {
        return None;
    }

    // amount0 = L * Q96 * (sb - sa) / (sa * sb)
    let diff = sb.sub(&sa);
    let liquidity_u256 = u256_from_u128(e, liquidity);
    let numerator1 = liquidity_u256.mul(&u256_q96(e));

    let temp = if round_up {
        mul_div_ceil(e, &numerator1, &diff, &sb)
    } else {
        mul_div_floor(e, &numerator1, &diff, &sb)
    };

    let amount = if round_up {
        u256_div_round_up(e, &temp, &sa)
    } else {
        temp.div(&sa)
    };

    Some(amount)
}

// Internal helper: compute amount1 delta as U256 (no truncation).
fn amount1_delta_u256(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    liquidity: u128,
    round_up: bool,
) -> U256 {
    if liquidity == 0 {
        return u256_zero(e);
    }

    let mut sa = sqrt_price_a_x96.clone();
    let mut sb = sqrt_price_b_x96.clone();
    if sa > sb {
        core::mem::swap(&mut sa, &mut sb);
    }

    let diff = sb.sub(&sa);
    let liquidity_u256 = u256_from_u128(e, liquidity);
    if round_up {
        mul_div_ceil(e, &liquidity_u256, &diff, &u256_q96(e))
    } else {
        mul_div_floor(e, &liquidity_u256, &diff, &u256_q96(e))
    }
}

// Compute token0 amount between two sqrt prices. Saturates at u128::MAX
// on overflow instead of panicking (with large liquidity at wide ranges the
// mathematical result can exceed u128 but is bounded by actual reserves).
pub fn amount0_delta(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    liquidity: u128,
    round_up: bool,
) -> u128 {
    match amount0_delta_u256(e, sqrt_price_a_x96, sqrt_price_b_x96, liquidity, round_up) {
        Some(v) => v.to_u128().unwrap_or(u128::MAX),
        None => panic_with_error!(e, Error::InvalidSqrtPrice),
    }
}

// Like `amount0_delta` but returns `None` when the result overflows u128
// or when a sqrt price is zero (invalid).  Used in swap-step target
// calculations where `None` means "the remaining input can never reach
// the target tick".
pub fn try_amount0_delta(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    liquidity: u128,
    round_up: bool,
) -> Option<u128> {
    match amount0_delta_u256(e, sqrt_price_a_x96, sqrt_price_b_x96, liquidity, round_up) {
        Some(v) => v.to_u128(),
        None => None, // sqrt_price was zero (invalid)
    }
}

// Compute token1 amount between two sqrt prices. Saturates at u128::MAX
// on overflow.
pub fn amount1_delta(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    liquidity: u128,
    round_up: bool,
) -> u128 {
    amount1_delta_u256(e, sqrt_price_a_x96, sqrt_price_b_x96, liquidity, round_up)
        .to_u128()
        .unwrap_or(u128::MAX)
}

// Like `amount1_delta` but returns `None` when the result overflows u128.
// Unlike `try_amount0_delta`, this cannot return `None` for invalid sqrt
// prices because `amount1_delta` only divides by Q96 (a constant).
pub fn try_amount1_delta(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    liquidity: u128,
    round_up: bool,
) -> Option<u128> {
    amount1_delta_u256(e, sqrt_price_a_x96, sqrt_price_b_x96, liquidity, round_up).to_u128()
}

pub fn mul_div_u128(
    e: &Env,
    amount: u128,
    numerator: u128,
    denominator: u128,
    round_up: bool,
) -> u128 {
    if denominator == 0 {
        panic_with_error!(e, Error::InvalidAmount);
    }

    if round_up {
        amount.fixed_mul_ceil(e, &numerator, &denominator)
    } else {
        amount.fixed_mul_floor(e, &numerator, &denominator)
    }
}

pub fn fee_growth_delta_x128(e: &Env, fee_amount: u128, liquidity: u128) -> U256 {
    if fee_amount == 0 || liquidity == 0 {
        return U256::from_u32(e, 0);
    }

    u256_mul_div(
        e,
        &u256_from_u128(e, fee_amount),
        &u256_q128(e),
        &u256_from_u128(e, liquidity),
        false,
    )
}

pub fn mul_div_fee_growth(e: &Env, growth_delta_x128: &U256, liquidity: u128) -> u128 {
    if liquidity == 0 {
        return 0;
    }

    let value = u256_mul_div(
        e,
        growth_delta_x128,
        &u256_from_u128(e, liquidity),
        &u256_q128(e),
        false,
    );

    u256_to_u128(e, &value)
}

// Compute maximum liquidity mintable from a given amount of token0.
pub fn liquidity_for_amount0(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    amount0: u128,
) -> u128 {
    if amount0 == 0 {
        return 0;
    }

    let mut sa = sqrt_price_a_x96.clone();
    let mut sb = sqrt_price_b_x96.clone();
    if sa > sb {
        core::mem::swap(&mut sa, &mut sb);
    }

    let diff = sb.sub(&sa);
    if diff == u256_zero(e) {
        return 0;
    }

    let intermediate = mul_div_floor(e, &sa, &sb, &u256_q96(e));
    let amount_u256 = u256_from_u128(e, amount0);
    let result = mul_div_floor(e, &amount_u256, &intermediate, &diff);

    u256_to_u128(e, &result)
}

// Compute maximum liquidity mintable from a given amount of token1.
pub fn liquidity_for_amount1(
    e: &Env,
    sqrt_price_a_x96: &U256,
    sqrt_price_b_x96: &U256,
    amount1: u128,
) -> u128 {
    if amount1 == 0 {
        return 0;
    }

    let mut sa = sqrt_price_a_x96.clone();
    let mut sb = sqrt_price_b_x96.clone();
    if sa > sb {
        core::mem::swap(&mut sa, &mut sb);
    }

    let diff = sb.sub(&sa);
    if diff == u256_zero(e) {
        return 0;
    }

    let amount_u256 = u256_from_u128(e, amount1);
    let result = mul_div_floor(e, &amount_u256, &u256_q96(e), &diff);

    u256_to_u128(e, &result)
}

// Compute next sqrt price given token0 input/output amount.
pub fn get_next_sqrt_price_from_amount0(
    e: &Env,
    sqrt_price_x96: &U256,
    liquidity: u128,
    amount: u128,
    add: bool,
) -> U256 {
    if amount == 0 {
        return sqrt_price_x96.clone();
    }

    let liq = u256_from_u128(e, liquidity);
    let q96 = u256_q96(e);

    let numerator1 = liq.mul(&q96);
    let amt = u256_from_u128(e, amount);
    let max_u256 = U256::from_be_bytes(e, &Bytes::from_array(e, &[0xFF; 32]));

    if add {
        let threshold = max_u256.div(sqrt_price_x96);
        if amt <= threshold {
            let product = amt.mul(sqrt_price_x96);
            let denominator = numerator1.add(&product);
            mul_div_ceil(e, &numerator1, sqrt_price_x96, &denominator)
        } else {
            let term = numerator1.div(sqrt_price_x96);
            let denominator = term.add(&amt);
            u256_div_round_up(e, &numerator1, &denominator)
        }
    } else {
        let threshold = max_u256.div(sqrt_price_x96);
        if amt <= threshold {
            let product = amt.mul(sqrt_price_x96);
            if numerator1 <= product {
                panic_with_error!(e, Error::PriceOutOfBounds);
            }
            let denominator = numerator1.sub(&product);
            if denominator == u256_zero(e) {
                panic_with_error!(e, Error::PriceOutOfBounds);
            }
            mul_div_ceil(e, &numerator1, sqrt_price_x96, &denominator)
        } else {
            panic_with_error!(e, Error::PriceOutOfBounds);
        }
    }
}

// Compute next sqrt price given token1 input/output amount.
pub fn get_next_sqrt_price_from_amount1(
    e: &Env,
    sqrt_price_x96: &U256,
    liquidity: u128,
    amount: u128,
    add: bool,
) -> U256 {
    let liq = u256_from_u128(e, liquidity);
    let q96 = u256_q96(e);
    let amt = u256_from_u128(e, amount);

    if add {
        let quotient = mul_div_floor(e, &amt, &q96, &liq);
        sqrt_price_x96.add(&quotient)
    } else {
        let quotient = mul_div_ceil(e, &amt, &q96, &liq);
        if *sqrt_price_x96 <= quotient {
            panic_with_error!(e, Error::PriceOutOfBounds);
        }
        sqrt_price_x96.sub(&quotient)
    }
}

// Compute next sqrt price given an input amount.
pub fn get_next_sqrt_price_from_input(
    e: &Env,
    sqrt_price_x96: &U256,
    liquidity: u128,
    amount_in: u128,
    zero_for_one: bool,
) -> U256 {
    if zero_for_one {
        get_next_sqrt_price_from_amount0(e, sqrt_price_x96, liquidity, amount_in, true)
    } else {
        get_next_sqrt_price_from_amount1(e, sqrt_price_x96, liquidity, amount_in, true)
    }
}

// Compute next sqrt price given an output amount.
pub fn get_next_sqrt_price_from_output(
    e: &Env,
    sqrt_price_x96: &U256,
    liquidity: u128,
    amount_out: u128,
    zero_for_one: bool,
) -> U256 {
    if zero_for_one {
        get_next_sqrt_price_from_amount1(e, sqrt_price_x96, liquidity, amount_out, false)
    } else {
        get_next_sqrt_price_from_amount0(e, sqrt_price_x96, liquidity, amount_out, false)
    }
}

// Compute sqrt_price_x96 from a token amount ratio.
// sqrt_price_x96 = sqrt(amount1 / amount0) * 2^96
pub fn sqrt_price_from_amounts(e: &Env, amount0: u128, amount1: u128) -> U256 {
    if amount0 == 0 || amount1 == 0 {
        panic_with_error!(e, Error::InvalidAmount);
    }
    let q96 = u256_q96(e);
    let q192 = q96.mul(&q96);
    let amount0_u256 = u256_from_u128(e, amount0);
    let amount1_u256 = u256_from_u128(e, amount1);
    let ratio_q192 = mul_div_floor(e, &amount1_u256, &q192, &amount0_u256);
    let sqrt_price = ratio_q192.sqrt();

    if sqrt_price < min_sqrt_ratio(e) || sqrt_price >= max_sqrt_ratio(e) {
        panic_with_error!(e, Error::PriceOutOfBounds);
    }
    sqrt_price
}

#[cfg(test)]
mod test {
    use super::{
        max_sqrt_ratio, min_sqrt_ratio, sqrt_ratio_at_tick, tick_at_sqrt_ratio, try_amount0_delta,
        try_amount1_delta, u256_max, u256_one, widening_mul, wrapping_add_u256, wrapping_sub_u256,
    };
    use crate::constants::{MAX_TICK, MIN_TICK};
    use soroban_sdk::{Env, U256};

    #[test]
    fn tick_math_roundtrip() {
        let e = Env::default();

        for tick in [-887_272, -100_000, -1, 0, 1, 100_000, 887_271] {
            let sqrt = sqrt_ratio_at_tick(&e, tick);
            let actual_tick = tick_at_sqrt_ratio(&e, &sqrt);
            assert_eq!(actual_tick, tick);
        }
    }

    #[test]
    fn tick_at_sqrt_ratio_v3_parity() {
        let e = Env::default();

        for tick in [
            MIN_TICK,
            MIN_TICK + 1,
            -100_000,
            -1,
            0,
            1,
            100_000,
            MAX_TICK - 2,
            MAX_TICK - 1,
        ] {
            let sqrt = sqrt_ratio_at_tick(&e, tick);
            assert_eq!(tick_at_sqrt_ratio(&e, &sqrt), tick);

            if tick < MAX_TICK {
                let next = sqrt_ratio_at_tick(&e, tick + 1);
                let just_below_next = next.sub(&U256::from_u32(&e, 1));
                assert_eq!(tick_at_sqrt_ratio(&e, &just_below_next), tick);
            }
        }
    }

    #[test]
    fn widening_mul_basic() {
        assert_eq!(widening_mul(1, 1), (0, 1));

        let max64 = u64::MAX as u128;
        assert_eq!(widening_mul(max64, max64), (0, max64 * max64));

        assert_eq!(widening_mul(1u128 << 127, 2), (1, 0));
        assert_eq!(widening_mul(u128::MAX, 2), (1, u128::MAX - 1));
        assert_eq!(widening_mul(u128::MAX, u128::MAX), (u128::MAX - 1, 1));
    }

    #[test]
    fn tick_at_sqrt_ratio_boundaries() {
        let e = Env::default();

        let min = min_sqrt_ratio(&e);
        let max = max_sqrt_ratio(&e);
        let max_minus_one = max.sub(&U256::from_u32(&e, 1));

        assert_eq!(tick_at_sqrt_ratio(&e, &min), MIN_TICK);
        assert_eq!(tick_at_sqrt_ratio(&e, &max_minus_one), MAX_TICK - 1);

        for tick in [-500_000, -50_000, 50_000, 500_000] {
            let lower = sqrt_ratio_at_tick(&e, tick);
            let upper = sqrt_ratio_at_tick(&e, tick + 1);
            let mid = lower.add(&upper).div(&U256::from_u32(&e, 2));
            assert_eq!(tick_at_sqrt_ratio(&e, &mid), tick);
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
        let expected = u256_max(&e).sub(&U256::from_u128(&e, 19));
        assert_eq!(wrapping_sub_u256(&e, &a, &b), expected);
    }

    #[test]
    fn wrapping_sub_double_wrap_identity() {
        let e = Env::default();
        let a = U256::from_u128(&e, 50);
        let b = U256::from_u128(&e, 200);
        let diff1 = wrapping_sub_u256(&e, &a, &b);
        let diff2 = wrapping_sub_u256(&e, &b, &a);
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
        assert_eq!(wrapping_sub_u256(&e, &zero, &one), u256_max(&e));
    }

    #[test]
    fn wrapping_sub_max_values() {
        let e = Env::default();
        let max = u256_max(&e);
        let zero = U256::from_u32(&e, 0);
        assert_eq!(wrapping_sub_u256(&e, &max, &zero), max);
        assert_eq!(wrapping_sub_u256(&e, &max, &max), zero);
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
        assert_eq!(wrapping_add_u256(&e, &max, &one), U256::from_u32(&e, 0));
    }

    #[test]
    fn wrapping_add_overflow_both_large() {
        let e = Env::default();
        let max = u256_max(&e);
        let expected = max.sub(&u256_one(&e));
        assert_eq!(wrapping_add_u256(&e, &max, &max), expected);
    }

    #[test]
    fn wrapping_add_sub_roundtrip() {
        let e = Env::default();
        let a = U256::from_u128(&e, 12345);
        let b = U256::from_u128(&e, 67890);
        let sum = wrapping_add_u256(&e, &a, &b);
        assert_eq!(wrapping_sub_u256(&e, &sum, &b), a);
    }

    #[test]
    fn wrapping_add_sub_roundtrip_overflow() {
        let e = Env::default();
        let max = u256_max(&e);
        let val = U256::from_u128(&e, 100);
        let sum = wrapping_add_u256(&e, &max, &val);
        assert_eq!(wrapping_sub_u256(&e, &sum, &val), max);
    }

    #[test]
    fn try_amount0_delta_returns_none_on_overflow() {
        let e = Env::default();
        // L=1e30 across nearly the entire tick range: result >> u128::MAX
        let sqrt_min = sqrt_ratio_at_tick(&e, MIN_TICK);
        let sqrt_max = sqrt_ratio_at_tick(&e, MAX_TICK - 1);
        let liquidity = 1_000_000_000_000_000_000_000_000_000_000u128; // 1e30
        assert_eq!(
            try_amount0_delta(&e, &sqrt_min, &sqrt_max, liquidity, true),
            None
        );
    }

    #[test]
    fn try_amount1_delta_returns_none_on_overflow() {
        let e = Env::default();
        let sqrt_min = sqrt_ratio_at_tick(&e, MIN_TICK);
        let sqrt_max = sqrt_ratio_at_tick(&e, MAX_TICK - 1);
        let liquidity = 1_000_000_000_000_000_000_000_000_000_000u128; // 1e30
        assert_eq!(
            try_amount1_delta(&e, &sqrt_min, &sqrt_max, liquidity, true),
            None
        );
    }

    #[test]
    fn try_amount0_delta_zero_liquidity_returns_some_zero() {
        let e = Env::default();
        let sqrt_a = sqrt_ratio_at_tick(&e, -1000);
        let sqrt_b = sqrt_ratio_at_tick(&e, 1000);
        assert_eq!(try_amount0_delta(&e, &sqrt_a, &sqrt_b, 0, true), Some(0));
    }

    #[test]
    fn try_amount0_delta_equal_prices_returns_some_zero() {
        let e = Env::default();
        let sqrt_a = sqrt_ratio_at_tick(&e, 0);
        assert_eq!(
            try_amount0_delta(&e, &sqrt_a, &sqrt_a, 1_000_000, true),
            Some(0)
        );
    }
}
