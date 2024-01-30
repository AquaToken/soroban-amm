use crate::calculator::{get_max_reserve, get_next_in_amt, normalize_reserves, price_weight};
use crate::constants::{FEE_MULTIPLIER, PRICE_PRECISION};
use soroban_sdk::{Env, Vec};

fn estimate_swap(
    _e: &Env,
    fee_fraction: u128,
    reserves: &Vec<u128>,
    in_idx: u32,
    out_idx: u32,
    in_amount: u128,
) -> u128 {
    let reserve_sell = reserves.get(in_idx).unwrap();
    let reserve_buy = reserves.get(out_idx).unwrap();

    // First calculate how much needs to be sold to buy amount out from the pool
    let multiplier_with_fee = FEE_MULTIPLIER - fee_fraction;
    let n = in_amount * reserve_buy * multiplier_with_fee;
    let d = reserve_sell * FEE_MULTIPLIER + in_amount * multiplier_with_fee;

    n / d
}

fn get_min_price(
    e: &Env,
    fee_fraction: u128,
    reserves: Vec<u128>,
    in_idx: u32,
    out_idx: u32,
) -> u128 {
    let min_amount = PRICE_PRECISION;
    let mut reserves_adj = Vec::new(e);
    for i in 0..reserves.len() {
        let value = reserves.get(i).unwrap();
        reserves_adj.push_back(value * PRICE_PRECISION);
    }

    &min_amount * PRICE_PRECISION
        / estimate_swap(e, fee_fraction, &reserves_adj, in_idx, out_idx, min_amount)
}

pub(crate) fn get_liquidity(
    e: &Env,
    fee_fraction: u128,
    reserves: &Vec<u128>,
    in_idx: u32,
    out_idx: u32,
) -> u128 {
    let reserve_in = reserves.get(0).unwrap();
    let reserve_out = reserves.get(1).unwrap();

    if reserve_in == 0 || reserve_out == 0 {
        return 0;
    }

    let (reserves_norm, nominator, denominator) = normalize_reserves(reserves);
    let min_price = get_min_price(e, fee_fraction, reserves_norm.clone(), in_idx, out_idx);

    let mut result_big = 0;
    let mut prev_price = 0;
    let mut prev_weight = 1;
    let mut prev_depth = 0;

    let mut first_iteration = true;
    let mut last_iteration = false;
    let mut in_amt = get_max_reserve(&reserves_norm) * 2;

    // todo: how to describe range properly?
    while !last_iteration {
        let mut depth = estimate_swap(e, fee_fraction, &reserves_norm, in_idx, out_idx, in_amt);
        let mut price = in_amt * PRICE_PRECISION / depth;
        let mut weight = price_weight(price, min_price);

        if first_iteration {
            prev_price = price.clone();
            prev_depth = depth.clone();
            prev_weight = weight.clone();
            first_iteration = false;
            continue;
        }

        // stop if rounding affects price
        // stop if steps are too small
        //  then integrate up to min price
        // todo: add exit condition on iterations amount
        if (price > prev_price) || (in_amt < 50_000) {
            // todo: do we need this case? don't go into last iteration since we've jumped below min price
            if prev_price < min_price {
                break;
            }

            price = min_price;
            weight = 1;
            depth = 0;
            last_iteration = true;
        }
        // // if price has changed for less than 0.01%, skip iteration
        // else if &price * U256M::from(10001_u32) > &prev_price * U256M::from(10000_u32) {
        //     in_amt = get_next_in_amt(&in_amt);
        //     continue;
        // }

        let depth_avg = (&depth + &prev_depth) / 2;
        let weight_avg = (&weight + &prev_weight) / 2;
        let d_price = &prev_price - &price;
        let integration_result =
            depth_avg * PRICE_PRECISION * weight_avg / PRICE_PRECISION * d_price / PRICE_PRECISION;

        result_big = result_big + integration_result;

        prev_price = price.clone();
        prev_weight = weight.clone();
        prev_depth = depth.clone();
        // let in_amt_prev = biguint_to_128(in_amt.clone());
        in_amt = get_next_in_amt(in_amt);
    }
    result_big / PRICE_PRECISION * nominator / denominator
}