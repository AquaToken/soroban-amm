use crate::constants::REWARD_PRECISION;
use crate::errors::RewardsError;
use crate::storage::{
    PoolRewardConfig, PoolRewardData, RewardsStorageTrait, Storage, UserRewardData,
};
use crate::RewardsConfig;
use cast::u128 as to_u128;
use soroban_sdk::{panic_with_error, token::TokenClient as Client, Address, Env};
use utils::bump::bump_instance;
use utils::storage_errors::StorageError;

pub struct Manager {
    env: Env,
    storage: Storage,
    config: RewardsConfig,
}

impl Manager {
    pub fn new(e: &Env, storage: Storage, config: &RewardsConfig) -> Manager {
        Manager {
            env: e.clone(),
            storage,
            config: config.clone(),
        }
    }

    pub fn set_reward_config(&mut self, total_shares: u128, expired_at: u64, tps: u128) {
        let mut expired_at = expired_at;

        let now = self.env.ledger().timestamp();
        let old_config = self.storage.get_pool_reward_config();
        // if we stop rewards manually by setting tps to zero,
        //  set expiration to the lowest possible value to avoid extra blocks
        if tps == 0 {
            expired_at = now;
        } else if old_config.expired_at == expired_at {
            // expiration time should differ as we rely on it inside the rewards manager
            panic_with_error!(&self.env, RewardsError::SameRewardsConfig);
        }

        if expired_at < now {
            panic_with_error!(&self.env, RewardsError::PastTimeNotAllowed);
        }
        if old_config.expired_at < now && tps == 0 {
            // config already expired, no need to override it with zero tps
            return;
        }

        self.update_rewards_data(total_shares);
        self.snapshot_rewards_data(total_shares);
        let config = PoolRewardConfig { tps, expired_at };

        bump_instance(&self.env);
        self.storage.set_pool_reward_config(&config);
    }

    // make sure pool rewards data represents the current state of the rewards. update if necessary
    pub fn update_rewards_data(&mut self, total_shares: u128) -> PoolRewardData {
        let config = self.storage.get_pool_reward_config();
        let mut data = self.storage.get_pool_reward_data();
        let now = self.env.ledger().timestamp();

        if now <= config.expired_at {
            // config not expired yet, yield rewards
            let generated_tokens = to_u128(now - data.last_time) * to_u128(config.tps);
            self.create_new_rewards_data(
                generated_tokens,
                total_shares,
                PoolRewardData {
                    block: data.block + 1,
                    accumulated: data.accumulated + generated_tokens,
                    claimed: data.claimed,
                    last_time: now,
                },
            )
        } else {
            // config already expired
            if data.last_time < config.expired_at {
                // last snapshot was before config expiration - yield up to expiration
                let generated_tokens =
                    to_u128(config.expired_at - data.last_time) * to_u128(config.tps);
                data = self.create_new_rewards_data(
                    generated_tokens,
                    total_shares,
                    PoolRewardData {
                        block: data.block + 1,
                        accumulated: data.accumulated + generated_tokens,
                        claimed: data.claimed,
                        last_time: config.expired_at,
                    },
                );
            }

            // snapshot is on expiration time. no reward should be generated,
            data
        }
    }

    // make sure pool rewards data is actual and ready for new configuration
    // to be used only after
    pub fn snapshot_rewards_data(&mut self, total_shares: u128) -> PoolRewardData {
        let data = self.storage.get_pool_reward_data();
        let now = self.env.ledger().timestamp();

        if data.last_time == now {
            // already snapshoted
            data
        } else {
            self.create_new_rewards_data(
                0,
                total_shares,
                PoolRewardData {
                    block: data.block + 1,
                    accumulated: data.accumulated,
                    claimed: data.claimed,
                    last_time: now,
                },
            )
        }
    }

    fn calculate_user_reward(
        &mut self,
        start_block: u64,
        end_block: u64,
        user_share: u128,
    ) -> u128 {
        let result = self.calculate_reward(start_block, end_block);
        (result) * user_share / REWARD_PRECISION
    }

    pub fn update_user_reward(
        &mut self,
        pool_data: &PoolRewardData,
        user: &Address,
        user_balance_shares: u128,
    ) -> UserRewardData {
        match self.storage.get_user_reward_data(user) {
            Some(user_data) => {
                if user_data.pool_accumulated == pool_data.accumulated {
                    // nothing accumulated since last update
                    return user_data;
                }

                if user_balance_shares == 0 {
                    // zero balance, no new reward
                    return self.create_new_user_data(user, pool_data, user_data.to_claim);
                }

                let reward = self.calculate_user_reward(
                    user_data.last_block + 1,
                    pool_data.block,
                    user_balance_shares,
                );
                self.create_new_user_data(user, pool_data, user_data.to_claim + reward)
            }
            None => self.create_new_user_data(user, pool_data, 0),
        }
    }

    pub fn get_amount_to_claim(
        &mut self,
        user: &Address,
        total_shares: u128,
        user_balance_shares: u128,
    ) -> u128 {
        // update pool data & calculate reward
        self.user_reward_data(user, total_shares, user_balance_shares)
            .to_claim
    }

