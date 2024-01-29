use soroban_sdk::{Env, Vec as SorobanVec};
// use crate::constants::{FEE_MULTIPLIER, PRICE_PRECISION};
// use num_bigint::BigUint;
// extern crate alloc;
// use alloc::vec::Vec;
// use crate::calculator::{get_next_in_amt, price_weight};
// use crate::utils::biguint_to_128;

// fn estimate_swap(
//     fee_fraction: &BigUint,
//     reserves: &Vec<BigUint>,
//     in_idx: u32,
//     out_idx: u32,
//     in_amount: &BigUint,
// ) -> BigUint {
//     let reserve_sell = reserves[in_idx as usize].clone();
//     let reserve_buy = reserves[out_idx as usize].clone();
//
//     // First calculate how much needs to be sold to buy amount out from the pool
//     let multiplier_with_fee = BigUint::from(FEE_MULTIPLIER) - fee_fraction;
//     let n = in_amount * reserve_buy * &multiplier_with_fee;
//     let d = reserve_sell * BigUint::from(FEE_MULTIPLIER) + in_amount * multiplier_with_fee;
//
//     n / d
// }

pub(crate) fn get_liquidity(
    fee_fraction: u128,
    reserves: &SorobanVec<u128>,
    in_idx: u32,
    out_idx: u32,
) -> u128 {
    if reserves.len() != 2 {
        panic!("liquidity calculation is allowed for 2 tokens only")
    }
    return 0;
    //
    // // pre-compiled frequent values
    // let zero = BigUint::from(0_u8);
    // let one = BigUint::from(1_u8);
    // let two = BigUint::from(2_u8);
    // let price_precision = BigUint::from(PRICE_PRECISION);
    //
    // let fee_fraction_big = BigUint::from(fee_fraction);
    // let reserve_in = BigUint::from(reserves.get(0).unwrap());
    // let reserve_out = BigUint::from(reserves.get(1).unwrap());
    //
    // if reserve_in == zero || reserve_out == zero {
    //     return 0;
    // }
    //
    // let mut result_big = zero.clone();
    // // let min_price_func = get_min_price(reserve_in, reserve_out, fee_fraction);
    // // let min_price = reserve_in * BigUint::from(PRICE_PRECISION) / reserve_out;
    //
    // let min_amount = price_precision.clone();
    // let mut reserves_adj = alloc::vec![];
    // let mut reserves_big = alloc::vec![];
    // for i in 0..reserves.len() {
    //     reserves_big.push(BigUint::from(reserves.get(i).unwrap()));
    //     reserves_adj.push(BigUint::from(reserves.get(i).unwrap()) * &price_precision);
    // }
    // let min_price = min_amount.clone() * &price_precision / estimate_swap(&fee_fraction_big, &reserves_adj, in_idx, out_idx, &min_amount);
    // let min_price_p8 = min_price.pow(8);
    //
    // let mut prev_price = zero.clone();
    // let mut prev_weight = one.clone();
    // let mut prev_depth = zero.clone();
    //
    // let mut first_iteration = true;
    // let mut last_iteration = false;
    // let mut in_amt: BigUint = reserve_in * &two;
    //
    // // todo: how to describe range properly?
    // while !last_iteration {
    //     let mut depth = estimate_swap(&fee_fraction_big, &reserves_big, in_idx, out_idx, &in_amt);
    //     let mut price = in_amt.clone() * &price_precision / depth.clone();
    //     let mut weight = price_weight(&price, &min_price_p8);
    //
    //     if first_iteration {
    //         prev_price = price.clone();
    //         prev_depth = depth.clone();
    //         prev_weight = weight.clone();
    //         first_iteration = false;
    //         continue;
    //     }
    //
    //     // stop if rounding affects price
    //     // then integrate up to min price
    //     if price > prev_price {
    //         // todo: do we need this case? don't go into last iteration since we've jumped below min price
    //         if prev_price < min_price {
    //             break;
    //         }
    //
    //         price = min_price.clone();
    //         weight = one.clone();
    //         depth = zero.clone();
    //         last_iteration = true;
    //     }
    //     // // if price has changed for less than 1%, skip iteration
    //     // else if &price * BigUint::from(101_u32) > &prev_price * BigUint::from(100_u32) {
    //     //     in_amt = get_next_in_amt(&in_amt);
    //     //     continue;
    //     // }
    //
    //     let depth_avg = (&depth + &prev_depth) / &two;
    //     let weight_avg = (&weight + &prev_weight) / &two;
    //     let d_price = &prev_price - &price;
    //     let integration_result = depth_avg * &price_precision * weight_avg / &price_precision * d_price / &price_precision;
    //
    //     result_big += integration_result;
    //
    //     prev_price = price.clone();
    //     prev_weight = weight.clone();
    //     prev_depth = depth.clone();
    //     in_amt = get_next_in_amt(&in_amt);
    // }
    // biguint_to_128(result_big / &price_precision)
}
