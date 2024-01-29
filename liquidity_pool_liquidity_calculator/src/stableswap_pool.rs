// use soroban_sdk::{Env, Vec as SorobanVec};
// use crate::constants::{FEE_MULTIPLIER, PRICE_PRECISION};
// use num_bigint::BigUint;
// extern crate alloc;
// use alloc::vec::Vec;
// use crate::calculator::{get_next_in_amt, price_weight};
// use crate::utils::biguint_to_128;
//
// const RATE: u128 = 1_0000000_u128;
// const PRECISION: u128 = 1_0000000_u128;
//
//
// fn a(e: &Env, initial_a: &BigUint, initial_a_time: &BigUint, future_a: &BigUint, future_a_time: &BigUint) -> BigUint {
//     // Handle ramping A up or down
//     let t1 = future_a_time;
//     let a1 = future_a;
//     let now = BigUint::from(e.ledger().timestamp());
//
//     if now < t1.clone() {
//         let a0 = initial_a;
//         let t0 = initial_a_time;
//         // Expressions in u128 cannot have negative numbers, thus "if"
//         if a1 > a0 {
//             (a0 + (a1 - a0) * (now - t0) / (t1 - t0)).clone()
//         } else {
//             (a0 - (a0 - a1) * (now - t0) / (t1 - t0)).clone()
//         }
//     } else {
//         // when t1 == 0 or block.timestamp >= t1
//         a1.clone()
//     }
// }
//
// // xp size = N_COINS
// fn get_d(n_coins: u32, xp: &Vec<BigUint>, amp: &BigUint) -> BigUint {
//     let mut s = BigUint::from(0_u8);
//     for x in xp {
//         s += x;
//     }
//     if s == BigUint::from(0_u8) {
//         return BigUint::from(0_u8);
//     }
//
//     let mut d_prev: BigUint;
//     let mut d = s.clone();
//     let ann = amp * n_coins as u128;
//     for _i in 0..255 {
//         let mut d_p = d.clone();
//         for x1 in xp {
//             d_p = &d_p * &d / (x1 * n_coins as u128) // If division by 0, this will be borked: only withdrawal will work. And that is good
//         }
//         d_prev = d.clone();
//         d = (&ann * &s + &d_p * n_coins as u128) * &d / ((&ann - BigUint::from(1_u8)) * &d + BigUint::from(n_coins + 1) * &d_p);
//         // // Equality with the precision of 1
//         if d > d_prev {
//             if &d - d_prev <= BigUint::from(1_u8) {
//                 break;
//             }
//         } else if d_prev - &d <= BigUint::from(1_u8) {
//             break;
//         }
//     }
//     d
// }
//
// fn get_y(n_coins: u32, in_idx: u32, out_idx: u32, x: &BigUint, xp: &Vec<BigUint>, a: &BigUint) -> BigUint {
//     // x in the input is converted to the same price/precision
//
//     if in_idx == out_idx {
//         panic!("same coin")
//     } // dev: same coin
//     // if !(j >= 0) {
//     //     panic!("j below zero")
//     // } // dev: j below zero
//     if out_idx >= n_coins {
//         panic!("j above N_COINS")
//     } // dev: j above N_COINS
//
//     // should be unreachable, but good for safety
//     // if !(i >= 0) {
//     //     panic!("bad arguments")
//     // }
//     if in_idx >= n_coins {
//         panic!("bad arguments")
//     }
//
//     let amp = a;
//     let d = get_d(xp.len() as u32, xp, amp);
//     let mut c = d.clone();
//     let mut s = BigUint::from(0_u8);
//     let ann = amp * n_coins as u128;
//
//     let mut x1;
//     for i in 0..n_coins {
//         if i == in_idx {
//             x1 = x.clone();
//         } else if i != out_idx {
//             x1 = xp[i as usize].clone();
//         } else {
//             continue;
//         }
//         s += &x1;
//         c = &c * &d / (&x1 * n_coins as u128);
//     }
//     c = &c * &d / (&ann * n_coins as u128);
//     let b = s + &d / &ann; // - D
//     let mut y_prev;
//     let mut y = d.clone();
//     for _i in 0..255 {
//         y_prev = y.clone();
//         y = (&y * &y + &c) / (BigUint::from(2_u8) * &y + &b - &d);
//         // Equality with the precision of 1
//         if y > y_prev {
//             if &y - y_prev <= BigUint::from(1_u8) {
//                 break;
//             }
//         } else if y_prev - &y <= BigUint::from(1_u8) {
//             break;
//         }
//     }
//     y
// }
//
// fn get_dy(reserves: &Vec<BigUint>, fee_fraction: &BigUint, a: &BigUint, i: u32, j: u32, dx: &BigUint) -> BigUint {
//     // dx and dy in c-units
//     let xp = reserves;
//
//     let x = xp[i as usize].clone() + (dx * BigUint::from(RATE) / BigUint::from(PRECISION));
//     let y = get_y(reserves.len() as u32, i, j, &x, &xp, a);
//
//     if y == BigUint::from(0_u8) {
//         // pool is empty
//         return BigUint::from(0_u8);
//     }
//
//     let dy = (xp[j as usize].clone() - y - BigUint::from(1_u8)) * BigUint::from(PRECISION) / BigUint::from(RATE);
//     let fee = fee_fraction * &dy / BigUint::from(FEE_MULTIPLIER);
//     dy - fee
// }
//
// fn estimate_swap(
//     e: &Env,
//     fee_fraction: &BigUint,
//     initial_a: &BigUint,
//     initial_a_time: &BigUint,
//     future_a: &BigUint,
//     future_a_time: &BigUint,
//     reserves: &Vec<BigUint>,
//     in_idx: u32,
//     out_idx: u32,
//     in_amount: &BigUint,
// ) -> BigUint {
//     let a = a(e, initial_a, initial_a_time, future_a, future_a_time);
//     get_dy(reserves, fee_fraction, &a, in_idx, out_idx, in_amount)
// }
//
// pub(crate) fn get_liquidity(
//     e: &Env,
//     fee_fraction: u128,
//     initial_a: u128,
//     initial_a_time: u128,
//     future_a: u128,
//     future_a_time: u128,
//     reserves: &SorobanVec<u128>,
//     in_idx: u32,
//     out_idx: u32,
// ) -> u128 {
//     let fee_fraction_big = BigUint::from(fee_fraction);
//     let initial_a_big = BigUint::from(initial_a);
//     let initial_a_time_big = BigUint::from(initial_a_time);
//     let future_a_big = BigUint::from(future_a);
//     let future_a_time_big = BigUint::from(future_a_time);
//     let reserve_in = BigUint::from(reserves.get(0).unwrap());
//     let reserve_out = BigUint::from(reserves.get(1).unwrap());
//
//     if reserve_in == BigUint::from(0_u8) || reserve_out == BigUint::from(0_u8) {
//         return 0;
//     }
//
//     let mut result_big = BigUint::from(0_u8);
//     // let min_price_func = get_min_price(reserve_in, reserve_out, fee_fraction);
//     // let min_price = reserve_in * BigUint::from(PRICE_PRECISION) / reserve_out;
//
//     let min_amount = BigUint::from(PRICE_PRECISION);
//     let mut reserves_adj = alloc::vec![];
//     let mut reserves_big = alloc::vec![];
//     for i in 0..reserves.len() {
//         reserves_big.push(BigUint::from(reserves.get(i).unwrap()));
//         reserves_adj.push(BigUint::from(reserves.get(i).unwrap()) * BigUint::from(PRICE_PRECISION));
//     }
//     let min_price = &min_amount * BigUint::from(PRICE_PRECISION) / estimate_swap(e, &fee_fraction_big, &initial_a_big, &initial_a_time_big, &future_a_big, &future_a_time_big, &reserves_adj, in_idx, out_idx, &min_amount);
//     let min_price_p8 = min_price.pow(8);
//
//     let mut prev_price = BigUint::from(0_u8);
//     let mut prev_weight = BigUint::from(1_u8);
//     let mut prev_depth = BigUint::from(0_u8);
//
//     let mut first_iteration = true;
//     let mut last_iteration = false;
//     let mut in_amt: BigUint = reserve_in * BigUint::from(2_u8);
//
//     // todo: how to describe range properly?
//     let mut i = 0;
//     while !last_iteration {
//         i += 1;
//         let mut depth = estimate_swap(e, &fee_fraction_big, &initial_a_big, &initial_a_time_big, &future_a_big, &future_a_time_big, &reserves_big, in_idx, out_idx, &in_amt);
//         let mut price = &in_amt * BigUint::from(PRICE_PRECISION) / &depth;
//         let mut weight = price_weight(e, &price, &min_price_p8);
//
//         if first_iteration {
//             prev_price = price.clone();
//             prev_depth = depth.clone();
//             prev_weight = weight.clone();
//             first_iteration = false;
//             continue;
//         }
//
//         // stop if rounding affects price
//         // then integrate up to min price
//         // todo: add exit condition on iterations amount
//         if price > prev_price {
//             // todo: do we need this case? don't go into last iteration since we've jumped below min price
//             if prev_price < min_price {
//                 break;
//             }
//
//             price = min_price.clone();
//             weight = BigUint::from(1_u8);
//             depth = BigUint::from(0_u8);
//             last_iteration = true;
//         }
//         // // if price has changed for less than 0.01%, skip iteration
//         // else if &price * BigUint::from(10001_u32) > &prev_price * BigUint::from(10000_u32) {
//         //     in_amt = get_next_in_amt(&in_amt);
//         //     continue;
//         // }
//
//         let depth_avg = (&depth + &prev_depth) / BigUint::from(2_u8);
//         let weight_avg = (&weight + &prev_weight) / BigUint::from(2_u8);
//         let d_price = &prev_price - &price;
//         let integration_result = depth_avg * BigUint::from(PRICE_PRECISION) * weight_avg / BigUint::from(PRICE_PRECISION) * d_price / BigUint::from(PRICE_PRECISION);
//
//         result_big += integration_result;
//
//         prev_price = price.clone();
//         prev_weight = weight.clone();
//         prev_depth = depth.clone();
//         // let in_amt_prev = biguint_to_128(in_amt.clone());
//         in_amt = get_next_in_amt(&in_amt);
//     }
//     biguint_to_128(result_big / BigUint::from(PRICE_PRECISION))
// }
