use soroban_sdk::{Env, U256, Vec};
use crate::calculator::{get_next_in_amt, price_weight};
use crate::constants::{FEE_MULTIPLIER, PRICE_PRECISION};
use crate::u256::U256M;
// use crate::utils::biguint_to_128;

const RATE: u128 = 1_0000000_u128;
const PRECISION: u128 = 1_0000000_u128;


fn a(e: &Env, initial_a: &U256M, initial_a_time: &U256M, future_a: &U256M, future_a_time: &U256M) -> U256M {
    // Handle ramping A up or down
    let t1 = future_a_time;
    let a1 = future_a;
    let now = &U256M::from_u128(e, e.ledger().timestamp().into());

    if now.v < t1.v {
        let a0 = initial_a;
        let t0 = initial_a_time;
        // Expressions in u128 cannot have negative numbers, thus "if"
        if a1.v > a0.v {
            a0 + &((a1 - a0) * (now - t0) / (t1 - t0))
        } else {
            a0 - &((a0 - a1) * (now - t0) / (t1 - t0))
        }
    } else {
        // when t1 == 0 or block.timestamp >= t1
        a1.clone()
    }
}

// xp size = N_COINS
fn get_d(e: &Env, n_coins: u32, xp: &Vec<U256>, amp: &U256M) -> U256M {
    let mut s = U256M::from_u32(e, 0);
    for x in xp.clone() {
        s = s + U256M::from_u256(e, x);
    }
    if s.v == U256M::from_u32(e, 0).v {
        return U256M::from_u32(e, 0);
    }

    let mut d_prev: U256M;
    let mut d = s.clone();
    let ann = amp * &U256M::from_u32(e, n_coins);
    for _i in 0..255 {
        let mut d_p = d.clone();
        for x1 in xp.clone() {
            d_p = &d_p * &d / (U256M::from_u256(e, x1) * &U256M::from_u32(e, n_coins)) // If division by 0, this will be borked: only withdrawal will work. And that is good
        }
        d_prev = d.clone();
        d = (&ann * &s + &d_p * &U256M::from_u32(e, n_coins)) * &d / ((&ann - &U256M::from_u32(e, 1)) * &d + U256M::from_u32(e, n_coins + 1) * &d_p);
        // // Equality with the precision of 1
        if d.v > d_prev.v {
            if (&d - d_prev).v <= U256M::from_u32(e, 1).v {
                break;
            }
        } else if (d_prev - &d).v <= U256M::from_u32(e, 1).v {
            break;
        }
    }
    d
}

fn get_y(e: &Env, n_coins: u32, in_idx: u32, out_idx: u32, x: &U256M, xp: &Vec<U256>, a: &U256M) -> U256M {
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
    let d = get_d(e, xp.len(), xp, amp);
    let mut c = d.clone();
    let mut s = U256M::from_u32(e, 0);
    let ann = amp * U256M::from_u32(e, n_coins);

    let mut x1;
    for i in 0..n_coins {
        if i == in_idx {
            x1 = x.clone();
        } else if i != out_idx {
            x1 = U256M::from_u256(e, xp.get(i).unwrap());
        } else {
            continue;
        }
        s = s + &x1;
        c = &c * &d / (&x1 * U256M::from_u32(e, n_coins));
    }
    c = &c * &d / (&ann * U256M::from_u32(e, n_coins));
    let b = s + &d / &ann; // - D
    let mut y_prev;
    let mut y = d.clone();
    for _i in 0..255 {
        y_prev = y.clone();
        y = (&y * &y + &c) / (U256M::from_u32(e, 2) * &y + &b - &d);
        // Equality with the precision of 1
        if y.v > y_prev.v {
            if (&y - y_prev).v <= U256M::from_u32(e, 1).v {
                break;
            }
        } else if (y_prev - &y).v <= U256M::from_u32(e, 1).v {
            break;
        }
    }
    y
}

fn get_dy(e: &Env, reserves: &Vec<U256>, fee_fraction: &U256M, a: &U256M, i: u32, j: u32, dx: &U256M) -> U256M {
    // dx and dy in c-units
    let xp = reserves;

    let x = U256M::from_u256(e, xp.get(i).unwrap()) + (dx * U256M::from_u128(e, RATE) / U256M::from_u128(e, PRECISION));
    let y = get_y(e, reserves.len(), i, j, &x, &xp, a);

    if y.v == U256M::from_u32(e, 0).v {
        // pool is empty
        return U256M::from_u32(e, 0);
    }

    let dy = (U256M::from_u256(e, xp.get(j).unwrap()) - y - U256M::from_u32(e, 1)) * U256M::from_u128(e, PRECISION) / U256M::from_u128(e, RATE);
    let fee = fee_fraction * &dy / U256M::from_u128(e, FEE_MULTIPLIER);
    dy - fee
}

