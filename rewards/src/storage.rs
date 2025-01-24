use crate::locker_feed::LockerFeedClient;
use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{contracttype, panic_with_error, Address, Env, Map, Vec};
use utils::bump::{bump_instance, bump_persistent};
use utils::storage_errors::StorageError;

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
    pub claimed: u128,
    pub last_time: u64,
}

#[derive(Clone)]
#[contracttype]
pub struct UserRewardData {
    pub pool_accumulated: u128,
    pub to_claim: u128,
    pub last_block: u64,
}

#[derive(Clone)]
#[contracttype]
enum DataKey {
    PoolRewardConfig,
    PoolRewardData,
    UserRewardData(Address),
    RewardInvData(u32, u64),
    RewardInvDataV2(u32, u64),
    RewardStorage,
    RewardToken,
    RewardBoostToken,
    RewardBoostFeed,
    WorkingBalance(Address),
    WorkingSupply,
}

pub struct Storage {
    env: Env,
    inv_cache: Map<DataKey, Vec<u128>>,
}

impl Storage {
    pub fn new(e: &Env) -> Storage {
        Storage {
            env: e.clone(),
            inv_cache: Map::new(e),
        }
    }

    // todo: split onto traits
    // Token locker
    pub fn get_reward_boost_token(&self) -> Address {
        match self
            .env
            .storage()
            .instance()
            .get(&DataKey::RewardBoostToken)
        {
            Some(v) => v,
            None => panic_with_error!(self.env, StorageError::ValueNotInitialized),
        }
    }

    pub fn put_reward_boost_token(&self, contract: Address) {
        bump_instance(&self.env);
        self.env
            .storage()
            .instance()
            .set(&DataKey::RewardBoostToken, &contract);
    }

    pub fn has_reward_boost_token(&self) -> bool {
        self.env
            .storage()
            .instance()
            .has(&DataKey::RewardBoostToken)
    }

    pub fn get_user_boost_balance(&self, user: &Address) -> u128 {
        match self.has_reward_boost_token() {
            true => {
                SorobanTokenClient::new(&self.env, &self.get_reward_boost_token()).balance(user)
                    as u128
            }
            false => 0,
        }
    }

    pub fn get_working_balance(&self, user: &Address) -> u128 {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::WorkingBalance(user.clone()))
            .unwrap()
    }

    pub fn has_working_balance(&self, user: &Address) -> bool {
        self.env
            .storage()
            .persistent()
            .has(&DataKey::WorkingBalance(user.clone()))
    }

    pub fn set_working_balance(&self, user: &Address, value: u128) {
        let key = DataKey::WorkingBalance(user.clone());
        self.env.storage().persistent().set(&key, &value);
        bump_persistent(&self.env, &key);
    }

    pub fn get_working_supply(&self) -> u128 {
        self.env
            .storage()
            .instance()
            .get(&DataKey::WorkingSupply)
            .unwrap()
    }

    pub fn set_working_supply(&self, value: u128) {
        bump_instance(&self.env);
        self.env
            .storage()
            .instance()
            .set(&DataKey::WorkingSupply, &value);
    }

    pub fn has_working_supply(&self) -> bool {
        self.env.storage().instance().has(&DataKey::WorkingSupply)
    }

    pub fn get_reward_boost_feed(&self) -> Address {
        match self.env.storage().instance().get(&DataKey::RewardBoostFeed) {
            Some(v) => v,
            None => panic_with_error!(self.env, StorageError::ValueNotInitialized),
        }
    }

    pub fn put_reward_boost_feed(&self, contract: Address) {
        bump_instance(&self.env);
        self.env
            .storage()
            .instance()
            .set(&DataKey::RewardBoostFeed, &contract);
    }

    fn has_reward_boost_feed(&self) -> bool {
        self.env.storage().instance().has(&DataKey::RewardBoostFeed)
    }

    pub fn get_total_locked(&self) -> u128 {
        match self.has_reward_boost_feed() {
            true => LockerFeedClient::new(&self.env, &self.get_reward_boost_feed()).total_supply(),
            false => 0,
        }
    }
}

pub trait RewardsStorageTrait {
    fn get_pool_reward_config(&self) -> PoolRewardConfig;
    fn set_pool_reward_config(&self, config: &PoolRewardConfig);

