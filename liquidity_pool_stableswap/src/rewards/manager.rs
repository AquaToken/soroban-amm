use crate::rewards::constants::PAGE_SIZE;
use crate::rewards::storage::{
    get_pool_reward_config, get_pool_reward_data, get_user_reward_data, set_pool_reward_config,
    set_pool_reward_data, set_user_reward_data, PoolRewardConfig, PoolRewardData, UserRewardData,
};
use crate::storage::get_reward_storage;
use crate::token::Client;
use crate::{rewards, storage, token};
use cast::u128 as to_u128;
use soroban_sdk::{Address, Env, Map};

pub fn update_reward_inv(e: &Env, accumulated: u128) {
    let total_shares = token::get_total_shares(e);
    let reward_per_share = if total_shares > 0 {
        accumulated / total_shares
    } else {
        0
    };

    let data = get_pool_reward_data(e);
    add_reward_inv(e, data.block, reward_per_share as u64);
}

pub fn calculate_reward(e: &Env, start_block: u64, end_block: u64, use_max_pow: bool) -> u64 {
    // calculate result from start_block to end_block [...]
    // use_max_pow disabled during aggregation process
    //  since we don't have such information and can be enabled after
    let mut result = 0;
    let mut block = start_block;

    let mut max_pow = 0;
    for pow in 1..255 {
        if start_block + PAGE_SIZE.pow(pow) - 1 > end_block {
            break;
        }
        max_pow = pow;
    }

    while block <= end_block {
        if block % PAGE_SIZE == 0 {
            // check possibilities to skip
            let mut block_increased = false;
            let mut max_block_pow = 0;
            for i in (1..max_pow + 1).rev() {
                if block % PAGE_SIZE.pow(i) == 0 {
                    max_block_pow = i;
                    break;
                }
            }
            if !use_max_pow {
                // value not precalculated yet
                max_block_pow -= 1;
            }

            for l_pow in (1..max_block_pow + 1).rev() {
                let next_block = block + PAGE_SIZE.pow(l_pow);
                if next_block > end_block {
                    continue;
                }

                let page_number = block / PAGE_SIZE.pow(l_pow + 1);
                // println!("skipping {} -> {} (page {}, pow {})", block, next_block, page_number, l_pow);
                let page = rewards::storage::get_reward_inv_page(e, l_pow, page_number);
                result += page.get(block).expect("unknown block");
                block = next_block;
                block_increased = true;
                break;
            }
            if !block_increased {
                // couldn't find shortcut, looks like we're close to the tail. go one by one
                // println!("skipping {} -> {} (page {}, pow {})", block, block + 1, block / PAGE_SIZE, 0);
                let page = rewards::storage::get_reward_inv_page(e, 0, block / PAGE_SIZE);
                result += page.get(block).expect("unknown block");
                block += 1;
            }
        } else {
            // println!("skipping {} -> {} (page {}, pow {})", block, block + 1, block / PAGE_SIZE, 0);
            let page = rewards::storage::get_reward_inv_page(e, 0, block / PAGE_SIZE);
            result += page.get(block).expect("unknown block");
            block += 1;
        }
    }
    result
}

pub fn write_reward_inv_to_page(e: &Env, pow: u32, start_block: u64, value: u64) {
    let page_number = start_block / PAGE_SIZE.pow(pow + 1);
    let mut page = match start_block % PAGE_SIZE.pow(pow + 1) {
        0 => Map::new(e),
        _ => rewards::storage::get_reward_inv_page(e, pow, page_number),
    };
    page.set(start_block, value);
    if pow > 0 {
        // println!("writing {} -> {} (page {}, pow {})", start_block, start_block + PAGE_SIZE.pow(pow) - 1, page_number, pow);
    } else {
        // println!("writing {} (page {})", start_block, page_number);
    }
    rewards::storage::set_reward_inv_page(e, pow, page_number, &page);
}

pub fn add_reward_inv(e: &Env, block: u64, value: u64) {
    // write zero level page first
    write_reward_inv_to_page(e, 0, block, value);

    if (block + 1) % PAGE_SIZE == 0 {
        // page end, at least one aggregation should be applicable
        for pow in 1..255 {
            let aggregation_size = PAGE_SIZE.pow(pow);
            if (block + 1) % aggregation_size != 0 {
                // aggregation level not applicable
                break;
            }
            let agg_page_start = block - block % aggregation_size;
            let aggregation = calculate_reward(e, agg_page_start, block, false);
            write_reward_inv_to_page(e, pow, agg_page_start, aggregation);
        }
    }
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

        let generated_tokens = to_u128(reward_timestamp - data.last_time) * config.tps;
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

            let generated_tokens = to_u128(reward_timestamp - data.last_time) * config.tps;
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

pub fn calculate_user_reward(e: &Env, start_block: u64, end_block: u64, user_share: u128) -> u128 {
    let result = calculate_reward(e, start_block, end_block, true);
    (result) as u128 * user_share
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

        let reward =
            calculate_user_reward(e, user_data.last_block + 1, pool_data.block, user_shares);
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

pub fn get_amount_to_claim(e: &Env, user: &Address) -> u128 {
    // update pool data & calculate reward
    let pool_data = update_rewards_data(e);
    let user_reward = update_user_reward(e, &pool_data, user);
    user_reward.to_claim
}

pub fn claim_reward(e: &Env, user: &Address) -> u128 {
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
        &(reward_amount as i128),
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

pub fn initialize(e: &Env) {
    add_reward_inv(&e, 0, 0);
    set_pool_reward_data(
        &e,
        &PoolRewardData {
            block: 0,
            accumulated: 0,
            last_time: 0,
        },
    );
    set_pool_reward_config(
        &e,
        &PoolRewardConfig {
            tps: 0,
            expired_at: 0,
        },
    );
}
