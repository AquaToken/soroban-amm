use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env};
pub use utils::bump::bump_instance;
use utils::storage_errors::StorageError;
use utils::{
    generate_instance_storage_getter_and_setter_with_default,
    generate_instance_storage_getter_with_default, generate_instance_storage_setter,
};

#[derive(Clone)]
#[contracttype]
enum DataKey {
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

pub(crate) fn set_plane(e: &Env, plane: &Address) {
    let key = DataKey::Plane;
    bump_instance(e);
    e.storage().instance().set(&key, plane);
}

pub(crate) fn get_plane(e: &Env) -> Address {
    let key = DataKey::Plane;
    match e.storage().instance().get(&key) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub(crate) fn has_plane(e: &Env) -> bool {
    let key = DataKey::Plane;
    e.storage().instance().has(&key)
}

pub(crate) fn set_router(e: &Env, plane: &Address) {
    let key = DataKey::Router;
    bump_instance(e);
    e.storage().instance().set(&key, plane);
}

pub(crate) fn get_router(e: &Env) -> Address {
    let key = DataKey::Router;
    match e.storage().instance().get(&key) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub(crate) fn set_token_future_wasm(e: &Env, value: &BytesN<32>) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::TokenFutureWASM, value)
}

pub(crate) fn get_token_future_wasm(e: &Env) -> BytesN<32> {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::TokenFutureWASM) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}
