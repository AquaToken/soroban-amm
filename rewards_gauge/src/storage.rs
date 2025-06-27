use paste::paste;
use soroban_sdk::{contracttype, log, panic_with_error, Address, Env};
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
#[derive(Clone)]
#[contracttype]
pub struct RewardConfig {
    pub tps: u128,
    pub expired_at: u64,
}

// Mutable pool reward data that evolves over time.
#[derive(Clone)]
#[contracttype]
pub struct GlobalRewardData {
    pub epoch: u64,
    pub accumulated: u128,
    pub claimed: u128,
}

// Per-user reward data.
#[derive(Clone)]
#[contracttype]
pub struct UserRewardData {
    pub epoch: u64,
    pub to_claim: u128,
}

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Pool,
    RewardToken,
    RewardConfig,
    GlobalRewardData,

    // User-level data
    UserRewardData(Address),

    // Global reward cumulative invariant per 1 share. key is the timestamp
    RewardInvData(u64),
}

generate_instance_storage_getter_and_setter!(pool, DataKey::Pool, Address);
generate_instance_storage_getter_and_setter!(reward_token, DataKey::RewardToken, Address);
generate_instance_storage_getter_and_setter_with_default!(
    reward_config,
    DataKey::RewardConfig,
    RewardConfig,
    RewardConfig {
        tps: 0,
        expired_at: 0,
    }
);
generate_instance_storage_getter_and_setter_with_default!(
    global_reward_data,
    DataKey::GlobalRewardData,
    GlobalRewardData,
    GlobalRewardData {
        epoch: 0,
        accumulated: 0,
        claimed: 0,
    }
);

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

pub(crate) fn set_reward_inv_data(env: &Env, epoch: u64, inv: u128) {
    log!(env, "inv({}): {}", epoch, inv);
    let key = DataKey::RewardInvData(epoch);
    env.storage().persistent().set(&key, &inv);
    bump_persistent(env, &key);
}

pub(crate) fn get_reward_inv_data(env: &Env, epoch: u64) -> u128 {
    let key = DataKey::RewardInvData(epoch);
    match env.storage().persistent().get(&key) {
        Some(inv) => {
            bump_persistent(env, &key);
            inv
        }
        None => {
            panic_with_error!(env, StorageError::ValueMissing);
        }
    }
}
