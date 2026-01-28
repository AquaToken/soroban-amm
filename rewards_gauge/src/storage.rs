use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, Env, Vec, U256};
use utils::storage_errors::StorageError;
use utils::{
    generate_instance_storage_getter, generate_instance_storage_getter_and_setter,
    generate_instance_storage_setter,
};

// ------------------------------------
// Data Structures
// ------------------------------------

// Rewards configuration for a specific pool.
#[derive(Clone, Default)]
#[contracttype]
pub struct RewardConfig {
    pub start_at: u64,
    pub tps: u128,
    pub expired_at: u64,
}

// Mutable global reward data that evolves over time.
#[derive(Clone)]
#[contracttype]
pub struct GlobalRewardData {
    pub epoch: u64,
    pub inv: U256,
    pub accumulated: u128,
    pub claimed: u128,
}

// Per-user reward data.
#[derive(Clone)]
#[contracttype]
pub struct UserRewardData {
    pub epoch: u64,
    pub last_inv: U256,
    pub to_claim: u128,
}

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Pool,
    RewardToken,
    RewardConfigs,
    GlobalRewardData,

    // User-level data
    UserRewardData(Address),
}

generate_instance_storage_getter_and_setter!(pool, DataKey::Pool, Address);
generate_instance_storage_getter_and_setter!(reward_token, DataKey::RewardToken, Address);

pub(crate) fn get_reward_configs(env: &Env) -> Vec<RewardConfig> {
    env.storage()
        .instance()
        .get(&DataKey::RewardConfigs)
        .unwrap_or(Vec::new(env))
}

pub(crate) fn set_reward_configs(env: &Env, configs: Vec<RewardConfig>) {
    env.storage()
        .instance()
        .set(&DataKey::RewardConfigs, &configs);
}

pub(crate) fn set_global_reward_data(env: &Env, data: &GlobalRewardData) {
    env.storage()
        .instance()
        .set(&DataKey::GlobalRewardData, data);
}

pub(crate) fn get_global_reward_data(env: &Env) -> GlobalRewardData {
    env.storage()
        .instance()
        .get(&DataKey::GlobalRewardData)
        .unwrap_or(GlobalRewardData {
            epoch: 0,
            inv: U256::from_u128(env, 0),
            accumulated: 0,
            claimed: 0,
        })
}

pub(crate) fn set_user_reward_data(env: &Env, user: Address, data: &UserRewardData) {
    let key = DataKey::UserRewardData(user);
    env.storage().persistent().set(&key, data);
}

pub(crate) fn get_user_reward_data(env: &Env, user: Address) -> Option<UserRewardData> {
    let key = DataKey::UserRewardData(user);
    env.storage().persistent().get(&key)
}
