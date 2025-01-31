use crate::storage::get_decimals;
use soroban_sdk::token::Client as SorobanTokenClient;
use soroban_sdk::{Address, Env, Vec};

// Get decimals for all pool tokens
pub fn read_decimals(e: &Env, tokens: &Vec<Address>) -> Vec<u32> {
    let mut decimals: Vec<u32> = Vec::new(&e);

    for token in tokens.iter() {
        // get coin decimals
        let token_client = SorobanTokenClient::new(&e, &token);
        let decimal = token_client.decimals();
        decimals.push_back(decimal);
    }
    decimals
}

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

// Reserves in normalized form (scaled to `Precision`)
pub fn xp(e: &Env, reserves: &Vec<u128>) -> Vec<u128> {
    let decimals = get_decimals(e);
    let mut result = get_precision_mul(e, &decimals);
    for i in 0..result.len() {
        result.set(i, result.get(i).unwrap() * reserves.get(i).unwrap())
    }
    result
}
