use crate::gauge::{checkpoint_global, checkpoint_user, sync_reward_global};
use crate::storage::{
    get_pool, get_reward_token, set_global_reward_data, set_pool, set_reward_config,
    set_reward_inv_data, set_reward_token, set_user_reward_data, RewardConfig,
};
// use crate::interface::AdminInterfaceTrait;
// use access_control::access::AccessControlTrait;
// use access_control::errors::AccessControlError;
// use access_control::interface::TransferableContract;
// use access_control::management::SingleAddressManagementTrait;
// use access_control::role::SymbolRepresentation;
// use access_control::transfer::TransferOwnershipTrait;
use soroban_sdk::token::Client;
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, Env};
// use upgrade::interface::UpgradeableContract;
use crate::errors::Error;
use crate::token_share::get_total_shares;

#[contract]
pub struct RewardsGauge;

#[contractimpl]
impl RewardsGauge {
    pub fn __constructor(e: Env, pool: Address, reward_token: Address) {
        set_pool(&e, &pool);
        set_reward_token(&e, &reward_token);

        set_reward_inv_data(&e, 0, 0);
    }

    pub fn set_rewards_config(e: Env, invoker: Address, duration: u64, tps: u128) {
        // todo: check if invoker is authorized to set rewards config
        // assert duration is not zero
        invoker.require_auth();

        let reward_token = Client::new(&e, &get_reward_token(&e));
        let new_reward = tps * duration as u128;
        reward_token.transfer(
            &invoker,
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