    fn get_pool_reward_data(&self) -> PoolRewardData;
    fn set_pool_reward_data(&self, data: &PoolRewardData);

    fn get_user_reward_data(&self, user: &Address) -> Option<UserRewardData>;
    fn set_user_reward_data(&self, user: &Address, config: &UserRewardData);
    fn bump_user_reward_data(&self, user: &Address);

    fn get_reward_inv_data(&mut self, pow: u32, page_number: u64) -> Vec<u128>;
    fn set_reward_inv_data(&mut self, pow: u32, page_number: u64, value: Vec<u128>);

    fn get_reward_token(&self) -> Address;
    fn put_reward_token(&self, contract: Address);
    fn has_reward_token(&self) -> bool;
}

impl RewardsStorageTrait for Storage {
    fn get_pool_reward_config(&self) -> PoolRewardConfig {
        match self
            .env
            .storage()
            .instance()
            .get(&DataKey::PoolRewardConfig)
        {
            Some(v) => v,
            None => PoolRewardConfig {
                tps: 0,
                expired_at: 0,
            },
        }
    }

    fn set_pool_reward_config(&self, config: &PoolRewardConfig) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::PoolRewardConfig, config);
    }

    fn get_pool_reward_data(&self) -> PoolRewardData {
        match self.env.storage().instance().get(&DataKey::PoolRewardData) {
            Some(v) => v,
            None => PoolRewardData {
                block: 0,
                accumulated: 0,
                claimed: 0,
                last_time: 0,
            },
        }
    }

    fn set_pool_reward_data(&self, data: &PoolRewardData) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::PoolRewardData, data);
    }

    fn get_user_reward_data(&self, user: &Address) -> Option<UserRewardData> {
        match self
            .env
            .storage()
            .persistent()
            .get(&DataKey::UserRewardData(user.clone()))
        {
            Some(data) => data,
            None => None,
        }
    }

    fn set_user_reward_data(&self, user: &Address, config: &UserRewardData) {
        self.env
            .storage()
            .persistent()
            .set(&DataKey::UserRewardData(user.clone()), config);
    }

    fn bump_user_reward_data(&self, user: &Address) {
        bump_persistent(&self.env, &DataKey::UserRewardData(user.clone()))
    }

    fn get_reward_inv_data(&mut self, pow: u32, page_number: u64) -> Vec<u128> {
        let key = DataKey::RewardInvDataV2(pow, page_number);
        let cached_value_result = self.inv_cache.get(key.clone());
        match cached_value_result {
            Some(value) => value,
            None => {
                let value = match self.env.storage().persistent().get(&key) {
                    Some(v) => v,
                    None => {
                        // try to find data using the legacy format
                        let key_old = DataKey::RewardInvData(pow, page_number);
                        let old_result: Option<Map<u64, u128>> =
                            self.env.storage().persistent().get(&key_old);
                        match old_result {
                            Some(legacy_value) => {
                                // legacy value exists - migrate Map<u64, u128> into Vec<u128>
                                let mut new_result = Vec::new(&self.env);
                                for (_k, local_value) in legacy_value {
                                    new_result.push_back(local_value);
                                }
                                self.set_reward_inv_data(pow, page_number, new_result.clone());
                                new_result
                            }
                            None => return Vec::new(&self.env),
                        }
                    }
                };
                self.inv_cache.set(key, value.clone());
                value
            }
        }
    }

    fn set_reward_inv_data(&mut self, pow: u32, page_number: u64, value: Vec<u128>) {
        let key = DataKey::RewardInvDataV2(pow, page_number);
        self.inv_cache.set(key.clone(), value.clone());
        self.env.storage().persistent().set(&key, &value);
        bump_persistent(&self.env, &key)
    }

    fn get_reward_token(&self) -> Address {
        match self.env.storage().instance().get(&DataKey::RewardToken) {
            Some(v) => v,
            None => panic_with_error!(self.env, StorageError::ValueNotInitialized),
        }
    }

    fn put_reward_token(&self, contract: Address) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::RewardToken, &contract);
    }

    fn has_reward_token(&self) -> bool {
        self.env.storage().instance().has(&DataKey::RewardToken)
    }
}
