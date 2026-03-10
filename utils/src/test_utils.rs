#![cfg(any(test, feature = "testutils"))]
extern crate std;

use soroban_sdk::testutils::{Events, Ledger, LedgerInfo};
use soroban_sdk::{Address, BytesN, Env, TryFromVal, Val, U256};

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

pub fn time_warp(e: &Env, time: u64) {
    assert!(e.ledger().timestamp() <= time, "Cannot warp to the past");
    e.ledger().set(LedgerInfo {
        timestamp: time,
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

// ── Generic event helpers ────────────────────────────────────────────────

/// Filter contract events by first-topic symbol name.
pub fn get_events_by_name(
    env: &Env,
    contract: &Address,
    name: &str,
) -> std::vec::Vec<soroban_sdk::xdr::ContractEvent> {
    env.events()
        .all()
        .filter_by_contract(contract)
        .events()
        .iter()
        .filter(|e| {
            let soroban_sdk::xdr::ContractEventBody::V0(body) = &e.body;
            matches!(body.topics.first(),
                Some(soroban_sdk::xdr::ScVal::Symbol(s)) if s.0.as_slice() == name.as_bytes())
        })
        .cloned()
        .collect()
}

/// Count contract events with the given first-topic symbol name.
pub fn count_events(env: &Env, contract: &Address, name: &str) -> usize {
    get_events_by_name(env, contract, name).len()
}

/// Extract topic at `index` from an XDR event as a Soroban `Address`.
pub fn event_topic_as_address(
    env: &Env,
    event: &soroban_sdk::xdr::ContractEvent,
    index: usize,
) -> Address {
    let soroban_sdk::xdr::ContractEventBody::V0(body) = &event.body;
    Address::try_from_val(env, &Val::try_from_val(env, &body.topics[index]).unwrap()).unwrap()
}

/// Extract event body as a concrete type `T`.
pub fn event_data<T: TryFromVal<Env, Val>>(
    env: &Env,
    event: &soroban_sdk::xdr::ContractEvent,
) -> T {
    let soroban_sdk::xdr::ContractEventBody::V0(body) = &event.body;
    let val: Val = Val::try_from_val(env, &body.data).unwrap();
    T::try_from_val(env, &val).unwrap()
}
