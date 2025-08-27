use crate::constants::REWARD_PRECISION;
use crate::storage::{
    get_global_reward_data, get_reward_configs, get_user_reward_data, set_global_reward_data,
    set_reward_configs, set_user_reward_data, GlobalRewardData, UserRewardData,
};
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::{Address, Env, Vec, U256};

pub(crate) fn checkpoint_global(env: &Env, working_supply: u128) -> GlobalRewardData {
    let configs = get_reward_configs(env);
    let mut configs_updated = Vec::new(env);
    let start_data = get_global_reward_data(env);
    let now = env.ledger().timestamp();

    let mut new_data = start_data.clone();
    new_data.epoch = now;

    for config in configs {
        if config.start_at > now {
            // Config not started yet, so no yield generated. skip
            configs_updated.push_back(config);
            continue;
        }

        let reward_start = if config.start_at < start_data.epoch {
            start_data.epoch
        } else {
            config.start_at
        };
        let reward_end = if config.expired_at > now {
            now
        } else {
            config.expired_at
        };

        let generated_tokens = (reward_end - reward_start) as u128 * config.tps;
        let reward_per_share = if working_supply > 0 {
            generated_tokens.fixed_mul_floor(&env, &REWARD_PRECISION, &working_supply)
        } else {
            0
        };

        // store only active or future configs
        if config.expired_at > now {
            configs_updated.push_back(config);
        }
        new_data.inv = new_data.inv.add(&U256::from_u128(env, reward_per_share));
        new_data.accumulated += generated_tokens;
    }
    set_global_reward_data(env, &new_data);
    set_reward_configs(env, configs_updated);
    new_data
}

pub(crate) fn checkpoint_user(
    env: &Env,
    global_data: &GlobalRewardData,
    user: &Address,
    working_balance: u128,
) -> UserRewardData {
    // start user inv from zero to retroactively calculate rewards
    let user_data = get_user_reward_data(env, user.clone()).unwrap_or(UserRewardData {
        epoch: 0,
        last_inv: U256::from_u32(env, 0),
        to_claim: 0,
    });

    // If no new accumulation, just return
    if user_data.epoch == global_data.epoch {
        return user_data;
    }

    if working_balance == 0 {
        // No new reward
        let new_data = UserRewardData {
            epoch: global_data.epoch,
            last_inv: global_data.inv.clone(),
            to_claim: user_data.to_claim,
        };
        set_user_reward_data(env, user.clone(), &new_data);
        return new_data;
    }

    let current_inv = global_data.inv.clone();
    let prev_inv = user_data.last_inv;
    let reward = U256::from_u128(env, working_balance)
        .mul(&current_inv.sub(&prev_inv))
        .div(&U256::from_u128(env, REWARD_PRECISION))
        .to_u128()
        .unwrap();
    let new_data = UserRewardData {
        epoch: global_data.epoch,
        last_inv: current_inv,
        to_claim: user_data.to_claim + reward,
    };
    set_user_reward_data(env, user.clone(), &new_data);
    new_data
}
