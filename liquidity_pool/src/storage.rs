use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env};
pub use utils::bump::bump_instance;
use utils::generate_instance_storage_getter;
use utils::storage_errors::StorageError;
use utils::{
    generate_instance_storage_getter_and_setter,
    generate_instance_storage_getter_and_setter_with_default,
    generate_instance_storage_getter_with_default, generate_instance_storage_setter,
};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    TokenA,
    TokenB,
    ReserveA,
    ReserveB,
    FeeFraction, // 1 = 0.01%
    Plane,
    Router,
    IsKilledSwap,
    IsKilledDeposit,
    IsKilledClaim,

    TokenFutureWASM,
    GaugeFutureWASM,

    ProtocolFeeFraction, // part of the fee that goes to the protocol, 5000 = 50% of the fee goes to the protocol
    ProtocolFeeA,
    ProtocolFeeB,
}

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
    protocol_fee_fraction,
    DataKey::ProtocolFeeFraction,
    u32,
    0
);
generate_instance_storage_getter_and_setter_with_default!(
    protocol_fee_a,
    DataKey::ProtocolFeeA,
    u128,
    0
);
generate_instance_storage_getter_and_setter_with_default!(
    protocol_fee_b,
    DataKey::ProtocolFeeB,
    u128,
    0
);
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

pub fn get_token_a(e: &Env) -> Address {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::TokenA) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn get_token_b(e: &Env) -> Address {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::TokenB) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn get_reserve_a(e: &Env) -> u128 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::ReserveA) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn get_reserve_b(e: &Env) -> u128 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::ReserveB) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_token_a(e: &Env, contract: Address) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::TokenA, &contract)
}

pub fn put_token_b(e: &Env, contract: Address) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::TokenB, &contract)
}

pub fn put_reserve_a(e: &Env, amount: u128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::ReserveA, &amount)
}

pub fn put_reserve_b(e: &Env, amount: u128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::ReserveB, &amount)
}

pub fn get_fee_fraction(e: &Env) -> u32 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::FeeFraction) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_fee_fraction(e: &Env, value: u32) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::FeeFraction, &value)
}

pub(crate) fn has_plane(e: &Env) -> bool {
    let key = DataKey::Plane;
    e.storage().instance().has(&key)
}
