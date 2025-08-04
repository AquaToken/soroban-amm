use crate::constants::MAX_REWARD_CONFIGS;
use crate::errors::GaugeError;
use crate::gauge::{checkpoint_global, checkpoint_user};
use crate::interface::UpgradeableContract;
use crate::storage::{
    get_global_reward_data, get_pool, get_reward_configs, get_reward_token, set_global_reward_data,
    set_pool, set_reward_configs, set_reward_token, set_user_reward_data, RewardConfig,
};
use soroban_sdk::token::Client;
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Vec};

#[contract]
pub struct RewardsGauge;

#[contractimpl]
impl RewardsGauge {
    pub fn __constructor(e: Env, pool: Address, reward_token: Address) {
        set_pool(&e, &pool);
        set_reward_token(&e, &reward_token);
    }

    pub fn schedule_rewards_config(
        e: Env,
        pool: Address,
        distributor: Address,
        start_at: Option<u64>,
        duration: u64,
        tps: u128,
        working_supply: u128,
    ) {
        pool.require_auth();
        distributor.require_auth();

        if get_pool(&e) != pool {
            panic_with_error!(&e, GaugeError::Unauthorized);
        }

        if duration == 0 || tps == 0 {
            panic_with_error!(&e, GaugeError::InvalidConfig);
        }

        let reward_token = Client::new(&e, &get_reward_token(&e));
        let new_reward = tps * duration as u128;
        reward_token.transfer(
            &distributor,
            &e.current_contract_address(),
            &(new_reward as i128),
        );

        // checkpoint the global data before setting the new config
        checkpoint_global(&e, working_supply);
        let mut current_configs = get_reward_configs(&e);

        let now = e.ledger().timestamp();
        let config_start_at = start_at.unwrap_or(now);

        // if start_at is provided, it must be in the future
        if config_start_at < now {
            panic_with_error!(&e, GaugeError::StartTooEarly);
        }

        let new_config = RewardConfig {
            start_at: config_start_at,
            expired_at: config_start_at + duration,
            tps,
        };
        current_configs.push_back(new_config);
        if current_configs.len() > MAX_REWARD_CONFIGS {
            panic_with_error!(&e, GaugeError::TooManyConfigs);
        }
        set_reward_configs(&e, current_configs);
    }

    pub fn checkpoint_user(
        e: Env,
        pool: Address,
        user: Address,
        working_balance: u128,
        working_supply: u128,
    ) {
        pool.require_auth();
        if get_pool(&e) != pool {
            panic_with_error!(&e, GaugeError::Unauthorized);
        }

        let global_data = checkpoint_global(&e, working_supply);
        checkpoint_user(&e, &global_data, &user, working_balance);
    }

    pub fn claim(
        e: Env,
        pool: Address,
        user: Address,
        working_balance: u128,
        working_supply: u128,
    ) -> u128 {
        pool.require_auth();
        if get_pool(&e) != pool {
            panic_with_error!(&e, GaugeError::Unauthorized);
        }

        let mut global_data = checkpoint_global(&e, working_supply);
        let mut user_data = checkpoint_user(&e, &global_data, &user, working_balance);

        let user_reward = user_data.to_claim;
        user_data.to_claim = 0;
        global_data.claimed += user_reward;
        set_global_reward_data(&e, &global_data);
        set_user_reward_data(&e, user.clone(), &user_data);

        // Transfer tokens
        let reward_token = get_reward_token(&e);
        Client::new(&e, &reward_token).transfer(
            &e.current_contract_address(),
            &user,
            &(user_reward as i128),
        );

        user_reward
    }

    pub fn get_user_reward(
        e: Env,
        pool: Address,
        user: Address,
        working_balance: u128,
        working_supply: u128,
    ) -> u128 {
        pool.require_auth();
        if get_pool(&e) != pool {
            panic_with_error!(&e, GaugeError::Unauthorized);
        }

        let global_data = checkpoint_global(&e, working_supply);
        let user_data = checkpoint_user(&e, &global_data, &user, working_balance);
        user_data.to_claim
    }

    pub fn get_reward_token(e: Env) -> Address {
        get_reward_token(&e)
    }

    pub fn get_reward_configs(e: Env) -> Vec<RewardConfig> {
        let now = e.ledger().timestamp();
        let mut current_configs = Vec::new(&e);
        for reward_config in get_reward_configs(&e) {
            if reward_config.expired_at >= now {
                current_configs.push_back(reward_config);
            }
        }
        current_configs
    }

    pub fn get_reward_config(e: Env) -> RewardConfig {
        let now = e.ledger().timestamp();
        let mut aggregated_config = RewardConfig {
            start_at: now,
            expired_at: 0,
            tps: 0,
        };
        for config in get_reward_configs(&e) {
            if config.start_at <= now && config.expired_at > now {
                aggregated_config.tps += config.tps;
                if aggregated_config.expired_at == 0
                    || aggregated_config.expired_at > config.expired_at
                {
                    aggregated_config.expired_at = config.expired_at;
                }
            }
        }
        aggregated_config
    }
}

// The `UpgradeableContract` trait provides the interface for upgrading the contract.
// This contract has no delayed upgrade. Liquidity Pool contract handles the upgrade delay.
#[contractimpl]
impl UpgradeableContract for RewardsGauge {
    // Returns the version of the contract.
    //
    // # Returns
    //
    // The version of the contract as a u32.
    fn version() -> u32 {
        170
    }

    fn upgrade(e: Env, pool: Address, new_wasm_hash: BytesN<32>) {
        pool.require_auth();
        if get_pool(&e) != pool {
            panic_with_error!(&e, GaugeError::Unauthorized);
        }

        e.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());
    }
}
