use crate::storage::get_decimals;
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::{Env, Vec};

// Target precision for internal calculations. It's the maximum precision of all tokens.
pub fn get_precision(decimals: &Vec<u32>) -> u128 {
    let max_decimals = decimals.iter().max().unwrap();
    10u128.pow(max_decimals)
}

// Scales raw token amounts to match `Precision`, accounting for decimal differences.
pub fn get_precision_mul(e: &Env, decimals: &Vec<u32>) -> Vec<u128> {
    let precision = get_precision(decimals);
    let mut precision_mul = Vec::new(e);
    for token_decimals in decimals.iter() {
        precision_mul.push_back(precision / 10u128.pow(token_decimals));
    }
    precision_mul
}

// Adjust token amounts for decimal differences
pub fn get_rates(e: &Env, decimals: &Vec<u32>) -> Vec<u128> {
    let mut rates = Vec::new(e);
    let precision = get_precision(&decimals);
    for precision_mul in get_precision_mul(e, &decimals) {
        rates.push_back(precision * precision_mul);
    }
    rates
}

// Reserves in normalized form (scaled to `Precision`)
pub fn xp(e: &Env, reserves: &Vec<u128>) -> Vec<u128> {
    let decimals = get_decimals(e);
    let precision = get_precision(&decimals);
    let mut result = get_rates(e, &decimals);
    for i in 0..result.len() {
        result.set(
            i,
            result
                .get(i)
                .unwrap()
                .fixed_mul_floor(e, &reserves.get(i).unwrap(), &precision),
        )
    }
    result
}
