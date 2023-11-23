use soroban_sdk::{contracttype, Address, Env, Map};
use utils::constant::{PERSISTENT_BUMP_AMOUNT, PERSISTENT_LIFETIME_THRESHOLD};

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

#[derive(Clone)]
#[contracttype]
enum DataKey {
    PoolRewardConfig,
    PoolRewardData,
    UserRewardData(Address),
    RewardInvData(u32, u64),
    RewardStorage,
    RewardToken,
}

pub struct Storage {
    env: Env,
}

impl Storage {
    pub fn new(e: &Env) -> Storage {
        Storage { env: e.clone() }
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

    fn get_reward_inv_data(&self, pow: u32, page_number: u64) -> Map<u64, u64>;
    fn set_reward_inv_data(&self, pow: u32, page_number: u64, value: &Map<u64, u64>);
    fn bump_reward_inv_data(&self, pow: u32, page_number: u64);

    fn get_reward_storage(&self) -> Address;
    fn put_reward_storage(&self, contract: Address);

    fn get_reward_token(&self) -> Address;
    fn put_reward_token(&self, contract: Address);
    fn has_reward_token(&self) -> bool;
}

impl RewardsStorageTrait for Storage {
    fn get_pool_reward_config(&self) -> PoolRewardConfig {
        self.env
            .storage()
            .instance()
            .get(&DataKey::PoolRewardConfig)
            .expect("Please, initialize pool reward config")
    }

    fn set_pool_reward_config(&self, config: &PoolRewardConfig) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::PoolRewardConfig, config);
    }

    fn get_pool_reward_data(&self) -> PoolRewardData {
        self.env
            .storage()
            .instance()
            .get(&DataKey::PoolRewardData)
            .expect("Please, initialize pool reward data")
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
        self.env.storage().persistent().bump(
            &DataKey::UserRewardData(user.clone()),
            PERSISTENT_LIFETIME_THRESHOLD,
            PERSISTENT_BUMP_AMOUNT,
        );
    }

    fn get_reward_inv_data(&self, pow: u32, page_number: u64) -> Map<u64, u64> {
        self.env
            .storage()
            .persistent()
            .get(&DataKey::RewardInvData(pow, page_number))
            .expect("Please, initialize reward inv data")
    }

    fn set_reward_inv_data(&self, pow: u32, page_number: u64, value: &Map<u64, u64>) {
        self.env
            .storage()
            .persistent()
            .set(&DataKey::RewardInvData(pow, page_number), value);
        self.bump_reward_inv_data(pow, page_number); // when set need bump
    }

    fn bump_reward_inv_data(&self, pow: u32, page_number: u64) {
        self.env.storage().persistent().bump(
            &DataKey::RewardInvData(pow, page_number),
            PERSISTENT_LIFETIME_THRESHOLD,
            PERSISTENT_BUMP_AMOUNT,
        );
    }

    fn get_reward_storage(&self) -> Address {
        self.env
            .storage()
            .instance()
            .get(&DataKey::RewardStorage)
            .expect("Trying to get reward storage")
    }

    fn get_reward_token(&self) -> Address {
        self.env
            .storage()
            .instance()
            .get(&DataKey::RewardToken)
            .expect("Trying to get reward token")
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

    fn put_reward_storage(&self, contract: Address) {
        self.env
            .storage()
            .instance()
            .set(&DataKey::RewardStorage, &contract)
    }
}
