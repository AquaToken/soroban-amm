#![allow(unused)]
use crate::storage;
use soroban_sdk::{Address, Bytes, BytesN, Env};

pub fn create_contract(e: &Env, token_wasm_hash: BytesN<32>) -> Address {
    let mut salt = Bytes::new(e);
    let salt = e.crypto().sha256(&salt);
    e.deployer()
        .with_current_contract(salt)
        .deploy(token_wasm_hash)
}
