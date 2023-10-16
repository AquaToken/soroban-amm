#![allow(unused)]
use crate::constants::{INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD};
use crate::storage;
use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env};

soroban_sdk::contractimport!(
    file = "../token/target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
);

pub fn create_contract(
    e: &Env,
    token_wasm_hash: BytesN<32>,
    // token_a: &Address,
    // token_b: &Address,
) -> Address {
    let mut salt = Bytes::new(e);
    // salt.append(&token_a.to_xdr(e));
    // salt.append(&token_b.to_xdr(e));
    let salt = e.crypto().sha256(&salt);
    e.deployer()
        .with_current_contract(salt)
        .deploy(token_wasm_hash)
}

fn get_balance(e: &Env, contract: Address) -> i128 {
    storage::bump_instance(e);
    Client::new(e, &contract).balance(&e.current_contract_address())
}

// pub fn get_balance_a(e: &Env) -> i128 {
//     get_balance(e, storage::get_token_a(e))
// }

// pub fn get_balance_b(e: &Env) -> i128 {
//     get_balance(e, storage::get_token_b(e))
// }

pub fn get_balance_shares(e: &Env) -> i128 {
    get_balance(e, storage::get_token_share(e))
}

pub fn get_user_balance_shares(e: &Env, user: &Address) -> i128 {
    Client::new(e, &storage::get_token_share(e)).balance(user)
}

pub fn get_total_shares(e: &Env) -> i128 {
    let share_token = storage::get_token_share(e);
    Client::new(e, &share_token).total_balance()
}

pub fn burn_shares(e: &Env, amount: i128) {
    let share_contract = storage::get_token_share(e);
    Client::new(e, &share_contract).burn(&e.current_contract_address(), &amount);
}

pub fn mint_shares(e: &Env, to: Address, amount: i128) {
    let share_contract_id = storage::get_token_share(e);
    Client::new(e, &share_contract_id).mint(&to, &amount);
}

// fn transfer(e: &Env, token: Address, to: Address, amount: i128) {
//     Client::new(e, &token).transfer(&e.current_contract_address(), &to, &amount);
// }
//
// pub fn transfer_a(e: &Env, to: Address, amount: i128) {
//     transfer(e, storage::get_token_a(e), to, amount);
// }
//
// pub fn transfer_b(e: &Env, to: Address, amount: i128) {
//     transfer(e, storage::get_token_b(e), to, amount);
// }
