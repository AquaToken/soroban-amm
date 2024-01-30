use crate::calculator::{get_next_in_amt, price_weight};
use crate::constants::{FEE_MULTIPLIER, PRICE_PRECISION};
use crate::u256::U256M;
use soroban_sdk::{Env, Vec, U256};

fn estimate_swap(
    e: &Env,
    fee_fraction: &U256M,
    reserves: &Vec<U256>,
    in_idx: u32,
    out_idx: u32,
    in_amount: &U256M,
) -> U256M {
    let reserve_sell = U256M::from_u256(e, reserves.get(in_idx).unwrap());
    let reserve_buy = U256M::from_u256(e, reserves.get(out_idx).unwrap());

    // First calculate how much needs to be sold to buy amount out from the pool
    let multiplier_with_fee = U256M::from_u128(e, FEE_MULTIPLIER) - fee_fraction;
    let n = in_amount * reserve_buy * &multiplier_with_fee;
    let d = reserve_sell * U256M::from_u128(e, FEE_MULTIPLIER) + in_amount * multiplier_with_fee;

    n / d
}

pub(crate) fn get_liquidity(
    e: &Env,
    fee_fraction: u128,
    reserves: &Vec<u128>,
    in_idx: u32,
    out_idx: u32,
) -> u128 {
    if reserves.len() != 2 {
        panic!("liquidity calculation is allowed for 2 tokens only")
    }

    // pre-compiled frequent values
    let zero = U256M::from_u32(e, 0);
    let one = U256M::from_u32(e, 1);
    let two = U256M::from_u32(e, 2);
    let price_precision = U256M::from_u128(e, PRICE_PRECISION);

    let fee_fraction_big = U256M::from_u128(e, fee_fraction);
    let reserve_in = U256M::from_u128(e, reserves.get(0).unwrap());
    let reserve_out = U256M::from_u128(e, reserves.get(1).unwrap());

    if reserve_in.v == zero.v || reserve_out.v == zero.v {
        return 0;
    }

    let mut result_big = zero.clone();
    // let min_price_func = get_min_price(reserve_in, reserve_out, fee_fraction);
    // let min_price = reserve_in * U256M::from_u128(e, PRICE_PRECISION) / reserve_out;

    let min_amount = price_precision.clone();
    let mut reserves_adj = Vec::new(e);
    let mut reserves_big = Vec::new(e);
    for i in 0..reserves.len() {
        reserves_big.push_back(U256::from_u128(e, reserves.get(i).unwrap()));
        reserves_adj.push_back(
            U256::from_u128(e, reserves.get(i).unwrap()).mul(&U256::from_u128(e, PRICE_PRECISION)),
        );
    }
    let min_price = min_amount.clone() * &price_precision
        / estimate_swap(
            e,
            &fee_fraction_big,
            &reserves_adj,
            in_idx,
            out_idx,
            &min_amount,
        );
    let min_price_p8 = min_price.pow(8);

    let mut prev_price = zero.clone();
    let mut prev_weight = one.clone();
    let mut prev_depth = zero.clone();

    let mut first_iteration = true;
    let mut last_iteration = false;
    let mut in_amt: U256M = reserve_in * &two;

    // todo: how to describe range properly?
    while !last_iteration {
        let mut depth = estimate_swap(
            e,
            &fee_fraction_big,
            &reserves_big,
            in_idx,
            out_idx,
            &in_amt,
        );
        let mut price = in_amt.clone() * &price_precision / depth.clone();
        let mut weight = price_weight(e, &price, &min_price_p8);

        if first_iteration {
            prev_price = price.clone();
            prev_depth = depth.clone();
            prev_weight = weight.clone();
            first_iteration = false;
            continue;
        }

        // stop if rounding affects price
        // then integrate up to min price
        if price.v > prev_price.v {
            // todo: do we need this case? don't go into last iteration since we've jumped below min price
            if prev_price.v < min_price.v {
                break;
            }

            price = min_price.clone();
            weight = one.clone();
            depth = zero.clone();
            last_iteration = true;
        }
        // if price has changed for less than 1%, skip iteration
        else if (&price * U256M::from_u32(e, 101)).v > (&prev_price * U256M::from_u32(e, 100)).v {
            in_amt = get_next_in_amt(e, &in_amt);
            continue;
        }

        let depth_avg = (&depth + &prev_depth) / &two;
        let weight_avg = (&weight + &prev_weight) / &two;
        let d_price = &prev_price - &price;
        let integration_result = depth_avg * &price_precision * weight_avg / &price_precision
            * d_price
            / &price_precision;

        result_big = result_big + integration_result;

        prev_price = price.clone();
        prev_weight = weight.clone();
        prev_depth = depth.clone();
        in_amt = get_next_in_amt(e, &in_amt);
    }
    (result_big / &price_precision).to_u128().unwrap()
}