    pub fn claim_reward(
        &mut self,
        user: &Address,
        total_shares: u128,
        user_balance_shares: u128,
    ) -> u128 {
        // update pool data & calculate reward
        let UserRewardData {
            last_block,
            pool_accumulated,
            to_claim: reward_amount,
        } = self.user_reward_data(user, total_shares, user_balance_shares);

        // increase total claimed amount
        let mut pool_data = self.storage.get_pool_reward_data();
        pool_data.claimed += reward_amount;
        self.storage.set_pool_reward_data(&pool_data);

        // transfer reward
        let reward_token = self.storage.get_reward_token();
        Client::new(&self.env, &reward_token).transfer(
            &self.env.current_contract_address(),
            user,
            &(reward_amount as i128),
        );

        // set available reward to zero
        let new_data = UserRewardData {
            last_block,
            pool_accumulated,
            to_claim: 0,
        };
        self.storage.set_user_reward_data(user, &new_data);
        reward_amount
    }

    // private functions
    fn calculate_reward(&mut self, start_block: u64, end_block: u64) -> u128 {
        // calculate result from start_block to end_block [...]
        //  since we don't have such information and can be enabled after
        let mut result = 0;
        let mut block = start_block;

        let mut max_pow = 0;
        for pow in 1..255 {
            max_pow = pow;
            if start_block + self.config.page_size.pow(pow) - 1 > end_block {
                break;
            }
        }

        while block <= end_block {
            let mut pow = 0;
            for i in (0..=max_pow).rev() {
                if block % self.config.page_size.pow(i) == 0 {
                    pow = i;
                    break;
                }
            }

            let next_block = block + self.config.page_size.pow(pow);
            let page_number = block / self.config.page_size.pow(pow + 1);
            let page = self.storage.get_reward_inv_data(pow, page_number);
            result += match page.get(block) {
                Some(v) => v,
                None => panic_with_error!(self.env, StorageError::ValueMissing),
            };
            if next_block > end_block {
                block = end_block + 1;
            } else {
                block = next_block;
            }
        }
        result
    }

    fn add_reward_inv(&mut self, block: u64, value: u128) {
        for pow in 0..255 {
            if pow > 0 && block + 1 < self.config.page_size.pow(pow - 1) {
                break;
            }

            let cell_size = self.config.page_size.pow(pow);
            let page_size = self.config.page_size.pow(pow + 1);
            let cell_start = block - block % cell_size;
            let page_start = block - block % page_size;
            let page_number = page_start / page_size;

            let mut aggregated_page = self.storage.get_reward_inv_data(pow, page_number);
            let current_value = aggregated_page.get(cell_start).unwrap_or(0);
            let increased_value = current_value + value;
            aggregated_page.set(cell_start, increased_value);
            self.storage
                .set_reward_inv_data(pow, page_number, aggregated_page);
        }
    }

    fn update_reward_inv(&mut self, accumulated: u128, total_shares: u128) {
        let reward_per_share = if total_shares > 0 {
            REWARD_PRECISION * accumulated / total_shares
        } else {
            0
        };

        let data = self.storage.get_pool_reward_data();
        self.add_reward_inv(data.block, reward_per_share);
    }

    fn create_new_rewards_data(
        &mut self,
        generated_tokens: u128,
        total_shares: u128,
        new_data: PoolRewardData,
    ) -> PoolRewardData {
        self.storage.set_pool_reward_data(&new_data);
        self.update_reward_inv(generated_tokens, total_shares);
        new_data
    }

    fn create_new_user_data(
        &self,
        user: &Address,
        pool_data: &PoolRewardData,
        to_claim: u128,
    ) -> UserRewardData {
        let new_data = UserRewardData {
            last_block: pool_data.block,
            pool_accumulated: pool_data.accumulated,
            to_claim,
        };
        self.storage.set_user_reward_data(user, &new_data);
        new_data
    }

    fn user_reward_data(
        &mut self,
        user: &Address,
        total_shares: u128,
        user_balance_shares: u128,
    ) -> UserRewardData {
        let rewards_data = self.update_rewards_data(total_shares);
        self.update_user_reward(&rewards_data, user, user_balance_shares)
    }

    pub fn get_total_accumulated_reward(&mut self, total_shares: u128) -> u128 {
        let data = self.update_rewards_data(total_shares);
        data.accumulated
    }

    pub fn get_total_claimed_reward(&mut self, total_shares: u128) -> u128 {
        let data = self.update_rewards_data(total_shares);
        data.claimed
    }

    pub fn get_total_configured_reward(&mut self, total_shares: u128) -> u128 {
        let config = self.storage.get_pool_reward_config();
        let data = self.update_rewards_data(total_shares);
        let rewarded_amount = data.accumulated;

        let now = self.env.ledger().timestamp();
        match config.expired_at <= now {
            true => {
                // no rewards configured in future
                rewarded_amount
            }
            false => {
                let outstanding_reward = (config.expired_at - now) as u128 * config.tps;
                rewarded_amount + outstanding_reward
            }
        }
    }
}
