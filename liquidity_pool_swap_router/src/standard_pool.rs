use crate::constants::FEE_MULTIPLIER;
use soroban_sdk::{Env, Vec, U256};

pub(crate) fn estimate_swap(
    e: &Env,
    fee_fraction: u128,
    reserves: Vec<u128>,
    in_idx: u32,
    out_idx: u32,
    in_amount: u128,
) -> u128 {
    let reserve_sell = reserves.get(in_idx).unwrap();
    let reserve_buy = reserves.get(out_idx).unwrap();

    // First calculate how much needs to be sold to buy amount out from the pool
    let multiplier_with_fee = FEE_MULTIPLIER - fee_fraction as u128;
    let n = U256::from_u128(&e, in_amount)
        .mul(&U256::from_u128(&e, reserve_buy))
        .mul(&U256::from_u128(&e, multiplier_with_fee));
    let d = (U256::from_u128(&e, reserve_sell).mul(&U256::from_u128(&e, FEE_MULTIPLIER)))
        .add(&(U256::from_u128(&e, in_amount).mul(&U256::from_u128(&e, multiplier_with_fee))));

    n.div(&d).to_u128().expect("math overflow")
}
