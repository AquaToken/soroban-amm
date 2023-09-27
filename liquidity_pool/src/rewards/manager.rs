use crate::rewards::storage::{
    get_pool_reward_config, get_pool_reward_data, get_user_reward_data, set_pool_reward_data,
    set_user_reward_data, PoolRewardData, UserRewardData,
};
use crate::token::Client;
use crate::{storage, token};
use cast::i128;
use soroban_sdk::{Address, Env};

pub fn update_rewards_data(e: &Env) -> PoolRewardData {
    let config = get_pool_reward_config(e);
    let data = get_pool_reward_data(e);

    if data.last_time >= config.expired_at || config.tps == 0 {
        let new_data = PoolRewardData {
            accumulated: data.accumulated,
            last_time: e.ledger().timestamp(),
        };
        set_pool_reward_data(e, &new_data);

        return new_data;
    }

    let reward_timestamp = if e.ledger().timestamp() > config.expired_at {
        config.expired_at
    } else {
        e.ledger().timestamp()
    };

    let generated_tokens = i128(reward_timestamp - data.last_time) * i128(config.tps);
    let new_data = PoolRewardData {
        accumulated: data.accumulated + generated_tokens,
        last_time: e.ledger().timestamp(),
    };
    set_pool_reward_data(e, &new_data);
    new_data
}

pub fn update_user_reward(e: &Env, pool_data: &PoolRewardData, user: &Address) -> UserRewardData {
    if let Some(user_data) = get_user_reward_data(e, user) {
        if user_data.pool_accumulated == pool_data.accumulated {
            // nothing accumulated since last update
            return user_data;
        }

        let user_shares = token::get_user_balance_shares(e, user);
        if user_shares == 0 {
            // zero balance, no new reward
            let new_data = UserRewardData {
                pool_accumulated: pool_data.accumulated,
                to_claim: user_data.to_claim,
            };
            set_user_reward_data(e, user, &new_data);
            return new_data;
        }

        let total_shares = storage::get_total_shares(e);
        let new_reward =
            (pool_data.accumulated - user_data.pool_accumulated) * user_shares / total_shares;
        let new_data = UserRewardData {
            pool_accumulated: pool_data.accumulated,
            to_claim: user_data.to_claim + new_reward,
        };
        set_user_reward_data(e, user, &new_data);
        return new_data;
    } else {
        // user has joined
        let new_data = UserRewardData {
            pool_accumulated: pool_data.accumulated,
            to_claim: 0,
        };
        set_user_reward_data(e, user, &new_data);
        return new_data;
    }
}

pub fn claim_reward(e: &Env, user: &Address) -> i128 {
    // update pool data & calculate reward
    let pool_data = update_rewards_data(e);
    let user_reward = update_user_reward(e, &pool_data, user);
    let reward_amount = user_reward.to_claim;

    // transfer reward
    let reward_token = storage::get_reward_token(e);
    Client::new(e, &reward_token).transfer(&e.current_contract_address(), &user, &reward_amount);

    // set available reward to zero
    let new_data = UserRewardData {
        pool_accumulated: pool_data.accumulated,
        to_claim: 0,
    };
    set_user_reward_data(e, user, &new_data);

    reward_amount
}
