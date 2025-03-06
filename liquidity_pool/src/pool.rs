use crate::constants::FEE_MULTIPLIER;
use crate::storage::get_fee_fraction;
use liquidity_pool_validation_errors::LiquidityPoolValidationError;
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::{panic_with_error, Env};

pub fn get_deposit_amounts(
    e: &Env,
    desired_a: u128,
    min_a: u128,
    desired_b: u128,
    min_b: u128,
    reserve_a: u128,
    reserve_b: u128,
) -> (u128, u128) {
    if reserve_a == 0 && reserve_b == 0 {
        return (desired_a, desired_b);
    }

    let amount_b = desired_a.fixed_mul_floor(e, &reserve_b, &reserve_a);
    if amount_b <= desired_b {
        if amount_b < min_b {
            panic_with_error!(e, LiquidityPoolValidationError::InvalidDepositAmount);
        }
        (desired_a, amount_b)
    } else {
        let amount_a = desired_b.fixed_mul_floor(&e, &reserve_a, &reserve_b);
        if amount_a > desired_a || desired_a < min_a {
            panic_with_error!(e, LiquidityPoolValidationError::InvalidDepositAmount);
        }
        (amount_a, desired_b)
    }
}

pub fn get_amount_out(
    e: &Env,
    in_amount: u128,
    reserve_sell: u128,
    reserve_buy: u128,
) -> (u128, u128) {
    if in_amount == 0 {
        return (0, 0);
    }

    // in * reserve_buy / (reserve_sell + in) - fee
    let fee_fraction = get_fee_fraction(&e);
    let result = in_amount.fixed_mul_floor(&e, &reserve_buy, &(reserve_sell + in_amount));
    let fee = result.fixed_mul_ceil(&e, &(fee_fraction as u128), &FEE_MULTIPLIER);
    (result - fee, fee)
}

pub fn get_amount_out_strict_receive(
    e: &Env,
    out_amount: u128,
    reserve_sell: u128,
    reserve_buy: u128,
) -> (u128, u128) {
    if out_amount == 0 {
        return (0, 0);
    }

    let dy_w_fee = out_amount.fixed_mul_ceil(
        &e,
        &FEE_MULTIPLIER,
        &(FEE_MULTIPLIER - get_fee_fraction(&e) as u128),
    );
    // if total value including fee is more than the reserve, math can't be done properly
    if dy_w_fee >= reserve_buy {
        panic_with_error!(e, LiquidityPoolValidationError::InsufficientBalance);
    }
    // +1 just in case there were some rounding errors & convert to real units in place
    let result = reserve_buy.fixed_mul_floor(&e, &reserve_sell, &(reserve_buy - dy_w_fee))
        - reserve_sell
        + 1;
    (result, dy_w_fee - out_amount)
}
