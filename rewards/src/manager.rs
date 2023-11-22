use crate::storage::{PoolRewardData, RewardsStorageTrait, Storage, UserRewardData};
use crate::Client;
use cast::u128 as to_u128;
use soroban_sdk::{Address, Env, Map};
use utils::constant::{PERSISTENT_BUMP_AMOUNT, PERSISTENT_LIFETIME_THRESHOLD};

// TODO: REFACTOR ManagerTrait FUNCTIONS FROM PREV COMMIT

pub struct Manager {
    env: Env,
    storage: Storage,
}

impl Manager {
    pub fn new(e: &Env) -> Manager {
        Manager {
            env: e.clone(),
            storage: Storage::new(e),
        }
    }
}

pub trait ManagerTrait {
    fn update_reward_inv(&self, accumulated: u128, total_shares: u128);
    fn add_reward_inv(&self, block: u64, value: u64);
    fn set_reward_inv(&self, value: &Map<u64, u64>);
    fn get_reward_inv(&self) -> Map<u64, u64>;
    fn update_rewards_data(&self, total_shares: u128) -> PoolRewardData;
    fn calculate_user_reward(&self, start_block: u64, end_block: u64, user_share: u128) -> u128;
    fn update_user_reward(
        &self,
        pool_data: &PoolRewardData,
        user: &Address,
        user_balance_shares: u128,
    ) -> UserRewardData;
    fn get_amount_to_claim(
        &self,
        user: &Address,
        total_shares: u128,
        user_balance_shares: u128,
    ) -> u128;
    fn claim_reward(&self, user: &Address, total_shares: u128, user_balance_shares: u128) -> u128;
}

impl ManagerTrait for Manager {
    // here need transfer total_shares outside
    fn update_reward_inv(&self, accumulated: u128, total_shares: u128) {
        let reward_per_share = if total_shares > 0 {
            accumulated / total_shares
        } else {
            0
        };
        self.add_reward_inv(
            self.storage.get_pool_reward_data().block,
            reward_per_share as u64,
        );
    }

    fn add_reward_inv(&self, block: u64, value: u64) {
        let mut reward_inv_data: Map<u64, u64> = self.storage.get_reward_inv_data();
        reward_inv_data.set(block, value);
        self.set_reward_inv(&reward_inv_data);
    }

    fn set_reward_inv(&self, value: &Map<u64, u64>) {
        self.storage.set_reward_inv_data(value);
    }

    fn get_reward_inv(&self) -> Map<u64, u64> {
        // todo: optimize memory usage
        // todo: do we need default here?
        let reward_inv_data = self.storage.get_reward_inv_data();
        self.storage.bump_reward_inv_data();
        reward_inv_data
    }

    fn update_rewards_data(&self, total_shares: u128) -> PoolRewardData {
        let config = self.storage.get_pool_reward_config();
        let data = self.storage.get_pool_reward_data();
        let now = self.env.ledger().timestamp();

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
            self.storage.set_pool_reward_data(&new_data);
            self.update_reward_inv(generated_tokens, total_shares);
            new_data
        } else {
            if data.last_time > config.expired_at {
                // todo: don't increase block
                let new_data = PoolRewardData {
                    block: data.block + 1,
                    accumulated: data.accumulated,
                    last_time: now,
                };
                self.storage.set_pool_reward_data(&new_data);
                self.update_reward_inv(0, total_shares);
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
                self.storage.set_pool_reward_data(&catchup_data);
                self.update_reward_inv(generated_tokens, total_shares);

                // todo: don't increase block when config not enabled thus keeping invariants list small
                let new_data = PoolRewardData {
                    block: catchup_data.block + 1,
                    accumulated: catchup_data.accumulated,
                    last_time: now,
                };
                self.storage.set_pool_reward_data(&new_data);
                self.update_reward_inv(0, total_shares);
                new_data
            }
        };
    }

    fn calculate_user_reward(&self, start_block: u64, end_block: u64, user_share: u128) -> u128 {
        let mut reward_inv = 0;
        for block in start_block..end_block + 1 {
            let block_inv = self
                .get_reward_inv()
                .get(block)
                .expect("Trying to get reward inv");
            reward_inv += block_inv;
        }
        self.storage.bump_reward_inv_data();
        (reward_inv) as u128 * user_share
    }

    fn update_user_reward(
        &self,
        pool_data: &PoolRewardData,
        user: &Address,
        user_balance_shares: u128,
    ) -> UserRewardData {
        return if let Some(user_data) = self.storage.get_user_reward_data(user) {
            if user_data.pool_accumulated == pool_data.accumulated {
                // nothing accumulated since last update
                return user_data;
            }

            let user_shares = user_balance_shares;
            if user_shares == 0 {
                // zero balance, no new reward
                let new_data = UserRewardData {
                    last_block: pool_data.block,
                    pool_accumulated: pool_data.accumulated,
                    to_claim: user_data.to_claim,
                };
                self.storage.set_user_reward_data(user, &new_data);
                return new_data;
            }

            let reward =
                self.calculate_user_reward(user_data.last_block + 1, pool_data.block, user_shares);
            // let new_reward =
            //     (pool_data.accumulated - user_data.pool_accumulated) * user_shares / total_shares;
            let new_data = UserRewardData {
                last_block: pool_data.block,
                pool_accumulated: pool_data.accumulated,
                to_claim: user_data.to_claim + reward,
            };
            self.storage.set_user_reward_data(user, &new_data);
            new_data
        } else {
            // user has joined
            let new_data = UserRewardData {
                last_block: pool_data.block,
                pool_accumulated: pool_data.accumulated,
                to_claim: 0,
            };
            self.storage.set_user_reward_data(user, &new_data);
            new_data
        };
    }

    fn get_amount_to_claim(
        &self,
        user: &Address,
        total_shares: u128,
        user_balance_shares: u128,
    ) -> u128 {
        // update pool data & calculate reward
        let pool_data = self.update_rewards_data(total_shares);
        let user_reward = self.update_user_reward(&pool_data, user, user_balance_shares);
        user_reward.to_claim
    }

    fn claim_reward(&self, user: &Address, total_shares: u128, user_balance_shares: u128) -> u128 {
        // update pool data & calculate reward
        let pool_data = self.update_rewards_data(total_shares);
        let user_reward = self.update_user_reward(&pool_data, user, user_balance_shares);
        let reward_amount = user_reward.to_claim;

        // transfer reward
        let reward_token = self.storage.get_reward_token();
        Client::new(&self.env, &reward_token).transfer_from(
            &self.env.current_contract_address(),
            &self.storage.get_reward_storage(),
            &user,
            &(reward_amount as i128),
        );

        // set available reward to zero
        let new_data = UserRewardData {
            last_block: pool_data.block,
            pool_accumulated: pool_data.accumulated,
            to_claim: 0,
        };
        self.storage.set_user_reward_data(user, &new_data);

        reward_amount
    }
}
