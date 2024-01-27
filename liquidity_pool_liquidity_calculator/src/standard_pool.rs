use soroban_sdk::Vec;
use crate::constants::{FEE_MULTIPLIER, PRICE_PRECISION};

pub(crate) fn estimate_swap(
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

fn price_weight(price: u128, min_price: u128) -> u128 {
    // returns price weighted with exponent (p_min/p)^8
    let mut result = PRICE_PRECISION * min_price / price;
    for _i in 1..8 {
        result = result * min_price / price;
    }
    result
}

pub(crate) fn get_liquidity(reserves: &Vec<u128>, in_idx: u32, out_idx: u32, fee_fraction: u128) -> u128 {
    if reserves.len() != 2 {
        panic!("liquidity calculation is allowed for 2 tokens only")
    }

    let reserve_in = reserves.get(0).unwrap();
    let reserve_out = reserves.get(1).unwrap();

    if reserve_in == 0 || reserve_out == 0 {
        return 0;
    }

    let mut result = 0;

    let min_amount = PRICE_PRECISION;
    let mut reserves_adj = reserves.clone();
    for i in 0..reserves.len() {
        reserves_adj.set(i, reserves_adj.get(i).unwrap() * PRICE_PRECISION);
    }
    let min_price = min_amount * PRICE_PRECISION / estimate_swap(fee_fraction, &reserves_adj, in_idx, out_idx, min_amount);

    let mut prev_price = 0;
    let mut prev_weight = 1;
    let mut prev_depth = 0;

    let mut first_iteration = true;
    let mut last_iteration = false;
    let mut in_amt = reserve_in * 2;

    // todo: how to describe range properly?
    while !last_iteration {
        let mut depth = estimate_swap(fee_fraction, &reserves, in_idx, out_idx, in_amt);
        let mut price = in_amt * PRICE_PRECISION / depth;
        let mut weight = price_weight(price, min_price);

        if first_iteration {
            prev_price = price;
            prev_depth = depth;
            prev_weight = weight;
            first_iteration = false;
            continue;
        }

        // stop if rounding affects price
        // then integrate up to min price
        if price > prev_price {
            price = min_price;
            weight = 1;
            depth = 0;
            last_iteration = true;
        }

        let depth_avg = (depth + prev_depth) / 2;
        let weight_avg = (weight + prev_weight) / 2;
        let d_price = prev_price - price;
        let integration_result = depth_avg * PRICE_PRECISION * weight_avg / PRICE_PRECISION * d_price / PRICE_PRECISION;

        result += integration_result;

        prev_price = price;
        prev_weight = weight;
        prev_depth = depth;
        // decrease dx exponentially
        in_amt = in_amt * 100 / 110;
    }
    result / PRICE_PRECISION
}
