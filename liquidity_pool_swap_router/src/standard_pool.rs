use crate::constants::FEE_MULTIPLIER;
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::{Env, Vec};

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

    let result = in_amount.fixed_mul_floor(&e, reserve_buy, reserve_sell + in_amount);
    let fee = result.fixed_mul_ceil(&e, fee_fraction, FEE_MULTIPLIER);
    Some(result - fee)
}
