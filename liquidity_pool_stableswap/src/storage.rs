use crate::constants::{
    INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT,
    PERSISTENT_LIFETIME_THRESHOLD,
};
use crate::pool_constants::N_COINS;
use soroban_sdk::{contracttype, Address, Env, IntoVal, TryFromVal, Val, Vec};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Tokens,
    Reserves,
    RewardToken,
    RewardStorage,
    TokenShare,
    Admin,
    FutureAdmin,
    PoolRewardConfig,
    PoolRewardData,
    UserRewardData(Address),
    RewardInvData,
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
}

pub fn bump_instance(e: &Env) {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

pub fn bump_persistent<K>(e: &Env, key: &K)
where
    K: IntoVal<Env, Val>,
{
    e.storage()
        .persistent()
        .bump(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

fn get_instance_value<K, V>(e: &Env, key: &K) -> Option<V>
where
    K: IntoVal<Env, Val>,
    V: TryFromVal<Env, Val>,
{
    bump_instance(e);
    e.storage().instance().get(key)
}

fn put_instance_value<K, V>(e: &Env, key: &K, value: &V)
where
    K: IntoVal<Env, Val>,
    V: IntoVal<Env, Val>,
{
    bump_instance(e);
    e.storage().instance().set(key, value)
}

pub fn get_tokens(e: &Env) -> Vec<Address> {
    get_instance_value(e, &DataKey::Tokens).unwrap()
}

pub fn get_reward_token(e: &Env) -> Address {
    get_instance_value(e, &DataKey::RewardToken).unwrap()
}

pub fn get_reward_storage(e: &Env) -> Address {
    get_instance_value(e, &DataKey::RewardStorage).unwrap()
}

pub fn get_token_share(e: &Env) -> Address {
    get_instance_value(e, &DataKey::TokenShare).unwrap()
}

pub fn get_reserves(e: &Env) -> Vec<u128> {
    get_instance_value(e, &DataKey::Reserves).unwrap()
}

pub fn put_tokens(e: &Env, contracts: &Vec<Address>) {
    if contracts.len() != N_COINS as u32 {
        panic!("wrong vector size")
    }
    bump_instance(e);
    e.storage().instance().set(&DataKey::Tokens, contracts);
}

pub fn put_reward_token(e: &Env, contract: Address) {
    put_instance_value(e, &DataKey::RewardToken, &contract);
}

pub fn put_reward_storage(e: &Env, contract: Address) {
    put_instance_value(e, &DataKey::RewardStorage, &contract);
}

pub fn put_token_share(e: &Env, contract: Address) {
    put_instance_value(e, &DataKey::TokenShare, &contract);
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
    match get_instance_value(e, &DataKey::InitialA) {
        Some(value) => value,
        None => panic!("please initialize initial_A"),
    }
}

pub fn put_initial_a(e: &Env, value: u128) {
    put_instance_value(e, &DataKey::InitialA, &value);
}

// initial A time
pub fn get_initial_a_time(e: &Env) -> u64 {
    get_instance_value(e, &DataKey::InitialATime).unwrap()
}

pub fn put_initial_a_time(e: &Env, value: u64) {
    put_instance_value(e, &DataKey::InitialATime, &value);
}

// future_a
pub fn get_future_a(e: &Env) -> u128 {
    match get_instance_value(e, &DataKey::FutureA) {
        Some(value) => value,
        None => panic!("please initialize future_A"),
    }
}

pub fn put_future_a(e: &Env, value: u128) {
    put_instance_value(e, &DataKey::FutureA, &value);
}

// fitire A time
pub fn get_future_a_time(e: &Env) -> u64 {
    get_instance_value(e, &DataKey::FutureATime).unwrap()
}

pub fn put_future_a_time(e: &Env, value: u64) {
    put_instance_value(e, &DataKey::FutureATime, &value);
}

// fee
pub fn get_fee(e: &Env) -> u128 {
    match get_instance_value(e, &DataKey::Fee) {
        Some(value) => value,
        None => panic!("please initialize fee"),
    }
}

pub fn put_fee(e: &Env, value: u128) {
    put_instance_value(e, &DataKey::Fee, &value);
}

// admin_fee
pub fn get_admin_fee(e: &Env) -> u128 {
    match get_instance_value(e, &DataKey::AdminFee) {
        Some(value) => value,
        None => panic!("please initialize admin_fee"),
    }
}

pub fn put_admin_fee(e: &Env, value: u128) {
    put_instance_value(e, &DataKey::AdminFee, &value);
}

// future_fee
pub fn get_future_fee(e: &Env) -> u128 {
    match get_instance_value(e, &DataKey::FutureFee) {
        Some(value) => value,
        None => panic!("please initialize future_fee"),
    }
}

pub fn put_future_fee(e: &Env, value: u128) {
    put_instance_value(e, &DataKey::FutureFee, &value);
}

// future_admin_fee
pub fn get_future_admin_fee(e: &Env) -> u128 {
    match get_instance_value(e, &DataKey::FutureAdminFee) {
        Some(value) => value,
        None => panic!("please initialize future_admin_fee"),
    }
}

pub fn put_future_admin_fee(e: &Env, value: u128) {
    put_instance_value(e, &DataKey::FutureAdminFee, &value);
}

// admin_actions_deadline
pub fn get_admin_actions_deadline(e: &Env) -> u64 {
    match get_instance_value(e, &DataKey::AdminActionsDeadline) {
        Some(value) => value,
        None => panic!("please initialize admin_actions_deadline"),
    }
}

pub fn put_admin_actions_deadline(e: &Env, value: u64) {
    put_instance_value(e, &DataKey::AdminActionsDeadline, &value);
}

// transfer_ownership_deadline
pub fn get_transfer_ownership_deadline(e: &Env) -> u64 {
    match get_instance_value(e, &DataKey::TransferOwnershipDeadline) {
        Some(value) => value,
        None => panic!("please initialize transfer_ownership_deadline"),
    }
}

pub fn put_transfer_ownership_deadline(e: &Env, value: u64) {
    put_instance_value(e, &DataKey::TransferOwnershipDeadline, &value);
}

// kill_deadline
pub fn get_kill_deadline(e: &Env) -> u64 {
    match get_instance_value(e, &DataKey::KillDeadline) {
        Some(value) => value,
        None => panic!("please initialize kill_deadline"),
    }
}

pub fn put_kill_deadline(e: &Env, value: u64) {
    put_instance_value(e, &DataKey::KillDeadline, &value);
}

// is_killed
pub fn get_is_killed(e: &Env) -> bool {
    match get_instance_value(e, &DataKey::IsKilled) {
        Some(value) => value,
        None => panic!("please initialize is_killed"),
    }
}

pub fn put_is_killed(e: &Env, value: bool) {
    put_instance_value(e, &DataKey::IsKilled, &value);
}
