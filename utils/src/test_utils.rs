#![cfg(any(test, feature = "testutils"))]

use soroban_sdk::testutils::{Ledger, LedgerInfo};
use soroban_sdk::{BytesN, Env, U256};

pub fn assert_approx_eq_abs(a: u128, b: u128, delta: u128) {
    assert!(
        a > b - delta && a < b + delta,
        "assertion failed: `(left != right)` \
         (left: `{:?}`, right: `{:?}`, epsilon: `{:?}`)",
        a,
        b,
        delta
    );
}

pub fn assert_approx_eq_abs_u256(a: U256, b: U256, delta: U256) {
    assert!(
        a > b.sub(&delta) && a < b.add(&delta),
        "assertion failed: `(left != right)` \
         (left: `{:?}`, right: `{:?}`, epsilon: `{:?}`)",
        a,
        b,
        delta
    );
}

pub fn jump(e: &Env, time: u64) {
    e.ledger().set(LedgerInfo {
        timestamp: e.ledger().timestamp().saturating_add(time),
        protocol_version: e.ledger().protocol_version(),
        sequence_number: e.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 999999,
        min_persistent_entry_ttl: 999999,
        max_entry_ttl: u32::MAX,
    });
}

pub fn jump_sequence(e: &Env, sequence: u32) {
    e.ledger().set(LedgerInfo {
        timestamp: e.ledger().timestamp(),
        protocol_version: e.ledger().protocol_version(),
        sequence_number: e.ledger().sequence().saturating_add(sequence),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 999999,
        min_persistent_entry_ttl: 999999,
        max_entry_ttl: u32::MAX,
    });
}

pub fn install_dummy_wasm<'a>(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(file = "../contracts/dummy_contract.wasm");
    e.deployer().upload_contract_wasm(WASM)
}
