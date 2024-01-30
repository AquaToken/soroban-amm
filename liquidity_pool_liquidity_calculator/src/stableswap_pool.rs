use crate::calculator::{get_max_reserve, get_next_in_amt, normalize_reserves, price_weight};
use crate::constants::{FEE_MULTIPLIER, PRECISION, PRICE_PRECISION};
use soroban_sdk::{Env, Vec};

const RATE: u128 = 1_0000000;

fn a(e: &Env, initial_a: u128, initial_a_time: u128, future_a: u128, future_a_time: u128) -> u128 {
    // Handle ramping A up or down
    let t1 = future_a_time;
    let a1 = future_a;
    let now = e.ledger().timestamp() as u128;

    if now < t1 {
        let a0 = initial_a;
        let t0 = initial_a_time;
        // Expressions in u128 cannot have negative numbers, thus "if"
        if a1 > a0 {
            a0 + (a1 - a0) * (now - t0) / (t1 - t0)
        } else {
            a0 - (a0 - a1) * (now - t0) / (t1 - t0)
        }
    } else {
        // when t1 == 0 or block.timestamp >= t1
        a1
    }
}

// xp size = N_COINS
fn get_d(n_coins: u32, xp: Vec<u128>, amp: u128) -> u128 {
    let mut s = 0;
    for x in xp.clone() {
        s += x;
    }
    if s == 0 {
        return 0;
    }

    let mut d_prev;
    let mut d = s;
    let ann = amp * n_coins as u128;
    for _i in 0..255 {
        let mut d_p = d;
        for x1 in xp.clone() {
            d_p = d_p * d / (x1 * n_coins as u128) // If division by 0, this will be borked: only withdrawal will work. And that is good
        }
        d_prev = d;
        d = (ann * s + d_p * n_coins as u128) * d / ((ann - 1) * d + (n_coins as u128 + 1) * d_p);
        // // Equality with the precision of 1
        if d > d_prev {
            if d - d_prev <= 1 {
                break;
            }
        } else if d_prev - d <= 1 {
            break;
        }
    }
    d
}

fn get_y(n_coins: u32, in_idx: u32, out_idx: u32, x: u128, xp: Vec<u128>, a: u128) -> u128 {
    // x in the input is converted to the same price/precision

    if in_idx == out_idx {
        panic!("same coin")
    } // dev: same coin
      // if !(j >= 0) {
      //     panic!("j below zero")
      // } // dev: j below zero
    if out_idx >= n_coins {
        panic!("j above N_COINS")
    } // dev: j above N_COINS

    // should be unreachable, but good for safety
    // if !(i >= 0) {
    //     panic!("bad arguments")
    // }
    if in_idx >= n_coins {
        panic!("bad arguments")
    }

    let amp = a;
    let d = get_d(xp.len(), xp.clone(), amp);
    let mut c = d;
    let mut s = 0;
    let ann = amp * n_coins as u128;

    let mut x1;
    for i in 0..n_coins {
        if i == in_idx {
            x1 = x;
        } else if i != out_idx {
            x1 = xp.get(i).unwrap();
        } else {
            continue;
        }
        s += x1;
        c = c * d / (x1 * n_coins as u128);
    }
    c = c * d / (ann * n_coins as u128);
    let b = s + d / ann; // - D
    let mut y_prev;
    let mut y = d;
    for _i in 0..255 {
        y_prev = y;
        y = (y * y + c) / (2 * y + b - d);
        // Equality with the precision of 1
        if y > y_prev {
            if y - y_prev <= 1 {
                break;
            }
        } else if y_prev - y <= 1 {
            break;
        }
    }
    y
}

fn get_dy(reserves: &Vec<u128>, fee_fraction: u128, a: u128, i: u32, j: u32, dx: u128) -> u128 {
    // dx and dy in c-units
    let xp = reserves.clone();

    let x = xp.get(i).unwrap() + (dx * RATE / PRECISION);
    let y = get_y(reserves.len(), i, j, x, xp.clone(), a);

    if y == 0 {
        // pool is empty
        return 0;
    }

    let dy = (xp.get(j).unwrap() - y - 1) * PRECISION / RATE;
    let fee = fee_fraction * dy / FEE_MULTIPLIER as u128;
    dy - fee
}

fn estimate_swap(
    e: &Env,
    fee_fraction: u128,
    initial_a: u128,
    initial_a_time: u128,
    future_a: u128,
    future_a_time: u128,
    reserves: &Vec<u128>,
    in_idx: u32,
    out_idx: u32,
    in_amount: u128,
) -> u128 {
    let a = a(e, initial_a, initial_a_time, future_a, future_a_time);
    get_dy(reserves, fee_fraction, a, in_idx, out_idx, in_amount)
}

pub(crate) fn get_liquidity(
    e: &Env,
    fee_fraction: u128,
    initial_a: u128,
    initial_a_time: u128,
    future_a: u128,
    future_a_time: u128,
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
        / estimate_swap(
            e,
            fee_fraction,
            initial_a,
            initial_a_time,
            future_a,
            future_a_time,
            &reserves_adj,
            in_idx,
            out_idx,
            min_amount,
        );
    // let _min_price_p8 = min_price.pow(8);

    let mut prev_price = 0;
    let mut prev_weight = 1;
    let mut prev_depth = 0;

    let mut first_iteration = true;
    let mut last_iteration = false;
    let mut in_amt = get_max_reserve(&reserves_norm) * 2;

    // todo: how to describe range properly?
    while !last_iteration {
        let mut depth = estimate_swap(
            e,
            fee_fraction,
            initial_a,
            initial_a_time,
            future_a,
            future_a_time,
            &reserves_big,
            in_idx,
            out_idx,
            in_amt,
        );
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
