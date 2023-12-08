#![no_std]

use access_control::access::{AccessControl, AccessControlTrait};
use soroban_sdk::{symbol_short, Address, Env, Map, Symbol};

mod constants;
pub mod manager;
pub mod storage;

use crate::constants::PAGE_SIZE;
use crate::storage::{PoolRewardConfig, RewardsStorageTrait, Storage};
use cast::i128 as to_i128;
pub use manager::Manager;
use token_share::{get_total_shares, get_user_balance_shares};
pub use utils;
use utils::bump::bump_instance;

#[derive(Clone)]
pub struct RewardsConfig {
    page_size: u64,
}

#[derive(Clone)]
pub struct Rewards {
    env: Env,
    config: RewardsConfig,
}

impl Rewards {
    #[inline(always)]
    pub fn new(env: &Env, page_size: u64) -> Rewards {
        Rewards {
            env: env.clone(),
            config: RewardsConfig { page_size },
        }
    }

    pub fn storage(&self) -> Storage {
        Storage::new(&self.env)
    }

    pub fn manager(&self) -> Manager {
        Manager::new(&self.env, self.storage(), &self.config)
    }
}

pub fn get_rewards_manager(e: &Env) -> Rewards {
    Rewards::new(&e, PAGE_SIZE)
}

pub trait RewardsTrait {
    fn initialize_rewards_config(e: Env, reward_token: Address, reward_storage: Address) -> bool {
        let rewards = Rewards::new(&e, PAGE_SIZE);
        if rewards.storage().has_reward_token() {
            panic!("rewards config already initialized")
        }
        rewards.storage().put_reward_token(reward_token);
        rewards.storage().put_reward_storage(reward_storage);
        true
    }

    fn set_rewards_config(
        e: Env,
        admin: Address,
        expired_at: u64, // timestamp
        tps: u128,       // value with 7 decimal places. example: 600_0000000
    ) -> bool {
        admin.require_auth();
        AccessControl::new(&e).check_admin(&admin);

        if expired_at < e.ledger().timestamp() {
            panic!("cannot set expiration time to the past");
        }

        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        rewards.manager().update_rewards_data(total_shares);

        let config = PoolRewardConfig { tps, expired_at };
        bump_instance(&e);
        rewards.storage().set_pool_reward_config(&config);
        true
    }

    fn get_rewards_info(e: Env, user: Address) -> Map<Symbol, i128> {
        let rewards = get_rewards_manager(&e);
        let config = rewards.storage().get_pool_reward_config();
        let total_shares = get_total_shares(&e);
        let pool_data = rewards.manager().update_rewards_data(total_shares);
        let user_shares = get_user_balance_shares(&e, &user);
        let user_data = rewards
            .manager()
            .update_user_reward(&pool_data, &user, user_shares);
        let mut result = Map::new(&e);
        result.set(symbol_short!("tps"), to_i128(config.tps).unwrap());
        result.set(symbol_short!("exp_at"), to_i128(config.expired_at));
        result.set(
            symbol_short!("acc"),
            to_i128(pool_data.accumulated).unwrap(),
        );
        result.set(symbol_short!("last_time"), to_i128(pool_data.last_time));
        result.set(
            symbol_short!("pool_acc"),
            to_i128(user_data.pool_accumulated).unwrap(),
        );
        result.set(symbol_short!("block"), to_i128(pool_data.block));
        result.set(symbol_short!("usr_block"), to_i128(user_data.last_block));
        result.set(
            symbol_short!("to_claim"),
            to_i128(user_data.to_claim).unwrap(),
        );
        result
    }

    fn get_user_reward(e: Env, user: Address) -> u128 {
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        rewards
            .manager()
            .get_amount_to_claim(&user, total_shares, user_shares)
    }

    fn claim(e: Env, user: Address) -> u128 {
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        let reward = rewards
            .manager()
            .claim_reward(&user, total_shares, user_shares);
        rewards.storage().bump_user_reward_data(&user);
        reward
    }
}
