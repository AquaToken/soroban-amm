use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, Env, Vec};

use rewards::utils::bump::bump_instance;
use utils::storage_errors::StorageError;
use utils::{
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
    TransferOwnershipDeadline,
    IsKilledSwap,
    IsKilledDeposit,
    IsKilledClaim,
    Plane,
    Router,
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
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
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

// admin_fee
pub fn get_admin_fee(e: &Env) -> u32 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::AdminFee) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_admin_fee(e: &Env, value: &u32) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::AdminFee, value);
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

// future_admin_fee
pub fn get_future_admin_fee(e: &Env) -> u32 {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::FutureAdminFee) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_future_admin_fee(e: &Env, value: &u32) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::FutureAdminFee, value);
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

// transfer_ownership_deadline
pub fn get_transfer_ownership_deadline(e: &Env) -> u64 {
    bump_instance(e);
    match e
        .storage()
        .instance()
        .get(&DataKey::TransferOwnershipDeadline)
    {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}

pub fn put_transfer_ownership_deadline(e: &Env, value: &u64) {
    bump_instance(e);
    e.storage()
        .instance()
        .set(&DataKey::TransferOwnershipDeadline, value);
}

// pool plane
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
