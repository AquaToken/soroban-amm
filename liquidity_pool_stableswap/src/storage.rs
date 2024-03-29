use crate::pool_constants::N_COINS;
use rewards::utils::bump::bump_instance;
use soroban_sdk::{contracttype, Address, Env, Vec};

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Tokens,
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
    KillDeadline,
    IsKilled,
    Plane,
}

pub fn get_tokens(e: &Env) -> Vec<Address> {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::Tokens)
        .expect("Please initialize Tokens")
}

pub fn get_reserves(e: &Env) -> Vec<u128> {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::Reserves)
        .expect("Please initialize Reserves")
}

pub fn put_tokens(e: &Env, contracts: &Vec<Address>) {
    if contracts.len() != N_COINS as u32 {
        panic!("wrong vector size")
    }
    bump_instance(e);
    e.storage().instance().set(&DataKey::Tokens, contracts);
}

pub fn put_reserves(e: &Env, amounts: &Vec<u128>) {
    if amounts.len() != N_COINS as u32 {
        panic!("wrong vector size")
    }
    bump_instance(e);
    e.storage().instance().set(&DataKey::Reserves, amounts);
}

// initial_A
pub fn get_initial_a(e: &Env) -> u128 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::InitialA)
        .expect("Please initialize initial_A")
}

pub fn put_initial_a(e: &Env, value: &u128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::InitialA, value);
}

// initial A time
pub fn get_initial_a_time(e: &Env) -> u64 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::InitialATime)
        .expect("Please initialize Initial A Time")
}

pub fn put_initial_a_time(e: &Env, value: &u64) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::InitialATime, value);
}

// future_a
pub fn get_future_a(e: &Env) -> u128 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::FutureA)
        .expect("Please initialize future_A")
}

pub fn put_future_a(e: &Env, value: &u128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::FutureA, value);
}

// fitire A time
pub fn get_future_a_time(e: &Env) -> u64 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::FutureATime)
        .expect("Please initialize FutureATime")
}

pub fn put_future_a_time(e: &Env, value: &u64) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::FutureATime, value);
}

// fee
pub fn get_fee(e: &Env) -> u32 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::Fee)
        .expect("Please initialize fee")
}

pub fn put_fee(e: &Env, value: &u32) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::Fee, value);
}

// admin_fee
pub fn get_admin_fee(e: &Env) -> u32 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::AdminFee)
        .expect("Please initialize admin_fee")
}

pub fn put_admin_fee(e: &Env, value: &u32) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::AdminFee, value);
}

// future_fee
pub fn get_future_fee(e: &Env) -> u32 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::FutureFee)
        .expect("Please initialize future_fee")
}

pub fn put_future_fee(e: &Env, value: &u32) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::FutureFee, value);
}

// future_admin_fee
pub fn get_future_admin_fee(e: &Env) -> u32 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::FutureAdminFee)
        .expect("Please initialize future_admin_fee")
}

pub fn put_future_admin_fee(e: &Env, value: &u32) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::FutureAdminFee, value);
}

// admin_actions_deadline
pub fn get_admin_actions_deadline(e: &Env) -> u64 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::AdminActionsDeadline)
        .expect("Please initialize admin_actions_deadline")
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
    e.storage()
        .instance()
        .get(&DataKey::TransferOwnershipDeadline)
        .expect("Please initialize transfer_ownership_deadline")
}

pub fn put_transfer_ownership_deadline(e: &Env, value: &u64) {
    bump_instance(e);
    e.storage()
        .instance()
        .set(&DataKey::TransferOwnershipDeadline, value);
}

// kill_deadline
pub fn get_kill_deadline(e: &Env) -> u64 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::KillDeadline)
        .expect("Please initialize kill_deadline")
}

pub fn put_kill_deadline(e: &Env, value: &u64) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::KillDeadline, value);
}

// is_killed
pub fn get_is_killed(e: &Env) -> bool {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::IsKilled)
        .expect("Please initialize is_killed")
}

pub fn put_is_killed(e: &Env, value: &bool) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::IsKilled, value);
}

pub(crate) fn set_plane(e: &Env, plane: &Address) {
    let key = DataKey::Plane;
    bump_instance(e);
    e.storage().instance().set(&key, plane);
}

pub(crate) fn get_plane(e: &Env) -> Address {
    let key = DataKey::Plane;
    e.storage()
        .instance()
        .get(&key)
        .expect("unable to get plane")
}

pub(crate) fn has_plane(e: &Env) -> bool {
    let key = DataKey::Plane;
    e.storage().instance().has(&key)
}
