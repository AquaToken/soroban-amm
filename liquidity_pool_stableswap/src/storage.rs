use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env, Vec};
use utils::generate_instance_storage_getter;

use crate::normalize;
use rewards::utils::bump::bump_instance;
use utils::storage_errors::StorageError;
use utils::{
    generate_instance_storage_getter_and_setter,
    generate_instance_storage_getter_and_setter_with_default,
    generate_instance_storage_getter_with_default, generate_instance_storage_setter,
};

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Tokens,
    Decimals,
    Reserves,
    InitialA,
    InitialATime,
    FutureA,
    FutureATime,
    Fee,
    FutureFee,
    AdminFee,
    FutureAdminFee,
    AdminActionsDeadline,
    IsKilledSwap,
    IsKilledDeposit,
    IsKilledClaim,
    IsKilledGaugesClaim,
    Plane,
    Router,
    TokenFutureWASM,
    GaugeFutureWASM,

    // Tokens precision
    Precision, // target precision for internal calculations. It's the maximum precision of all tokens.
    PrecisionMul, // Scales raw token amounts to match `Precision`, accounting for decimal differences.

    ProtocolFeeFraction, // part of the fee that goes to the protocol, 5000 = 50% of the fee goes to the protocol
    ProtocolFees,
}

generate_instance_storage_getter_and_setter!(router, DataKey::Router, Address);
generate_instance_storage_getter_and_setter!(plane, DataKey::Plane, Address);
generate_instance_storage_getter_and_setter!(
    token_future_wasm,
    DataKey::TokenFutureWASM,
    BytesN<32>
);
generate_instance_storage_getter_and_setter!(
    gauge_future_wasm,
    DataKey::GaugeFutureWASM,
    BytesN<32>
);
generate_instance_storage_getter_and_setter_with_default!(
    is_killed_swap,
    DataKey::IsKilledSwap,
    bool,
    false
);
generate_instance_storage_getter_and_setter_with_default!(
    is_killed_deposit,
    DataKey::IsKilledDeposit,
    bool,
    false
);
generate_instance_storage_getter_and_setter_with_default!(
    is_killed_claim,
    DataKey::IsKilledClaim,
    bool,
    false
);
generate_instance_storage_getter_and_setter_with_default!(
    is_killed_gauges_claim,
    DataKey::IsKilledGaugesClaim,
    bool,
    false
);
generate_instance_storage_getter_and_setter_with_default!(
    protocol_fee_fraction,
    DataKey::ProtocolFeeFraction,
    u32,
    0
);

pub fn get_tokens(e: &Env) -> Vec<Address> {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::Tokens) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn get_decimals(e: &Env) -> Vec<u32> {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::Decimals) {
        Some(v) => v,
        None => {
            let decimals = normalize::read_decimals(e, &get_tokens(e));
            put_decimals(e, &decimals);
            decimals
        }
    }
}

pub fn get_reserves(e: &Env) -> Vec<u128> {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::Reserves) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_tokens(e: &Env, contracts: &Vec<Address>) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::Tokens, contracts);
}

pub fn put_decimals(e: &Env, decimals: &Vec<u32>) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::Decimals, decimals);
}

pub fn put_reserves(e: &Env, amounts: &Vec<u128>) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::Reserves, amounts);
}

// initial_A
pub fn get_initial_a(e: &Env) -> u128 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::InitialA) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_initial_a(e: &Env, value: &u128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::InitialA, value);
}

// initial A time
pub fn get_initial_a_time(e: &Env) -> u64 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::InitialATime) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_initial_a_time(e: &Env, value: &u64) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::InitialATime, value);
}

// future_a
pub fn get_future_a(e: &Env) -> u128 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::FutureA) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_future_a(e: &Env, value: &u128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::FutureA, value);
}

// future A time
pub fn get_future_a_time(e: &Env) -> u64 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::FutureATime) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_future_a_time(e: &Env, value: &u64) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::FutureATime, value);
}

// fee
pub fn get_fee(e: &Env) -> u32 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::Fee) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_fee(e: &Env, value: &u32) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::Fee, value);
}

// future_fee
pub fn get_future_fee(e: &Env) -> u32 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::FutureFee) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_future_fee(e: &Env, value: &u32) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::FutureFee, value);
}

// admin_actions_deadline
pub fn get_admin_actions_deadline(e: &Env) -> u64 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::AdminActionsDeadline) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_admin_actions_deadline(e: &Env, value: &u64) {
    bump_instance(e);
    e.storage()
        .instance()
        .set(&DataKey::AdminActionsDeadline, value);
}

pub(crate) fn has_plane(e: &Env) -> bool {
    let key = DataKey::Plane;
    e.storage().instance().has(&key)
}

// Tokens precision
// Precision - target precision for internal calculations. It's the maximum precision of all tokens.
pub fn set_precision(e: &Env, value: &u128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::Precision, value);
}

pub fn get_precision(e: &Env) -> u128 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::Precision) {
        Some(v) => v,
        None => {
            let precision = normalize::get_precision(&get_decimals(e));
            set_precision(e, &precision);
            precision
        }
    }
}

// Precision mul - Scales raw token amounts to match `Precision`, accounting for decimal differences.
pub fn set_precision_mul(e: &Env, value: &Vec<u128>) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::PrecisionMul, value);
}

pub fn get_precision_mul(e: &Env) -> Vec<u128> {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::PrecisionMul) {
        Some(v) => v,
        None => {
            let precision_mul = normalize::get_precision_mul(e, &get_decimals(e));
            set_precision_mul(e, &precision_mul);
            precision_mul
        }
    }
}

pub(crate) fn put_protocol_fees(e: &Env, value: &Vec<u128>) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::ProtocolFees, value);
}

pub(crate) fn get_protocol_fees(e: &Env) -> Vec<u128> {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::ProtocolFees) {
        Some(v) => v,
        None => {
            let tokens = get_tokens(e);
            let mut fees = Vec::new(e);
            for _ in tokens {
                fees.push_back(0);
            }
            put_protocol_fees(e, &fees);
            fees
        }
    }
}
