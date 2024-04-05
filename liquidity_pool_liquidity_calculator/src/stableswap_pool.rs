use crate::calculator::{get_max_reserve, get_next_in_amt, normalize_reserves, price_weight};
use crate::constants::{FEE_MULTIPLIER, PRECISION};
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::{Env, Vec};

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
            a0 + (a1 - a0).fixed_mul_floor(&e, now - t0, t1 - t0)
        } else {
            a0 - (a0 - a1).fixed_mul_floor(&e, now - t0, t1 - t0)
        }
    } else {
        // when t1 == 0 or block.timestamp >= t1
        a1
    }
}

// xp size = N_COINS
fn get_d(e: &Env, n_coins: u32, xp: &Vec<u128>, amp: u128) -> u128 {
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
        let mut d_p = d.clone();
        for x1 in xp.clone() {
            d_p = d_p.fixed_mul_floor(&e, d, x1 * n_coins as u128);
        }
        d_prev = d.clone();
        d = (ann * s + d_p * n_coins as u128).fixed_mul_floor(
            &e,
            d,
            (ann - 1) * d + (n_coins as u128 + 1) * d_p,
        );

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

fn get_y(
    e: &Env,
    d: u128,
    n_coins: u32,
    in_idx: u32,
    out_idx: u32,
    x: u128,
    xp: Vec<u128>,
    amp: u128,
) -> u128 {
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
        c = c.fixed_mul_floor(e, d, x1 * n_coins as u128);
    }
    c = c.fixed_mul_floor(e, d, ann * n_coins as u128);
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

fn get_dy(
    e: &Env,
    d: u128,
    reserves: &Vec<u128>,
    fee_fraction: u128,
    amp: u128,
    i: u32,
    j: u32,
    dx: u128,
) -> u128 {
    // dx and dy in c-units
    let xp = reserves.clone();

    let x = xp.get(i).unwrap() + dx;
    let y = get_y(e, d, reserves.len(), i, j, x, xp.clone(), amp);

    if y == 0 {
        // pool is empty
        return 0;
    }

    let dy = xp.get(j).unwrap() - y - 1;
    // The `fixed_mul_ceil` function is used to perform the multiplication
    //  to ensure user cannot exploit rounding errors.
    let fee = fee_fraction.fixed_mul_ceil(&e, dy, FEE_MULTIPLIER);
    dy - fee
}

fn estimate_swap(
    e: &Env,
    fee_fraction: u128,
    d: u128,
    amp: u128,
    reserves: &Vec<u128>,
    in_idx: u32,
    out_idx: u32,
    in_amount: u128,
) -> u128 {
    get_dy(
        e,
        d,
        reserves,
        fee_fraction,
        amp,
        in_idx,
        out_idx,
        in_amount,
    )
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
    let min_amount = PRECISION;
    let mut reserves_adj = Vec::new(e);
    let mut reserves_big = Vec::new(e);
    for i in 0..reserves_norm.len() {
        let value = reserves_norm.get(i).unwrap();
        reserves_big.push_back(value);
        reserves_adj.push_back(value * PRECISION);
    }

    let amp = a(e, initial_a, initial_a_time, future_a, future_a_time);
    let n_tokens = reserves_adj.len();
    let d_adj = get_d(e, n_tokens, &reserves_adj, amp);
    let d = get_d(e, n_tokens, &reserves_big, amp);
    let min_price = min_amount * PRECISION
        / estimate_swap(
            e,
            fee_fraction,
            d_adj,
            amp,
            &reserves_adj,
            in_idx,
            out_idx,
            min_amount,
        );

    let mut prev_price = 0;
    let mut prev_weight = 1;
    let mut prev_depth = 0;

    let mut first_iteration = true;
    let mut last_iteration = false;

    // euristic. 2x is because of weight function - after 1.6 it affects less than 1%
    let mut in_amt = get_max_reserve(&reserves_norm) * 2;

    while !last_iteration {
        let mut depth = estimate_swap(
            e,
            fee_fraction,
            d,
            amp,
            &reserves_big,
            in_idx,
            out_idx,
            in_amt,
        );
        let mut price = in_amt * PRECISION / depth;
        let mut weight = price_weight(price, min_price);

        if first_iteration {
            prev_price = price;
            prev_depth = depth;
            prev_weight = weight;
            first_iteration = false;
            continue;
        }

        // stop if rounding affects price
        // stop if steps are too small
        //  then integrate up to min price
        if price > prev_price {
            // don't go into last iteration since we've already jumped below min price
            if prev_price < min_price {
                break;
            }

            price = min_price;
            weight = 1;
            depth = 0;
            last_iteration = true;
        }

        let depth_avg = (depth + prev_depth) / 2;
        let weight_avg = (weight + prev_weight) / 2;
        let d_price = prev_price - price;
        let integration_result =
            depth_avg * PRECISION * weight_avg / PRECISION * d_price / PRECISION;

        result_big += integration_result;

        prev_price = price;
        prev_weight = weight;
        prev_depth = depth;
        in_amt = get_next_in_amt(in_amt);
    }
    result_big / PRECISION * nominator / denominator
}
