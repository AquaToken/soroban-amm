use soroban_sdk::Env;
use crate::constants::PRICE_PRECISION;
use crate::u256::U256M;


// const POOL_TYPE_STANDARD: Symbol = symbol_short!("standard");
// const POOL_TYPE_STABLESWAP: Symbol = symbol_short!("stable");


pub (crate) fn get_next_in_amt(e: &Env, in_amt: &U256M) -> U256M {
    // decrease dx exponentially
    // in_amt * U256M::from(100_u8) / U256M::from(110_u8)
    in_amt * U256M::from_u32(e, 100) / U256M::from_u32(e, 125)
    // in_amt * U256M::from(50_u8) / U256M::from(100_u8)
}

pub (crate) fn price_weight(e: &Env, price: &U256M, min_price_p8: &U256M) -> U256M {
    // return U256M::from(1_u32);
    // returns price weighted with exponent (p_min/p)^8
    U256M::from_u128(e, PRICE_PRECISION) * min_price_p8 / price.pow(8)
}

// fn estimate_swap(
//     pool_type: Symbol,
//     init_args: SorobanVec<u128>,
//     reserves: Vec<BigUint>,
// ) -> u128 {
//     if pool_type == POOL_TYPE_STANDARD {
//         let (fee, reserves) = parse_standard_data(init_args, reserves);
//         estimate_swap(&fee_fraction_big, &reserves_adj, in_idx, out_idx, &min_amount)
//     } else if pool_type == POOL_TYPE_STABLESWAP {
//
//     } else {
//         panic!("unknown pool type");
//     }
// }

// pub(crate) fn get_liquidity(
//     e: &Env,
//     pool_type: Symbol,
//     init_args: &Vec<u128>,
//     reserves: &Vec<u128>,
//     in_idx: u32,
//     out_idx: u32,
// ) -> u128 {
//     if reserves.len() != 2 {
//         panic!("liquidity calculation is allowed for 2 tokens only")
//     }
//
//     // pre-compiled frequent values
//     let zero = BigUint::from(0_u8);
//     let one = BigUint::from(1_u8);
//     let two = BigUint::from(2_u8);
//     let price_precision = BigUint::from(PRICE_PRECISION);
//
//     let fee_fraction_big = BigUint::from(fee_fraction);
//     let reserve_in = BigUint::from(reserves.get(0).unwrap());
//     let reserve_out = BigUint::from(reserves.get(1).unwrap());
//
//     if reserve_in == zero || reserve_out == zero {
//         return 0;
//     }
//
//     let mut result_big = zero.clone();
//     // let min_price_func = get_min_price(reserve_in, reserve_out, fee_fraction);
//     // let min_price = reserve_in * BigUint::from(PRICE_PRECISION) / reserve_out;
//
//     let min_amount = price_precision.clone();
//     let mut reserves_adj = alloc::vec![];
//     let mut reserves_big = alloc::vec![];
//     for i in 0..reserves.len() {
//         reserves_big.push(BigUint::from(reserves.get(i).unwrap()));
//         reserves_adj.push(BigUint::from(reserves.get(i).unwrap()) * &price_precision);
//     }
//     let min_price = min_amount.clone() * &price_precision / estimate_swap(&fee_fraction_big, &reserves_adj, in_idx, out_idx, &min_amount);
//     let min_price_p8 = min_price.pow(8);
//
//     let mut prev_price = zero.clone();
//     let mut prev_weight = one.clone();
//     let mut prev_depth = zero.clone();
//
//     let mut first_iteration = true;
//     let mut last_iteration = false;
//     let mut in_amt: BigUint = reserve_in * &two;
//
//     // todo: how to describe range properly?
//     while !last_iteration {
//         let mut depth = estimate_swap(&fee_fraction_big, &reserves_big, in_idx, out_idx, &in_amt);
//         let mut price = in_amt.clone() * &price_precision / depth.clone();
//         let mut weight = price_weight(&price, &min_price_p8);
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
//         if price > prev_price {
//             // todo: do we need this case? don't go into last iteration since we've jumped below min price
//             if prev_price < min_price {
//                 break;
//             }
//
//             price = min_price.clone();
//             weight = one.clone();
//             depth = zero.clone();
//             last_iteration = true;
//         }
//         // // if price has changed for less than 1%, skip iteration
//         // else if &price * BigUint::from(101_u32) > &prev_price * BigUint::from(100_u32) {
//         //     in_amt = get_next_in_amt(&in_amt);
//         //     continue;
//         // }
//
//         let depth_avg = (&depth + &prev_depth) / &two;
//         let weight_avg = (&weight + &prev_weight) / &two;
//         let d_price = &prev_price - &price;
//         let integration_result = depth_avg * &price_precision * weight_avg / &price_precision * d_price / &price_precision;
//
//         result_big += integration_result;
//
//         prev_price = price.clone();
//         prev_weight = weight.clone();
//         prev_depth = depth.clone();
//         in_amt = get_next_in_amt(&in_amt);
//     }
//     biguint_to_128(result_big / &price_precision)
// }
//
//
// pub (crate) fn get_pool_liquidity(e: &Env, pool_type: Symbol, init_args: Vec<u128>, reserves: Vec<u128>) -> u128 {
//     let mut out = 0;
//     if pool_type == POOL_TYPE_STANDARD {
//         let (fee, reserves) = parse_standard_data(init_args, reserves);
//         out += standard_pool::get_liquidity(fee, &reserves, 0, 1);
//         out += standard_pool::get_liquidity(fee, &reserves, 1, 0);
//     } else if pool_type == POOL_TYPE_STABLESWAP {
//         let data = parse_stableswap_data(init_args, reserves);
//         // calculate liquidity for all non-duplicate permutations
//         for i in 0..data.reserves.len(){
//             for j in 0..data.reserves.len() {
//                 let in_idx = i;
//                 let out_idx = data.reserves.len() - j - 1;
//                 if in_idx == out_idx {
//                     continue;
//                 }
//
//                 out += stableswap_pool::get_liquidity(
//                     e,
//                     data.fee,
//                     data.initial_a,
//                     data.initial_a_time,
//                     data.future_a,
//                     data.future_a_time,
//                     &data.reserves,
//                     in_idx,
//                     out_idx,
//                 );
//             }
//         }
//     } else {
//         panic!("unknown pool type");
//     };
//     out
// }