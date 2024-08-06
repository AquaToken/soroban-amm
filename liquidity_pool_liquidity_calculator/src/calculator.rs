use crate::constants::{PRECISION, RESERVES_NORM};
use soroban_sdk::Vec;

pub fn price_weight(price: u128, min_price: u128) -> u128 {
    if price == 0 {
        return 0;
    }

    // returns price weighted with exponent (p_min/p)^8
    let mut result = PRECISION * min_price / price;
    for _i in 1..8 {
        result = result * min_price / price;
    }
    result
}

pub fn get_next_in_amt(in_amt: u128) -> u128 {
    // decrease dx exponentially
    in_amt * 100 / 125
}

pub fn get_max_reserve(reserves: &Vec<u128>) -> u128 {
    let mut max_reserve = 0;
    for value in reserves.clone() {
        if max_reserve < value {
            max_reserve = value;
        }
    }
    max_reserve
}

pub fn normalize_reserves(reserves: &Vec<u128>) -> (Vec<u128>, u128, u128) {
    let mut reserves_norm = reserves.clone();
    let max_reserve = get_max_reserve(reserves);

    if max_reserve == 0 {
        // nothing to normalize. we'll just get division by zero error
        return (reserves_norm, 1, 1);
    }

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
