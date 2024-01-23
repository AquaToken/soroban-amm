use crate::constants::FEE_MULTIPLIER;
use soroban_sdk::{Env, Vec};

pub(crate) fn estimate_swap(
    _e: &Env,
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
    let n = in_amount * reserve_buy * multiplier_with_fee;
    let d = reserve_sell * FEE_MULTIPLIER + in_amount * multiplier_with_fee;
    let out = n / d;
    out
}
