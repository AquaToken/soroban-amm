#![no_std]

use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{contracttype, panic_with_error, Address, Env};
use utils::bump::bump_instance;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    TokenShare,
    TotalShares,
}

pub mod token {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
}
pub use token::{self as token_contract, Client};
use utils::storage_errors::StorageError;

pub fn get_token_share(e: &Env) -> Address {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::TokenShare) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_token_share(e: &Env, contract: Address) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::TokenShare, &contract)
}

pub fn get_user_balance_shares(e: &Env, user: &Address) -> u128 {
    SorobanTokenClient::new(e, &get_token_share(e)).balance(user) as u128
}

pub fn get_total_shares(e: &Env) -> u128 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::TotalShares)
        .unwrap_or(0)
}

pub fn put_total_shares(e: &Env, value: u128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::TotalShares, &value)
}

pub fn burn_shares(e: &Env, from: &Address, amount: u128) {
    let total_share = get_total_shares(e);
    put_total_shares(e, total_share - amount);

    let share_contract = get_token_share(e);
    SorobanTokenClient::new(e, &share_contract).burn(from, &(amount as i128));
}

pub fn mint_shares(e: &Env, to: &Address, amount: i128) {
    let total_share = get_total_shares(e);
    put_total_shares(e, total_share + amount as u128);

    let share_contract_id = get_token_share(e);
    SorobanTokenAdminClient::new(e, &share_contract_id).mint(to, &amount);
}
