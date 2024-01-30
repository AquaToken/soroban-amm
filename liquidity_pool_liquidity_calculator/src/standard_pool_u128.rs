use crate::constants::{FEE_MULTIPLIER, PRICE_PRECISION};
use soroban_sdk::{Env, Vec};

const RATE: u128 = 1_0000000;
const PRECISION: u128 = 1_0000000;
const RESERVES_NORM: u128 = 1_000_0000000_u128;

fn estimate_swap(
    e: &Env,
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

fn get_next_in_amt(in_amt: u128) -> u128 {
    // decrease dx exponentially
    // in_amt * U256M::from(100_u8) / U256M::from(110_u8)
    // in_amt * 100 / 125
    in_amt * 100 / 110
    // in_amt * U256M::from(50_u8) / U256M::from(100_u8)
}

fn get_max_reserve(reserves: &Vec<u128>) -> u128 {
    let mut max_reserve = 0;
    for value in reserves.clone() {
        if max_reserve < value {
            max_reserve = value;
        }
    }
    max_reserve
}

fn normalize_reserves(reserves: &Vec<u128>) -> (Vec<u128>, u128, u128) {
    let mut reserves_norm = reserves.clone();
    let mut max_reserve = get_max_reserve(reserves);

    // normalize reserves
    let mut nominator = 1;
    let mut denominator = 1;
    if max_reserve > RESERVES_NORM * 2 {
        nominator = max_reserve / RESERVES_NORM;
        for i in 0..reserves_norm.len() {
            let value = reserves_norm.get(i).unwrap();
            let adj_value = value / nominator;
            reserves_norm.set(i, adj_value);
        }
    } else if max_reserve < RESERVES_NORM / 2 {
        denominator = RESERVES_NORM / max_reserve;
        for i in 0..reserves_norm.len() {
            let value = reserves_norm.get(i).unwrap();
            let adj_value = value * denominator;
            reserves_norm.set(i, adj_value);
        }
    }
    (reserves_norm, nominator, denominator)
}

pub(crate) fn get_liquidity(
    e: &Env,
    fee_fraction: u128,
    reserves: &Vec<u128>,
    in_idx: u32,
    out_idx: u32,
) -> u128 {
    let mut reserve_in = reserves.get(0).unwrap();
    let reserve_out = reserves.get(1).unwrap();

    if reserve_in == 0 || reserve_out == 0 {
        return 0;
    }

    let (reserves_norm, nominator, denominator) = normalize_reserves(reserves);
    reserve_in = reserve_in / nominator * denominator;

    let mut result_big = 0;
    // let min_price_func = get_min_price(reserve_in, reserve_out, fee_fraction);
    // let min_price = reserve_in * U256M::from_u128(e, PRICE_PRECISION) / reserve_out;
    let min_amount = PRICE_PRECISION;
    let mut reserves_adj = Vec::new(e);
    let mut reserves_big = Vec::new(e);
    for i in 0..reserves_norm.len() {
        let value = reserves_norm.get(i).unwrap();
        reserves_big.push_back(value);
        reserves_adj.push_back(value * PRICE_PRECISION);
    }

    let min_price = &min_amount * PRICE_PRECISION
        / estimate_swap(e, fee_fraction, &reserves_adj, in_idx, out_idx, min_amount);
    // let _min_price_p8 = min_price.pow(8);

    let mut prev_price = 0;
    let mut prev_weight = 1;
    let mut prev_depth = 0;

    let mut first_iteration = true;
    let mut last_iteration = false;
    let mut in_amt = get_max_reserve(&reserves_norm) * 2;

    // todo: how to describe range properly?
    let mut i = 0;
    while !last_iteration {
        i += 1;
        let mut depth = estimate_swap(e, fee_fraction, &reserves_big, in_idx, out_idx, in_amt);
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
        // then integrate up to min price
        // todo: add exit condition on iterations amount
        if price > prev_price {
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
