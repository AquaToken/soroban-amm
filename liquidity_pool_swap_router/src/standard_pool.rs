use crate::constants::FEE_MULTIPLIER;
use soroban_sdk::{panic_with_error, Env, Vec, U256};
use utils::math_errors::MathError;

pub(crate) fn estimate_swap(
    e: &Env,
    fee_fraction: u128,
    reserves: Vec<u128>,
    in_idx: u32,
    out_idx: u32,
    in_amount: u128,
) -> Option<u128> {
    for i in 0..reserves.len() {
        if reserves.get(i).unwrap() == 0 {
            return None;
        }
    }

    let reserve_sell = reserves.get(in_idx).unwrap();
    let reserve_buy = reserves.get(out_idx).unwrap();

    // First calculate how much needs to be sold to buy amount out from the pool
    let multiplier_with_fee = FEE_MULTIPLIER - fee_fraction as u128;
    let n = U256::from_u128(&e, in_amount)
        .mul(&U256::from_u128(&e, reserve_buy))
        .mul(&U256::from_u128(&e, multiplier_with_fee));
    let d = (U256::from_u128(&e, reserve_sell).mul(&U256::from_u128(&e, FEE_MULTIPLIER)))
        .add(&(U256::from_u128(&e, in_amount).mul(&U256::from_u128(&e, multiplier_with_fee))));

    match n.div(&d).to_u128() {
        Some(v) => Some(v),
        None => panic_with_error!(&e, MathError::NumberOverflow),
    }
}
