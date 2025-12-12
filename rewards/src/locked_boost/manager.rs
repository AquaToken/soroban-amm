use crate::locked_boost::boost_feed::RewardBoostFeedClient;
use crate::manager::ManagerPlugin;
use crate::storage::{BoostFeedStorageTrait, BoostTokenStorageTrait, Storage};
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{Address, Env};

pub(crate) struct BoostManagerPlugin {
    env: Env,
}

impl BoostManagerPlugin {
    pub fn new(e: &Env) -> Self {
        Self { env: e.clone() }
    }

    // ------------------------------------
    // Basic getters for boost balances
    // ------------------------------------

    pub fn get_user_boost_balance(&self, storage: &Storage, user: &Address) -> u128 {
        if storage.has_reward_boost_token() {
            match SorobanTokenClient::new(&self.env, &storage.get_reward_boost_token())
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

    pub fn get_total_locked(&self, storage: &Storage) -> u128 {
        if storage.has_reward_boost_feed() {
            RewardBoostFeedClient::new(&self.env, &storage.get_reward_boost_feed()).total_supply()
        } else {
            0
        }
    }
}

impl ManagerPlugin for BoostManagerPlugin {
    fn calculate_effective_balance(
        &self,
        storage: &Storage,
        user: &Address,
        share_balance: u128,
        total_share: u128,
    ) -> u128 {
        // b_u = 2.5 * min(0.4 * b_u + 0.6 * S * w_i / W, b_u)
        let lock_balance = self.get_user_boost_balance(storage, &user);
        let total_locked = self.get_total_locked(storage);

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
}
