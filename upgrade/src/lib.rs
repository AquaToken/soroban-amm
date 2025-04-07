#![no_std]

mod constants;
mod errors;
pub mod events;
pub mod interface;
mod storage;

use crate::constants::UPGRADE_DELAY;
use crate::errors::Error;
use crate::storage::{
    get_future_wasm, get_upgrade_deadline, put_future_wasm, put_upgrade_deadline,
};
use access_control::emergency::get_emergency_mode;
use soroban_sdk::{panic_with_error, BytesN, Env};
use utils::storage_errors::StorageError;

pub fn commit_upgrade(e: &Env, new_wasm_hash: &BytesN<32>) {
    if get_upgrade_deadline(e) != 0 {
        panic_with_error!(e, Error::AnotherActionActive);
    }

    let deadline = e.ledger().timestamp() + UPGRADE_DELAY;
    put_upgrade_deadline(e, &deadline);
    put_future_wasm(e, &new_wasm_hash);
}

pub fn apply_upgrade(e: &Env) -> BytesN<32> {
    if !get_emergency_mode(e) {
        if e.ledger().timestamp() < get_upgrade_deadline(e) {
            panic_with_error!(e, Error::ActionNotReadyYet);
        }
        if get_upgrade_deadline(e) == 0 {
            panic_with_error!(e, Error::NoActionActive);
        }
    }

    put_upgrade_deadline(e, &0);
    let new_wasm_hash = match get_future_wasm(e) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    };
    e.deployer()
        .update_current_contract_wasm(new_wasm_hash.clone());
    new_wasm_hash
}

pub fn revert_upgrade(e: &Env) {
    put_upgrade_deadline(e, &0);
}
