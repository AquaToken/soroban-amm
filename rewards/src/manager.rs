use crate::boost_feed::RewardBoostFeedClient;
use crate::constants::REWARD_PRECISION;
use crate::errors::RewardsError;
use crate::storage::{
    BoostFeedStorageTrait, BoostTokenStorageTrait, PoolRewardConfig, PoolRewardData,
    PoolRewardsStorageTrait, RewardInvDataStorageTrait, RewardTokenStorageTrait, Storage,
    UserRewardData, UserRewardsStorageTrait, WorkingBalancesStorageTrait,
};
use crate::RewardsConfig;
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{panic_with_error, token::TokenClient as Client, Address, Env, Vec};
use utils::bump::bump_instance;

// `Manager` orchestrates the reward logic, pulling data and methods from `Storage`.
// It relies on Storage sub-traits to handle actual storage I/O.
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

    // ------------------------------------
    // Basic getters for boost balances
    // ------------------------------------

    pub fn get_user_boost_balance(&self, user: &Address) -> u128 {
        if self.storage.has_reward_boost_token() {
            match SorobanTokenClient::new(&self.env, &self.storage.get_reward_boost_token())
                .try_balance(user)
            {
                Ok(balance) => balance.unwrap() as u128,
                // if trustline is not established, return 0
                Err(_) => 0,
            }
        } else {
            0
        }
    }

    pub fn get_total_locked(&self) -> u128 {
        if self.storage.has_reward_boost_feed() {
            RewardBoostFeedClient::new(&self.env, &self.storage.get_reward_boost_feed())
                .total_supply()
        } else {
            0
        }
    }

    // ------------------------------------
    // Effective balance logic
    // ------------------------------------

    fn calculate_effective_balance(
        &self,
        user: &Address,
        share_balance: u128,
        total_share: u128,
    ) -> u128 {
        // b_u = 2.5 * min(0.4 * b_u + 0.6 * S * w_i / W, b_u)
        let lock_balance = self.get_user_boost_balance(&user);
        let total_locked = self.get_total_locked();

        let mut adjusted_balance = share_balance;
        if total_locked > 0 {
            adjusted_balance +=
                3 * lock_balance.fixed_mul_floor(&self.env, &total_share, &total_locked) / 2;
        }
        let max_effective_balance = share_balance * 5 / 2;

        // min(adjusted_balance, max_effective_balance)
        if adjusted_balance > max_effective_balance {
            max_effective_balance
        } else {
            adjusted_balance
        }
    }

    // ------------------------------------
    // Reward configuration
    // ------------------------------------

    // Sets the reward configuration for the pool.
    //
    // # Arguments
    //
    // * `total_shares` - The total shares in the pool.
    // * `expired_at` - The expiration time for the reward configuration.
    // * `tps` - The number of tokens per second for the reward configuration.
    //
    // # Panics
    //
    // This method will panic if the expiration time is in the past or if the tokens per second is zero and the configuration has already expired.
    pub fn set_reward_config(&mut self, total_shares: u128, mut expired_at: u64, tps: u128) {
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
            // Already expired, no need to override
            return;
        }

        let working_supply = self.get_working_supply(total_shares);

        // Bring pool data up-to-date
        self.update_rewards_data(working_supply);
        self.snapshot_rewards_data(working_supply);

        let config = PoolRewardConfig { tps, expired_at };

        bump_instance(&self.env);
        self.storage.set_pool_reward_config(&config);
    }

    // ------------------------------------
    // Updating pool & user reward data
    // ------------------------------------

    // Updates the pool rewards data to represent the current state of the rewards.
    //
    // # Arguments
    //
    // * `total_shares` - The total shares in the pool.
    //
    // # Returns
    //
    // * The updated `PoolRewardData` instance.
    fn update_rewards_data(&mut self, working_supply: u128) -> PoolRewardData {
        let config = self.storage.get_pool_reward_config();
        let mut data = self.storage.get_pool_reward_data();
        let now = self.env.ledger().timestamp();

        if now <= config.expired_at {
            // config not expired yet, yield rewards
            let generated_tokens = (now - data.last_time) as u128 * config.tps;
            self.create_new_rewards_data(
                generated_tokens,
                working_supply,
                PoolRewardData {
                    block: data.block + 1,
                    accumulated: data.accumulated + generated_tokens,
                    claimed: data.claimed,
                    last_time: now,
                },
            )
        } else {
            // Already expired
            if data.last_time < config.expired_at {
                // last snapshot was before config expiration - yield up to expiration
                let generated_tokens = (config.expired_at - data.last_time) as u128 * config.tps;
                data = self.create_new_rewards_data(
                    generated_tokens,
                    working_supply,
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

    // Ensures that the pool rewards data represents the current state of the rewards and is ready for a new configuration.
    //
    // This method checks if the last snapshot was taken at the current time. If not, it creates a new snapshot with the current time.
    // No new reward is generated in this process.
    //
    // # Arguments
    //
    // * `total_shares` - The total shares in the pool.
    //
    // # Returns
    //
    // * The updated `PoolRewardData` instance.
    fn snapshot_rewards_data(&mut self, working_supply: u128) -> PoolRewardData {
        let data = self.storage.get_pool_reward_data();
        let now = self.env.ledger().timestamp();

        if data.last_time == now {
            // snapshot already made
            data
        } else {
            self.create_new_rewards_data(
                0,
                working_supply,
                PoolRewardData {
                    block: data.block + 1,
                    accumulated: data.accumulated,
                    claimed: data.claimed,
                    last_time: now,
                },
            )
        }
    }

    // Updates the reward data for a specific user.
    //
    // # Arguments
    //
    // * `pool_data` - The current pool reward data.
    // * `user` - The address of the user for whom the reward data is being updated.
    // * `user_balance_shares` - The number of shares the user has in the pool.
    //
    // # Returns
    //
    // * The updated `UserRewardData` instance for the user.
    fn update_user_reward(
        &mut self,
        pool_data: &PoolRewardData,
        user: &Address,
        user_balance_shares: u128,
    ) -> UserRewardData {
        if let Some(user_data) = self.storage.get_user_reward_data(user) {
            // If no new accumulation, just return
            if user_data.pool_accumulated == pool_data.accumulated {
                return user_data;
            }

            if user_balance_shares == 0 {
                // No new reward
                return self.create_new_user_data(user, pool_data, user_data.to_claim);
            }

            let reward = self.calculate_user_reward(
                user_data.last_block + 1,
                pool_data.block,
                user_balance_shares,
            );
            self.create_new_user_data(user, pool_data, user_data.to_claim + reward)
        } else {
            self.create_new_user_data(user, pool_data, 0)
        }
    }

    // Calculates the reward for a user based on their share of the total shares.
    //
    // # Arguments
    //
    // * `start_block` - The block number from which the reward calculation starts.
    // * `end_block` - The block number at which the reward calculation ends.
    // * `user_share` - The share of the user in the total shares.
    //
    // # Returns
    //
    // * The calculated reward for the user.
    fn calculate_user_reward(
        &mut self,
        start_block: u64,
        end_block: u64,
        user_share: u128,
    ) -> u128 {
        let result = self.calculate_reward(start_block, end_block);
        // scale by user_share / REWARD_PRECISION
        result * user_share / REWARD_PRECISION
    }

    // ------------------------------------
    // Public user methods
    // ------------------------------------

    // Calculates the amount of reward a user is eligible to claim.
    //
    // # Arguments
    //
    // * `user` - The address of the user for whom the reward is being calculated.
    // * `total_shares` - The total shares in the pool.
    // * `user_balance_shares` - The number of shares the user has in the pool.
    //
    // # Returns
    //
    // * The amount of reward the user is eligible to claim.
    pub fn get_amount_to_claim(
        &mut self,
        user: &Address,
        total_shares: u128,
        user_balance_shares: u128,
    ) -> u128 {
        // update pool data & calculate reward
        self.checkpoint_user(user, total_shares, user_balance_shares)
            .to_claim
    }

    // Actually claims the user's reward and transfers tokens.
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
        } = self.checkpoint_user(user, total_shares, user_balance_shares);

        // Increase total claimed in the pool
        let mut pool_data = self.storage.get_pool_reward_data();
        pool_data.claimed += reward_amount;
        self.storage.set_pool_reward_data(&pool_data);

        // Transfer tokens
        let reward_token = self.storage.get_reward_token();
        Client::new(&self.env, &reward_token).transfer(
            &self.env.current_contract_address(),
            user,
            &(reward_amount as i128),
        );

        // Reset user reward
        let new_data = UserRewardData {
            last_block,
            pool_accumulated,
            to_claim: 0,
        };
        self.storage.set_user_reward_data(user, &new_data);

        reward_amount
    }

    // Forces an update of the user's reward data based on the new working balance.
    pub fn checkpoint_user(
        &mut self,
        user: &Address,
        total_shares: u128,
        user_balance_shares: u128,
    ) -> UserRewardData {
        let (working_balance, new_working_supply) =
            self.update_working_balance(user, total_shares, user_balance_shares);

        let pool_data = self.update_rewards_data(new_working_supply);
        let user_data = self.update_user_reward(&pool_data, user, working_balance);

        // Bump storage for the user's data
        self.storage.bump_user_reward_data(user);
        user_data
    }

    // ------------------------------------
    // Working balance manipulation
    // ------------------------------------

    pub fn get_working_supply(&mut self, total_shares: u128) -> u128 {
        if self.storage.has_working_supply() {
            self.storage.get_working_supply()
        } else {
            self.storage.set_working_supply(total_shares);
            total_shares
        }
    }

    pub fn get_working_balance(&mut self, user: &Address, user_balance_shares: u128) -> u128 {
        if self.storage.has_working_balance(user) {
            self.storage.get_working_balance(user)
        } else {
            self.storage.set_working_balance(user, user_balance_shares);
            user_balance_shares
        }
    }

    pub fn update_working_balance(
        &mut self,
        user: &Address,
        total_shares: u128,
        user_balance_shares: u128,
    ) -> (u128, u128) {
        let prev_working_balance = self.get_working_balance(user, user_balance_shares);
        let prev_working_supply = self.get_working_supply(total_shares);

        let working_balance =
            self.calculate_effective_balance(user, user_balance_shares, total_shares);

        let new_working_supply = prev_working_supply + working_balance - prev_working_balance;
        self.storage.set_working_supply(new_working_supply);
        self.storage.set_working_balance(user, working_balance);

        (working_balance, new_working_supply)
    }

    // ------------------------------------
    // Aggregated reward pages
    // ------------------------------------

    // Aggregated reward page data getter
    // normalizes the length of the page up to the page size for predictable limits calculation
    //
    // # Arguments
    //
    // * `pow` - The power of the page size.
    // * `page_number` - The number of the page.
    //
    // # Returns The aggregated page data.
    //
    // * The aggregated page data.
    fn get_reward_inv_data(&mut self, pow: u32, page_number: u64) -> Vec<u128> {
        let mut page = self.storage.get_reward_inv_data(pow, page_number);

        if pow == 0 {
            // for consistency, normalize the length to config.page_size for pow=0
            for _ in page.len() as u64..self.config.page_size {
                page.push_back(0);
            }
        }

        page
    }

    // Aggregated reward page data setter
    //
    // # Arguments
    //
    // * `pow` - The power of the page size.
    // * `page_number` - The number of the page.
    // * `aggregated_page` - The aggregated page data.
    fn set_reward_inv_data(&mut self, pow: u32, page_number: u64, aggregated_page: Vec<u128>) {
        self.storage
            .set_reward_inv_data(pow, page_number, aggregated_page);
    }

    // ------------------------------------
    // Reward calculation by blocks
    // ------------------------------------

    // Calculates the total reward between two blocks.
    //
    // This method calculates the total reward from the start block to the end block inclusively
    //
    // # Arguments
    //
    // * `start_block` - The block number from which the reward calculation starts.
    // * `end_block` - The block number at which the reward calculation ends.
    fn calculate_reward(&mut self, start_block: u64, end_block: u64) -> u128 {
        // 1. Find the largest pow where (start_block + page_size^pow) <= end_block
        // 2. Move block to next chunk, accumulate from stored data

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

            let cell_size = self.config.page_size.pow(pow);
            let page_size = cell_size * self.config.page_size;
            let cell_idx = block % page_size / cell_size;
            let page_number = block / page_size;
            let next_block = block + cell_size;

            let page = self.get_reward_inv_data(pow, page_number);
            let val = page.get(cell_idx as u32).unwrap_or(0);
            result += val;

            if next_block > end_block {
                block = end_block + 1;
            } else {
                block = next_block;
            }
        }
        result
    }

    // Updates the invariant storage with the reward per share for each block.
    //
    // The reward per share for a block is calculated by dividing the total accumulated reward by the total shares.
    // This value is then added to the cumulative reward per share for the current block in the invariant storage.
    //
    // # Arguments
    //
    // * `block` - The block number for which the reward per share is being calculated.
    // * `value` - The total accumulated reward.
    fn add_reward_inv(&mut self, block: u64, value: u128) {
        // For each pow level, update the relevant page.
        for pow in 0..255 {
            if pow > 0 && block + 1 < self.config.page_size.pow(pow - 1) {
                break;
            }

            let cell_size = self.config.page_size.pow(pow);
            let page_size = cell_size * self.config.page_size;
            let cell_idx = (block % page_size / cell_size) as u32;
            let page_number = block / page_size;

            let mut aggregated_page = self.get_reward_inv_data(pow, page_number);
            let old_val = aggregated_page.get(cell_idx).unwrap_or(0);
            let new_val = old_val + value;
            // pow 0 page is fixed length=config.page_size
            // pow 1+ pages are growable
            if pow > 0 && cell_idx == aggregated_page.len() {
                aggregated_page.push_back(new_val);
            } else {
                aggregated_page.set(cell_idx, new_val);
            }
            self.set_reward_inv_data(pow, page_number, aggregated_page);
        }
    }

    // Updates the invariant storage with the reward per share for the current block.
    //
    // # Arguments
    //
    // * `accumulated` - The total accumulated reward.
    // * `total_shares` - The total shares in the pool.
    fn update_reward_inv(&mut self, accumulated: u128, working_supply: u128) {
        let reward_per_share = if working_supply > 0 {
            REWARD_PRECISION * accumulated / working_supply
        } else {
            0
        };

        let data = self.storage.get_pool_reward_data();
        self.add_reward_inv(data.block, reward_per_share);
    }

    // ------------------------------------
    // Helpers for consistent data writes
    // ------------------------------------

    fn create_new_rewards_data(
        &mut self,
        generated_tokens: u128,
        working_supply: u128,
        new_data: PoolRewardData,
    ) -> PoolRewardData {
        // Persist the new pool data
        self.storage.set_pool_reward_data(&new_data);

        // Update the reward_inv with newly generated tokens
        self.update_reward_inv(generated_tokens, working_supply);
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

    // ------------------------------------
    // Additional getters
    // ------------------------------------

    pub fn get_total_accumulated_reward(&mut self, total_shares: u128) -> u128 {
        let working_supply = self.get_working_supply(total_shares);
        let data = self.update_rewards_data(working_supply);
        data.accumulated
    }

    pub fn get_total_claimed_reward(&mut self, total_shares: u128) -> u128 {
        let working_supply = self.get_working_supply(total_shares);
        let data = self.update_rewards_data(working_supply);
        data.claimed
    }

    pub fn get_total_configured_reward(&mut self, total_shares: u128) -> u128 {
        let config = self.storage.get_pool_reward_config();
        let working_supply = self.get_working_supply(total_shares);
        let data = self.update_rewards_data(working_supply);
        let rewarded_amount = data.accumulated;

        let now = self.env.ledger().timestamp();
        if config.expired_at <= now {
            // no rewards configured in future
            rewarded_amount
        } else {
            let outstanding_reward = (config.expired_at - now) as u128 * config.tps;
            rewarded_amount + outstanding_reward
        }
    }
}
