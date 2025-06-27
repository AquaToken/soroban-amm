use crate::constants::REWARD_PRECISION;
use crate::storage::{
    get_global_reward_data, get_reward_config, get_reward_inv_data, get_user_reward_data,
    set_global_reward_data, set_reward_inv_data, set_user_reward_data, GlobalRewardData,
    UserRewardData,
};
use soroban_sdk::{Address, Env};

pub(crate) fn checkpoint_global(env: &Env, working_supply: u128) -> GlobalRewardData {
    let config = get_reward_config(env);
    let data = get_global_reward_data(env);
    let now = env.ledger().timestamp();

    if now <= config.expired_at {
        // config not expired yet, yield rewards
        let generated_tokens = (now - data.epoch) as u128 * config.tps;
        let reward_per_share = if working_supply > 0 {
            REWARD_PRECISION * generated_tokens / working_supply
        } else {
            0
        };
        let prev_reward_inv = get_reward_inv_data(env, data.epoch);

        let new_data = GlobalRewardData {
            epoch: now,
            accumulated: data.accumulated + generated_tokens,
            claimed: data.claimed,
        };
        set_global_reward_data(env, &new_data);
        let new_reward_inv = prev_reward_inv + reward_per_share;
        set_reward_inv_data(env, new_data.epoch, new_reward_inv);
        new_data
    } else {
        // Already expired
        if data.epoch < config.expired_at {
            // last snapshot was before config expiration - yield up to expiration
            let generated_tokens = (config.expired_at - data.epoch) as u128 * config.tps;
            let reward_per_share = if working_supply > 0 {
                REWARD_PRECISION * generated_tokens / working_supply
            } else {
                0
            };
            let prev_reward_inv = get_reward_inv_data(env, data.epoch);
            let new_data = GlobalRewardData {
                epoch: config.expired_at,
                accumulated: data.accumulated + generated_tokens,
                claimed: data.claimed,
            };
            set_global_reward_data(env, &new_data);
            let new_reward_inv = prev_reward_inv + reward_per_share;
            set_reward_inv_data(env, new_data.epoch, new_reward_inv);
            new_data
        } else {
            // already expired, no new rewards
            data
        }
    }
}

pub(crate) fn sync_reward_global(env: &Env) -> GlobalRewardData {
    let data = get_global_reward_data(env);
    let now = env.ledger().timestamp();

    if data.epoch == now {
        // snapshot already made
        data
    } else {
        let new_data = GlobalRewardData {
            epoch: now,
            accumulated: data.accumulated,
            claimed: data.claimed,
        };
        set_global_reward_data(env, &new_data);
        set_reward_inv_data(env, new_data.epoch, get_reward_inv_data(env, data.epoch));
        new_data
    }
}

pub(crate) fn checkpoint_user(
    env: &Env,
    global_data: &GlobalRewardData,
    user: &Address,
    working_balance: u128,
) -> UserRewardData {
    if let Some(user_data) = get_user_reward_data(env, user.clone()) {
        // If no new accumulation, just return
        if user_data.epoch == global_data.epoch {
            return user_data;
        }

        if working_balance == 0 {
            // No new reward
            let new_data = UserRewardData {
                epoch: global_data.epoch,
                to_claim: user_data.to_claim,
            };
            set_user_reward_data(env, user.clone(), &new_data);
            return new_data;
        }

        let current_inv = get_reward_inv_data(env, global_data.epoch);
        let prev_inv = get_reward_inv_data(env, user_data.epoch);
        let reward = working_balance * (current_inv - prev_inv) / REWARD_PRECISION;
        let new_data = UserRewardData {
            epoch: global_data.epoch,
            to_claim: user_data.to_claim + reward,
        };
        set_user_reward_data(env, user.clone(), &new_data);
        new_data
    } else {
        let new_data = UserRewardData {
            epoch: global_data.epoch,
            to_claim: 0,
        };
        set_user_reward_data(env, user.clone(), &new_data);
        new_data
    }
}
