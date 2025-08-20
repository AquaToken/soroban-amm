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
        if amount_a > desired_a || amount_a < min_a {
            panic_with_error!(e, LiquidityPoolValidationError::InvalidDepositAmount);
        }
        (amount_a, desired_b)
    }
}

pub fn get_amount_out(
    e: &Env,
    in_amount: u128,    // dx  – exact tokens the trader wants to sell
    reserve_sell: u128, // x
    reserve_buy: u128,  // y
) -> (u128, u128) {
    if in_amount == 0 {
        return (0, 0);
    }

    let fee_fraction = get_fee_fraction(e) as u128; // e.g. 30 => 0.3 %
    let in_after_fee = in_amount * (FEE_MULTIPLIER - fee_fraction) / FEE_MULTIPLIER;
    let raw_out = in_after_fee.fixed_mul_floor(e, &reserve_buy, &(reserve_sell + in_after_fee));
    (raw_out, in_amount - in_after_fee) // fee is taken on input
}

pub fn get_amount_out_strict_receive(
    e: &Env,
    out_amount: u128,   // dy  – exact tokens the trader wants to receive
    reserve_sell: u128, // x
    reserve_buy: u128,  // y
) -> (u128, u128) {
    if out_amount == 0 {
        return (0, 0);
    }
    if out_amount >= reserve_buy {
        panic_with_error!(e, LiquidityPoolValidationError::InsufficientBalance);
    }

    let fee_fraction = get_fee_fraction(&e) as u128;

    // ----------  Step 1: dx_after_fee = ceil(x·dy / (y-dy))  ----------
    let dx_after_fee = reserve_sell.fixed_mul_ceil(e, &out_amount, &(reserve_buy - out_amount));

    // ----------  Step 2: gross-up for fee on *input* side  -------------
    // dx_before_fee = ceil( dx_after_fee / (1-f) )
    let dx_before_fee =
        dx_after_fee.fixed_mul_ceil(e, &FEE_MULTIPLIER, &(FEE_MULTIPLIER - fee_fraction));

    // ----------  Step 3: fee = dx_before_fee - dx_after_fee -----------
    let fee = dx_before_fee - dx_after_fee;

    (dx_before_fee, fee)
}
