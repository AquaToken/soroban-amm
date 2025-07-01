use crate::errors::Error;
use crate::gauge::{checkpoint_global, checkpoint_user, sync_reward_global};
use crate::interface::UpgradeableContract;
use crate::storage::{
    get_operator, get_pool, get_reward_config, get_reward_token, set_future_reward_config,
    set_global_reward_data, set_operator, set_pool, set_reward_config, set_reward_token,
    set_user_reward_data, RewardConfig,
};
use soroban_sdk::token::Client;
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env};

#[contract]
pub struct RewardsGauge;

#[contractimpl]
impl RewardsGauge {
    pub fn __constructor(e: Env, pool: Address, operator: Address, reward_token: Address) {
        set_pool(&e, &pool);
        set_operator(&e, &operator);
        set_reward_token(&e, &reward_token);
    }

    pub fn schedule_rewards_config(
        e: Env,
        pool: Address,
        operator: Address,
        start_at: Option<u64>,
        duration: u64,
        tps: u128,
        working_supply: u128,
    ) {
        pool.require_auth();
        if get_pool(&e) != pool {
            panic_with_error!(&e, Error::Unauthorized);
        }

        operator.require_auth();
        if get_operator(&e) != operator {
            panic_with_error!(&e, Error::Unauthorized);
        }

        if duration == 0 || tps == 0 {
            panic_with_error!(&e, Error::InvalidConfig);
        }

        let reward_token = Client::new(&e, &get_reward_token(&e));
        let new_reward = tps * duration as u128;
        reward_token.transfer(
            &operator,
            &e.current_contract_address(),
            &(new_reward as i128),
        );

        // checkpoint the global data before setting the new config
        checkpoint_global(&e, working_supply);
        let current_config = get_reward_config(&e);
        let now = e.ledger().timestamp();

        match start_at {
            Some(start_at) => {
                // if start_at is provided, it must be in the future
                if start_at < now {
                    panic_with_error!(&e, Error::StartTooEarly);
                }

                // don't allow overlap with existing config
                if start_at < current_config.expired_at {
                    panic_with_error!(&e, Error::StartTooEarly);
                }

                // schedule reward config to the future
                set_future_reward_config(
                    &e,
                    &Some(RewardConfig {
                        start_at,
                        tps,
                        expired_at: start_at + duration,
                    }),
                )
            }
            None => {
                // don't allow setting a new config if the current one is not expired
                if current_config.expired_at > now {
                    panic_with_error!(&e, Error::ConfigNotExpiredYet);
                }

                // force sync of global reward data up to now
                sync_reward_global(&e, now);
                set_reward_config(
                    &e,
                    &RewardConfig {
                        start_at: now,
                        expired_at: now + duration,
                        tps,
                    },
                )
            }
        };
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
            panic_with_error!(&e, Error::Unauthorized);
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
            panic_with_error!(&e, Error::Unauthorized);
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
        160
    }

    fn upgrade(e: Env, pool: Address, new_wasm_hash: BytesN<32>) {
        pool.require_auth();
        if get_pool(&e) != pool {
            panic_with_error!(&e, Error::Unauthorized);
        }

        e.deployer()
            .update_current_contract_wasm(new_wasm_hash.clone());
    }
}
