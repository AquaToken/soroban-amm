use crate::storage::{get_token_a, get_token_b};
use soroban_sdk::token::Client;
use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env};
use token_share::get_balance;

pub fn create_contract(
    e: &Env,
    token_wasm_hash: BytesN<32>,
    token_a: &Address,
    token_b: &Address,
) -> Address {
    let mut salt = Bytes::new(e);
    salt.append(&token_a.to_xdr(e));
    salt.append(&token_b.to_xdr(e));
    let salt = e.crypto().sha256(&salt);
    e.deployer()
        .with_current_contract(salt)
        .deploy(token_wasm_hash)
}

pub fn get_balance(e: &Env, contract: Address) -> u128 {
    bump_instance(e);
    Client::new(e, &contract).balance(&e.current_contract_address()) as u128
}

pub fn get_balance_a(e: &Env) -> u128 {
    get_balance(e, get_token_a(e))
}

pub fn get_balance_b(e: &Env) -> u128 {
    get_balance(e, get_token_b(e))
}

fn transfer(e: &Env, token: Address, to: Address, amount: i128) {
    Client::new(e, &token).transfer(&e.current_contract_address(), &to, &amount);
}

pub fn transfer_a(e: &Env, to: Address, amount: u128) {
    transfer(e, get_token_a(e), to, amount as i128);
}

pub fn transfer_b(e: &Env, to: Address, amount: u128) {
    transfer(e, get_token_b(e), to, amount as i128);
}
