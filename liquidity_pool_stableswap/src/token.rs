#![allow(unused)]
use crate::storage;
use soroban_sdk::{xdr::ToXdr, Address, Bytes, BytesN, Env, Vec};

pub fn create_contract(e: &Env, token_wasm_hash: BytesN<32>, tokens: &Vec<Address>) -> Address {
    let mut salt = Bytes::new(e);
    for token in tokens.iter() {
        salt.append(&token.to_xdr(e));
    }
    let salt = e.crypto().sha256(&salt);
    e.deployer()
        .with_current_contract(salt)
        .deploy_v2(token_wasm_hash, ())
}
