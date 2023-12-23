#![no_std]

use soroban_sdk::{contracttype, Address, Env};
use utils::bump::bump_instance;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    TokenShare,
}

pub mod token {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
}
pub use token::{self as token_contract, Client};

fn get_balance(e: &Env, contract: Address) -> u128 {
    bump_instance(e);
    Client::new(e, &contract).balance(&e.current_contract_address()) as u128
}

pub fn get_token_share(e: &Env) -> Address {
    bump_instance(&e);
    e.storage()
        .instance()
        .get(&DataKey::TokenShare)
        .expect("Trying to get Token Share")
}

pub fn put_token_share(e: &Env, contract: Address) {
    bump_instance(&e);
    e.storage().instance().set(&DataKey::TokenShare, &contract)
}

pub fn get_balance_shares(e: &Env) -> u128 {
    get_balance(e, get_token_share(e))
}

pub fn get_user_balance_shares(e: &Env, user: &Address) -> u128 {
    Client::new(e, &get_token_share(e)).balance(user) as u128
}

pub fn get_total_shares(e: &Env) -> u128 {
    let share_token = get_token_share(e);
    Client::new(e, &share_token).total_balance() as u128
}

pub fn burn_shares(e: &Env, amount: i128) {
    let share_contract = get_token_share(e);
    Client::new(e, &share_contract).burn(&e.current_contract_address(), &amount);
}

pub fn mint_shares(e: &Env, to: Address, amount: i128) {
    let share_contract_id = get_token_share(e);
    Client::new(e, &share_contract_id).mint(&to, &amount);
}