fn estimate_swap(
    e: &Env,
    fee_fraction: &U256M,
    initial_a: &U256M,
    initial_a_time: &U256M,
    future_a: &U256M,
    future_a_time: &U256M,
    reserves: &Vec<U256>,
    in_idx: u32,
    out_idx: u32,
    in_amount: &U256M,
) -> U256M {
    let a = a(e, initial_a, initial_a_time, future_a, future_a_time);
    get_dy(e, reserves, fee_fraction, &a, in_idx, out_idx, in_amount)
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
    let fee_fraction_big = U256M::from_u128(e, fee_fraction);
    let initial_a_big = U256M::from_u128(e, initial_a);
    let initial_a_time_big = U256M::from_u128(e, initial_a_time);
    let future_a_big = U256M::from_u128(e, future_a);
    let future_a_time_big = U256M::from_u128(e, future_a_time);
    let reserve_in = U256M::from_u128(e, reserves.get(0).unwrap());
    let reserve_out = U256M::from_u128(e, reserves.get(1).unwrap());

    if reserve_in.v == U256M::from_u32(e, 0).v || reserve_out.v == U256M::from_u32(e, 0).v {
        return 0;
    }

    let mut result_big = U256M::from_u32(e, 0);
    // let min_price_func = get_min_price(reserve_in, reserve_out, fee_fraction);
    // let min_price = reserve_in * U256M::from_u128(e, PRICE_PRECISION) / reserve_out;

    let min_amount = U256M::from_u128(e, PRICE_PRECISION);
    let mut reserves_adj = Vec::new(e);
    let mut reserves_big = Vec::new(e);
    for i in 0..reserves.len() {
        reserves_big.push_back(U256::from_u128(e, reserves.get(i).unwrap()));
        reserves_adj.push_back(U256::from_u128(e, reserves.get(i).unwrap()).mul(&U256::from_u128(e, PRICE_PRECISION)));
    }
    let min_price = &min_amount * U256M::from_u128(e, PRICE_PRECISION) / estimate_swap(e, &fee_fraction_big, &initial_a_big, &initial_a_time_big, &future_a_big, &future_a_time_big, &reserves_adj, in_idx, out_idx, &min_amount);
    let min_price_p8 = min_price.pow(8);

    let mut prev_price = U256M::from_u32(e, 0);
    let mut prev_weight = U256M::from_u32(e, 1);
    let mut prev_depth = U256M::from_u32(e, 0);

    let mut first_iteration = true;
    let mut last_iteration = false;
    let mut in_amt: U256M = reserve_in * U256M::from_u32(e, 2);

    // todo: how to describe range properly?
    let mut i = 0;
    while !last_iteration {
        i += 1;
        let mut depth = estimate_swap(e, &fee_fraction_big, &initial_a_big, &initial_a_time_big, &future_a_big, &future_a_time_big, &reserves_big, in_idx, out_idx, &in_amt);
        let mut price = &in_amt * U256M::from_u128(e, PRICE_PRECISION) / &depth;
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
        // todo: add exit condition on iterations amount
        if price.v > prev_price.v {
            // todo: do we need this case? don't go into last iteration since we've jumped below min price
            if prev_price.v < min_price.v {
                break;
            }

            price = min_price.clone();
            weight = U256M::from_u32(e, 1);
            depth = U256M::from_u32(e, 0);
            last_iteration = true;
        }
        // // if price has changed for less than 0.01%, skip iteration
        // else if &price * U256M::from(10001_u32) > &prev_price * U256M::from(10000_u32) {
        //     in_amt = get_next_in_amt(&in_amt);
        //     continue;
        // }

        let depth_avg = (&depth + &prev_depth) / U256M::from_u32(e, 2);
        let weight_avg = (&weight + &prev_weight) / U256M::from_u32(e, 2);
        let d_price = &prev_price - &price;
        let integration_result = depth_avg * U256M::from_u128(e, PRICE_PRECISION) * weight_avg / U256M::from_u128(e, PRICE_PRECISION) * d_price / U256M::from_u128(e, PRICE_PRECISION);

        result_big = result_big + integration_result;

        prev_price = price.clone();
        prev_weight = weight.clone();
        prev_depth = depth.clone();
        // let in_amt_prev = biguint_to_128(in_amt.clone());
        in_amt = get_next_in_amt(e, &in_amt);
    }
    (result_big / U256M::from_u128(e, PRICE_PRECISION)).to_u128().unwrap()
}
