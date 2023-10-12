use crate::constants::{PERSISTENT_BUMP_AMOUNT, PERSISTENT_LIFETIME_THRESHOLD};
use crate::rewards::storage::{
    get_pool_reward_config, get_pool_reward_data, get_user_reward_data, set_pool_reward_data,
    set_user_reward_data, PoolRewardData, UserRewardData,
};
use crate::storage::{get_reward_storage, DataKey};
use crate::token::Client;
use crate::{storage, token};
use cast::i128 as to_i128;
use soroban_sdk::{Address, Env, Map};

pub fn update_reward_inv(e: &Env, accumulated: i128) {
    let total_shares = token::get_total_shares(e);
    let reward_per_share = if total_shares > 0 {
        accumulated / total_shares
    } else {
        0
    };

    let data = get_pool_reward_data(e);
    add_reward_inv(e, data.block, reward_per_share as u64);
}

pub fn add_reward_inv(e: &Env, block: u64, value: u64) {
    // todo: optimize map key/value size
    let mut reward_inv_data: Map<u64, u64> = e
        .storage()
        .persistent()
        .get(&DataKey::RewardInvData)
        .unwrap();
    reward_inv_data.set(block, value);
    set_reward_inv(e, &reward_inv_data);
    e.storage().persistent().bump(
        &DataKey::RewardInvData,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
}

pub fn set_reward_inv(e: &Env, value: &Map<u64, u64>) {
    e.storage().persistent().set(&DataKey::RewardInvData, value);
    e.storage().persistent().bump(
        &DataKey::RewardInvData,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
}

pub fn get_reward_inv(e: &Env) -> Map<u64, u64> {
    // todo: optimize memory usage
    // todo: do we need default here?
    let reward_inv_data = e
        .storage()
        .persistent()
        .get(&DataKey::RewardInvData)
        .unwrap();
    e.storage().persistent().bump(
        &DataKey::RewardInvData,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
    reward_inv_data
}

pub fn update_rewards_data(e: &Env) -> PoolRewardData {
    let config = get_pool_reward_config(e);
    let data = get_pool_reward_data(e);
    let now = e.ledger().timestamp();

    // 1. config not expired - snapshot reward
    // 2. config expired
    //  2.a data before config expiration - snapshot reward for now, increase block and generate inv
    //  2.b data after config expiration - snapshot reward for config end, increase block, snapshot reward for now, don't increase block

    return if now < config.expired_at {
        let reward_timestamp = now;

        let generated_tokens = to_i128(reward_timestamp - data.last_time) * to_i128(config.tps);
        let new_data = PoolRewardData {
            block: data.block + 1,
            accumulated: data.accumulated + generated_tokens,
            last_time: now,
        };
        set_pool_reward_data(e, &new_data);
        update_reward_inv(e, generated_tokens);
        new_data
    } else {
        if data.last_time > config.expired_at {
            // todo: don't increase block
            let new_data = PoolRewardData {
                block: data.block + 1,
                accumulated: data.accumulated,
                last_time: now,
            };
            set_pool_reward_data(e, &new_data);
            update_reward_inv(e, 0);
            new_data
        } else {
            // catchup up to config expiration
            let reward_timestamp = config.expired_at;

            let generated_tokens = to_i128(reward_timestamp - data.last_time) * to_i128(config.tps);
            let catchup_data = PoolRewardData {
                block: data.block + 1,
                accumulated: data.accumulated + generated_tokens,
                last_time: config.expired_at,
            };
            set_pool_reward_data(e, &catchup_data);
            update_reward_inv(e, generated_tokens);

            // todo: don't increase block when config not enabled thus keeping invariants list small
            let new_data = PoolRewardData {
                block: catchup_data.block + 1,
                accumulated: catchup_data.accumulated,
                last_time: now,
            };
            set_pool_reward_data(e, &new_data);
            update_reward_inv(e, 0);
            new_data
        }
    };
}

pub fn calculate_user_reward(
    e: &Env,
    start_block: u64,
    end_block: u64,
    user_share: i128,
) -> i128 {
    let mut reward_inv = 0;
    for block in start_block..end_block + 1 {
        let block_inv = get_reward_inv(e).get(block).unwrap();
        reward_inv += block_inv;
    }
    (reward_inv) as i128 * user_share
}

pub fn update_user_reward(e: &Env, pool_data: &PoolRewardData, user: &Address) -> UserRewardData {
    return if let Some(user_data) = get_user_reward_data(e, user) {
        if user_data.pool_accumulated == pool_data.accumulated {
            // nothing accumulated since last update
            return user_data;
        }

        let user_shares = token::get_user_balance_shares(e, user);
        if user_shares == 0 {
            // zero balance, no new reward
            let new_data = UserRewardData {
                last_block: pool_data.block,
                pool_accumulated: pool_data.accumulated,
                to_claim: user_data.to_claim,
            };
            set_user_reward_data(e, user, &new_data);
            return new_data;
        }

        let reward = calculate_user_reward(
            e,
            user_data.last_block + 1,
            pool_data.block,
            user_shares,
        );
        // let new_reward =
        //     (pool_data.accumulated - user_data.pool_accumulated) * user_shares / total_shares;
        let new_data = UserRewardData {
            last_block: pool_data.block,
            pool_accumulated: pool_data.accumulated,
            to_claim: user_data.to_claim + reward,
        };
        set_user_reward_data(e, user, &new_data);
        new_data
    } else {
        // user has joined
        let new_data = UserRewardData {
            last_block: pool_data.block,
            pool_accumulated: pool_data.accumulated,
            to_claim: 0,
        };
        set_user_reward_data(e, user, &new_data);
        new_data
    };
}

pub fn get_amount_to_claim(e: &Env, user: &Address) -> i128 {
    // update pool data & calculate reward
    let pool_data = update_rewards_data(e);
    let user_reward = update_user_reward(e, &pool_data, user);
    user_reward.to_claim
}

pub fn claim_reward(e: &Env, user: &Address) -> i128 {
    // update pool data & calculate reward
    let pool_data = update_rewards_data(e);
    let user_reward = update_user_reward(e, &pool_data, user);
    let reward_amount = user_reward.to_claim;

    // transfer reward
    let reward_token = storage::get_reward_token(e);
    Client::new(e, &reward_token).transfer_from(
        &e.current_contract_address(),
        &get_reward_storage(e),
        &user,
        &reward_amount,
    );

    // set available reward to zero
    let new_data = UserRewardData {
        last_block: pool_data.block,
        pool_accumulated: pool_data.accumulated,
        to_claim: 0,
    };
    set_user_reward_data(e, user, &new_data);

    reward_amount
}
