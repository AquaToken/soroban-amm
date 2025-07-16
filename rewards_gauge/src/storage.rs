use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, Env, U256};
use utils::bump::{bump_instance, bump_persistent};
use utils::storage_errors::StorageError;
use utils::{
    generate_instance_storage_getter, generate_instance_storage_getter_and_setter,
    generate_instance_storage_getter_and_setter_with_default,
    generate_instance_storage_getter_with_default, generate_instance_storage_setter,
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
    Operator,
    RewardToken,
    RewardConfig,
    FutureRewardConfig,
    GlobalRewardData,

    // User-level data
    UserRewardData(Address),
}

generate_instance_storage_getter_and_setter!(pool, DataKey::Pool, Address);
generate_instance_storage_getter_and_setter!(operator, DataKey::Operator, Address);
generate_instance_storage_getter_and_setter!(reward_token, DataKey::RewardToken, Address);
generate_instance_storage_getter_and_setter_with_default!(
    reward_config,
    DataKey::RewardConfig,
    RewardConfig,
    RewardConfig::default()
);
generate_instance_storage_getter_and_setter_with_default!(
    future_reward_config,
    DataKey::FutureRewardConfig,
    Option<RewardConfig>,
    None
);

pub(crate) fn set_global_reward_data(env: &Env, data: &GlobalRewardData) {
    bump_instance(env);
    env.storage().instance().set(&DataKey::GlobalRewardData, data);
}

pub(crate) fn get_global_reward_data(env: &Env) -> GlobalRewardData {
    bump_instance(env);
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
    bump_persistent(env, &key);
}

pub(crate) fn get_user_reward_data(env: &Env, user: Address) -> Option<UserRewardData> {
    let key = DataKey::UserRewardData(user);
    let data = env.storage().persistent().get(&key);
    if data.is_some() {
        bump_persistent(env, &key);
    }
    data
}
