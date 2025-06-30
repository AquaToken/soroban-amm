use crate::errors::Error;
use crate::gauge::{checkpoint_global, checkpoint_user, sync_reward_global};
use crate::interface::UpgradeableContract;
use crate::storage::{
    get_operator, get_pool, get_reward_token, set_global_reward_data, set_operator, set_pool,
    set_reward_config, set_reward_token, set_user_reward_data, RewardConfig,
};
use crate::token_share::get_total_shares;
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

    pub fn set_rewards_config(e: Env, pool: Address, operator: Address, duration: u64, tps: u128) {
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
        let expired_at = e.ledger().timestamp() + duration;
        let working_supply = get_total_shares(&e, &get_pool(&e));
        checkpoint_global(&e, working_supply);
        sync_reward_global(&e);
        set_reward_config(&e, &RewardConfig { tps, expired_at })
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
