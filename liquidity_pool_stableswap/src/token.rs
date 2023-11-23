#![allow(unused)]
use crate::storage;
use rewards::utils::constant::{INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD};
use soroban_sdk::{Address, Bytes, BytesN, Env};

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
