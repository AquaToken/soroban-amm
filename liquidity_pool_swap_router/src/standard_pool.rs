use crate::constants::FEE_MULTIPLIER;
use soroban_sdk::Vec;

pub(crate) fn estimate_swap(
    reserves: Vec<u128>,
    fee_fraction: u128,
    in_idx: u32,
    out_idx: u32,
    in_amount: u128,
) -> u128 {
    if in_idx == out_idx {
        panic!("cannot swap token to same one")
    }

    if in_idx > 1 {
        panic!("in_idx out of bounds");
    }

    if out_idx > 1 {
        panic!("out_idx out of bounds");
    }

    let reserve_sell = reserves.get(in_idx).unwrap();
    let reserve_buy = reserves.get(out_idx).unwrap();

    // First calculate how much needs to be sold to buy amount out from the pool
    let multiplier_with_fee = FEE_MULTIPLIER - fee_fraction as u128;
    let n = in_amount * reserve_buy * multiplier_with_fee;
    let d = reserve_sell * FEE_MULTIPLIER + in_amount * multiplier_with_fee;
    let out = n / d;
    out
}
