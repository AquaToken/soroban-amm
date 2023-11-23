use crate::storage::{
    PoolRewardConfig, PoolRewardData, RewardsStorageTrait, Storage, UserRewardData,
};
use crate::Client;
use cast::u128 as to_u128;
use soroban_sdk::{Address, Env, Map};

// TODO: REFACTOR ManagerTrait FUNCTIONS FROM PREV COMMIT

pub struct Manager {
    env: Env,
    storage: Storage,
}

impl Manager {
    pub fn new(e: &Env, storage: Storage) -> Manager {
        Manager {
            env: e.clone(),
            storage,
        }
    }

    pub fn update_rewards_data(&self, total_shares: u128) -> PoolRewardData {
        let config = self.storage.get_pool_reward_config();
        let data = self.storage.get_pool_reward_data();
        let now = self.env.ledger().timestamp();

        // 1. config not expired - snapshot reward
        // 2. config expired
        //  2.a data before config expiration - snapshot reward for now, increase block and generate inv
        //  2.b data after config expiration - snapshot reward for config end, increase block, snapshot reward for now, don't increase block

        if now < config.expired_at {
            self.update_rewards_data_snapshot(now, &config, &data, total_shares)
        } else if data.last_time > config.expired_at {
            self.create_new_rewards_data(
                0,
                total_shares,
                PoolRewardData {
                    block: data.block + 1,
                    accumulated: data.accumulated,
                    last_time: now,
                },
            )
        } else {
            self.update_rewards_data_catchup(now, &config, &data, total_shares)
        }
    }

    fn calculate_user_reward(&self, start_block: u64, end_block: u64, user_share: u128) -> u128 {
        let mut reward_inv = 0;
        for block in start_block..end_block + 1 {
            let block_inv = self
                .storage
                .get_reward_inv_data()
                .get(block)
                .expect("Trying to get reward inv");
            reward_inv += block_inv;
        }
        self.storage.bump_reward_inv_data();
        (reward_inv) as u128 * user_share
    }

    pub fn update_user_reward(
        &self,
        pool_data: &PoolRewardData,
        user: &Address,
        user_balance_shares: u128,
    ) -> UserRewardData {
        return match self.storage.get_user_reward_data(user) {
            Some(user_data) => {
                if user_data.pool_accumulated == pool_data.accumulated {
                    // nothing accumulated since last update
                    return user_data;
                }

                if user_balance_shares == 0 {
                    // zero balance, no new reward
                    return self.create_new_user_data(&user, &pool_data, user_data.to_claim);
                }

                let reward = self.calculate_user_reward(
                    user_data.last_block + 1,
                    pool_data.block,
                    user_balance_shares,
                );
                // let new_reward =
                //     (pool_data.accumulated - user_data.pool_accumulated) * user_shares / total_shares;
                self.create_new_user_data(&user, &pool_data, user_data.to_claim + reward)
            }
            None => self.create_new_user_data(&user, &pool_data, 0),
        };
    }

    pub fn get_amount_to_claim(
        &self,
        user: &Address,
        total_shares: u128,
        user_balance_shares: u128,
    ) -> u128 {
        // update pool data & calculate reward
        self.user_reward_data(user, total_shares, user_balance_shares)
            .to_claim
    }

    pub fn claim_reward(
        &self,
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
            last_block,
            pool_accumulated,
            to_claim: 0,
        };
        self.storage.set_user_reward_data(user, &new_data);
        reward_amount
    }

    // private functions

    fn update_reward_inv(&self, accumulated: u128, total_shares: u128) {
        let reward_per_share = if total_shares > 0 {
            accumulated / total_shares
        } else {
            0
        };
        let mut reward_inv_data: Map<u64, u64> = self.storage.get_reward_inv_data();
        reward_inv_data.set(
            self.storage.get_pool_reward_data().block,
            reward_per_share as u64,
        );
        self.storage.set_reward_inv_data(&reward_inv_data);
    }

    fn update_rewards_data_snapshot(
        &self,
        now: u64,
        config: &PoolRewardConfig,
        data: &PoolRewardData,
        total_shares: u128,
    ) -> PoolRewardData {
        let reward_timestamp = now;
        let generated_tokens = to_u128(reward_timestamp - data.last_time) * to_u128(config.tps);
        self.create_new_rewards_data(
            generated_tokens,
            total_shares,
            PoolRewardData {
                block: data.block + 1,
                accumulated: data.accumulated + generated_tokens,
                last_time: now,
            },
        )
    }

    fn create_new_rewards_data(
        &self,
        generated_tokens: u128,
        total_shares: u128,
        new_data: PoolRewardData,
    ) -> PoolRewardData {
        self.storage.set_pool_reward_data(&new_data);
        self.update_reward_inv(generated_tokens, total_shares);
        new_data
    }

    fn update_rewards_data_catchup(
        &self,
        now: u64,
        config: &PoolRewardConfig,
        data: &PoolRewardData,
        total_shares: u128,
    ) -> PoolRewardData {
        let reward_timestamp = config.expired_at;

        let generated_tokens = to_u128(reward_timestamp - data.last_time) * to_u128(config.tps);
        let catchup_data = PoolRewardData {
            block: data.block + 1,
            accumulated: data.accumulated + generated_tokens,
            last_time: config.expired_at,
        };
        self.create_new_rewards_data(generated_tokens, total_shares, catchup_data.clone());
        self.create_new_rewards_data(
            0,
            total_shares,
            PoolRewardData {
                block: catchup_data.block + 1,
                accumulated: catchup_data.accumulated,
                last_time: now,
            },
        )
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
        &self,
        user: &Address,
        total_shares: u128,
        user_balance_shares: u128,
    ) -> UserRewardData {
        self.update_user_reward(
            &self.update_rewards_data(total_shares),
            user,
            user_balance_shares,
        )
    }
}
