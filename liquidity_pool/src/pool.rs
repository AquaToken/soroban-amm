use liquidity_pool_validation_errors::LiquidityPoolValidationError;
use soroban_sdk::{panic_with_error, Env};

use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::Env;

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

    let amount_b = desired_a.fixed_mul_floor(e, reserve_b, reserve_a);
    if amount_b <= desired_b {
        if amount_b < min_b {
            panic_with_error!(e, LiquidityPoolValidationError::InvalidDepositAmount);
        }
        (desired_a, amount_b)
    } else {
        let amount_a = desired_b.fixed_mul_floor(&e, reserve_a, reserve_b);
        if amount_a > desired_a || desired_a < min_a {
            panic_with_error!(e, LiquidityPoolValidationError::InvalidDepositAmount);
        }
        (amount_a, desired_b)
    }
}
