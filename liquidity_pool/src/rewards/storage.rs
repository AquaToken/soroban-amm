use crate::constants::{PERSISTENT_BUMP_AMOUNT, PERSISTENT_LIFETIME_THRESHOLD};
use crate::storage::{bump_persistent, DataKey};
use soroban_sdk::{contracttype, Address, Env, Map};

// Rewards configuration for specific pool
#[derive(Clone)]
#[contracttype]
pub struct PoolRewardConfig {
    pub tps: u128,
    pub expired_at: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct PoolRewardData {
    pub block: u64,
    pub accumulated: u128,
    pub last_time: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct UserRewardData {
    pub pool_accumulated: u128,
    pub to_claim: u128,
    pub last_block: u64,
}

// pool reward config
pub fn get_pool_reward_config(e: &Env) -> PoolRewardConfig {
    e.storage()
        .instance()
        .get(&DataKey::PoolRewardConfig)
        .unwrap()
}

pub fn set_pool_reward_config(e: &Env, config: &PoolRewardConfig) {
    e.storage()
        .instance()
        .set(&DataKey::PoolRewardConfig, config);
}

// pool reward data
pub fn get_pool_reward_data(e: &Env) -> PoolRewardData {
    e.storage()
        .instance()
        .get(&DataKey::PoolRewardData)
        .unwrap()
}

pub fn set_pool_reward_data(e: &Env, data: &PoolRewardData) {
    e.storage().instance().set(&DataKey::PoolRewardData, data);
}

// user reward data
pub fn bump_user_reward_data(e: &Env, user: &Address) {
    bump_persistent(e, &DataKey::UserRewardData(user.clone()));
}

pub fn get_user_reward_data(e: &Env, user: &Address) -> Option<UserRewardData> {
    match e
        .storage()
        .persistent()
        .get(&DataKey::UserRewardData(user.clone()))
    {
        Some(data) => data,
        None => None,
    }
}

pub fn set_user_reward_data(e: &Env, user: &Address, config: &UserRewardData) {
    e.storage()
        .persistent()
        .set(&DataKey::UserRewardData(user.clone()), config)
}

pub fn set_reward_inv_page(e: &Env, pow: u32, page_number: u64, value: &Map<u64, u64>) {
    let key = DataKey::RewardInvData(pow, page_number);
    e.storage().persistent().set(&key, value);
    e.storage()
        .persistent()
        .bump(&key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

pub fn get_reward_inv_page(e: &Env, pow: u32, page_number: u64) -> Map<u64, u64> {
    // println!("get_reward_inv_page: ({}, {})", pow, page_number);
    let key = DataKey::RewardInvData(pow, page_number);
    let reward_inv_data = e
        .storage()
        .persistent()
        .get(&key)
        .expect("unknown storage key");
    e.storage()
        .persistent()
        .bump(&key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
    reward_inv_data
}
