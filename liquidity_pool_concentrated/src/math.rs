use crate::errors::Error;
use crate::storage::{MAX_TICK, MIN_TICK};
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

    // Rearranged to avoid large intermediate multiplication:
    // amount0 = liquidity * ((sb - sa) * Q96 / sb) / sa
    let price_ratio_q96 = if round_up {
        u256_mul_div(e, &sb.sub(&sa), &u256_q96(e), &sb, true)?
    } else {
        u256_mul_div(e, &sb.sub(&sa), &u256_q96(e), &sb, false)?
    };

    let liquidity_u256 = u256_from_u128(e, liquidity);
    let amount_u256 = if round_up {
        u256_div_round_up(e, &liquidity_u256.mul(&price_ratio_q96), &sa)?
    } else {
        liquidity_u256.fixed_mul_floor(e, &price_ratio_q96, &sa)
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

    // Use an intermediate 2^64 scale to preserve precision while avoiding overflow.
    let scale_q64 = u256_one(e).shl(64);
    let diff = sb.sub(&sa);
    let diff_ratio_q64 = if round_up {
        u256_mul_div(e, &diff, &scale_q64, &u256_q96(e), true)?
    } else {
        u256_mul_div(e, &diff, &scale_q64, &u256_q96(e), false)?
    };

    let liquidity_u256 = u256_from_u128(e, liquidity);
    let amount_u256 = if round_up {
        u256_mul_div(e, &liquidity_u256, &diff_ratio_q64, &scale_q64, true)?
    } else {
        u256_mul_div(e, &liquidity_u256, &diff_ratio_q64, &scale_q64, false)?
    };

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

#[cfg(test)]
mod test {
    use super::{max_sqrt_ratio, min_sqrt_ratio, sqrt_ratio_at_tick, tick_at_sqrt_ratio};
    use soroban_sdk::Env;

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
}
