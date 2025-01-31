use soroban_sdk::{contracttype, panic_with_error, Address, Env, Map, Vec};
use utils::bump::{bump_instance, bump_persistent};
use utils::storage_errors::StorageError;

// ------------------------------------
// Data Structures
// ------------------------------------

// Rewards configuration for a specific pool.
#[derive(Clone)]
#[contracttype]
pub struct PoolRewardConfig {
    pub tps: u128,
    pub expired_at: u64,
}

// Mutable pool reward data that evolves over time.
#[derive(Clone)]
#[contracttype]
pub struct PoolRewardData {
    pub block: u64,
    pub accumulated: u128,
    pub claimed: u128,
    pub last_time: u64,
}

// Per-user reward data.
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
    // Pool-level data
    PoolRewardConfig,
    PoolRewardData,

    // User-level data
    UserRewardData(Address),

    // Reward invariants (legacy + new)
    RewardInvData(u32, u64),   // legacy
    RewardInvDataV2(u32, u64), // new

    // Reward tokens & lock tokens
    RewardStorage,
    RewardToken,
    RewardBoostToken,
    RewardBoostFeed,

    // Working balances
    WorkingBalance(Address),
    WorkingSupply,
}

// ------------------------------------
// Core Storage Struct
// ------------------------------------

// Storage struct contains the environment and a local cache (`inv_cache`)
// to avoid repeated loading for reward invariants.
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
}

// ------------------------------------
// Sub-trait: Boost Token
// ------------------------------------

pub trait BoostTokenStorageTrait {
    fn get_reward_boost_token(&self) -> Address;
    fn put_reward_boost_token(&self, contract: Address);
    fn has_reward_boost_token(&self) -> bool;
}

impl BoostTokenStorageTrait for Storage {
    fn get_reward_boost_token(&self) -> Address {
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

    fn put_reward_boost_token(&self, contract: Address) {
        bump_instance(&self.env);
        self.env
            .storage()
            .instance()
            .set(&DataKey::RewardBoostToken, &contract);
    }

    fn has_reward_boost_token(&self) -> bool {
        self.env
            .storage()
            .instance()
            .has(&DataKey::RewardBoostToken)
    }
}

// ------------------------------------
// Sub-trait: Boost Feed
// ------------------------------------

pub trait BoostFeedStorageTrait {
    fn get_reward_boost_feed(&self) -> Address;
    fn put_reward_boost_feed(&self, contract: Address);
    fn has_reward_boost_feed(&self) -> bool;
}

impl BoostFeedStorageTrait for Storage {
    fn get_reward_boost_feed(&self) -> Address {
        match self.env.storage().instance().get(&DataKey::RewardBoostFeed) {
            Some(v) => v,
            None => panic_with_error!(self.env, StorageError::ValueNotInitialized),
        }
    }

    fn put_reward_boost_feed(&self, contract: Address) {
        bump_instance(&self.env);
        self.env
            .storage()
            .instance()
            .set(&DataKey::RewardBoostFeed, &contract);
    }

    fn has_reward_boost_feed(&self) -> bool {
        self.env.storage().instance().has(&DataKey::RewardBoostFeed)
    }
}

// ------------------------------------
// Sub-trait: Working Balances
// ------------------------------------

pub trait WorkingBalancesStorageTrait {
    fn get_working_balance(&self, user: &Address) -> u128;
    fn has_working_balance(&self, user: &Address) -> bool;
    fn set_working_balance(&self, user: &Address, value: u128);

    fn get_working_supply(&self) -> u128;
    fn set_working_supply(&self, value: u128);
    fn has_working_supply(&self) -> bool;
}

impl WorkingBalancesStorageTrait for Storage {
    fn get_working_balance(&self, user: &Address) -> u128 {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::WorkingBalance(user.clone()))
            .unwrap()
    }

    fn has_working_balance(&self, user: &Address) -> bool {
        self.env
            .storage()
            .persistent()
            .has(&DataKey::WorkingBalance(user.clone()))
    }

    fn set_working_balance(&self, user: &Address, value: u128) {
        let key = DataKey::WorkingBalance(user.clone());
        self.env.storage().persistent().set(&key, &value);
        bump_persistent(&self.env, &key);
    }

    fn get_working_supply(&self) -> u128 {
        self.env
            .storage()
            .instance()
            .get(&DataKey::WorkingSupply)
            .unwrap()
    }

    fn set_working_supply(&self, value: u128) {
        bump_instance(&self.env);
        self.env
            .storage()
            .instance()
            .set(&DataKey::WorkingSupply, &value);
    }

    fn has_working_supply(&self) -> bool {
        self.env.storage().instance().has(&DataKey::WorkingSupply)
    }
}

// ------------------------------------
// Sub-trait: Pool Rewards
// ------------------------------------

pub trait PoolRewardsStorageTrait {
    fn get_pool_reward_config(&self) -> PoolRewardConfig;
    fn set_pool_reward_config(&self, config: &PoolRewardConfig);

    fn get_pool_reward_data(&self) -> PoolRewardData;
    fn set_pool_reward_data(&self, data: &PoolRewardData);
}

impl PoolRewardsStorageTrait for Storage {
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
}

// ------------------------------------
// Sub-trait: User Rewards
// ------------------------------------

pub trait UserRewardsStorageTrait {
    fn get_user_reward_data(&self, user: &Address) -> Option<UserRewardData>;
    fn set_user_reward_data(&self, user: &Address, config: &UserRewardData);
    fn bump_user_reward_data(&self, user: &Address);
}

impl UserRewardsStorageTrait for Storage {
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
}

// ------------------------------------
// Sub-trait: Reward Invariants
// ------------------------------------

pub trait RewardInvDataStorageTrait {
    fn get_reward_inv_data(&mut self, pow: u32, page_number: u64) -> Vec<u128>;
    fn set_reward_inv_data(&mut self, pow: u32, page_number: u64, value: Vec<u128>);
}

impl RewardInvDataStorageTrait for Storage {
    fn get_reward_inv_data(&mut self, pow: u32, page_number: u64) -> Vec<u128> {
        let key = DataKey::RewardInvDataV2(pow, page_number);
        if let Some(cached) = self.inv_cache.get(key.clone()) {
            return cached;
        }

        let value = match self.env.storage().persistent().get::<_, Vec<u128>>(&key) {
            Some(v) => v,
            None => {
                // fallback to legacy key
                let key_old = DataKey::RewardInvData(pow, page_number);
                let old_result: Option<Map<u64, u128>> =
                    self.env.storage().persistent().get(&key_old);
                match old_result {
                    Some(legacy_map) => {
                        let mut new_vec = Vec::new(&self.env);
                        for (_, local_value) in legacy_map {
                            new_vec.push_back(local_value);
                        }
                        self.set_reward_inv_data(pow, page_number, new_vec.clone());
                        new_vec
                    }
                    None => return Vec::new(&self.env),
                }
            }
        };

        self.inv_cache.set(key, value.clone());
        value
    }

    fn set_reward_inv_data(&mut self, pow: u32, page_number: u64, value: Vec<u128>) {
        let key = DataKey::RewardInvDataV2(pow, page_number);
        self.inv_cache.set(key.clone(), value.clone());
        self.env.storage().persistent().set(&key, &value);
        bump_persistent(&self.env, &key);
    }
}

// ------------------------------------
// Sub-trait: Reward Token
// ------------------------------------

pub trait RewardTokenStorageTrait {
    fn get_reward_token(&self) -> Address;
    fn put_reward_token(&self, contract: Address);
    fn has_reward_token(&self) -> bool;
}

impl RewardTokenStorageTrait for Storage {
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
