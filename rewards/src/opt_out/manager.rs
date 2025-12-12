use crate::manager::ManagerPlugin;
use crate::storage::Storage;
use soroban_sdk::Address;

pub(crate) struct OptOutManagerPlugin;

impl OptOutManagerPlugin {
    pub fn get_user_rewards_state(&self, storage: &Storage, user: &Address) -> bool {
        storage.get_user_rewards_state(user)
    }

    pub fn set_user_rewards_state(&self, storage: &Storage, user: &Address, value: bool) {
        storage.set_user_rewards_state(user, value)
    }

    pub fn get_total_excluded_shares(&self, storage: &Storage) -> u128 {
        storage.get_total_excluded_shares()
    }
}

impl ManagerPlugin for OptOutManagerPlugin {
    fn calculate_effective_balance(
        &self,
        storage: &Storage,
        user: &Address,
        share_balance: u128,
        total_share: u128,
    ) -> (u128, u128) {
        let effective_balance = match storage.get_user_rewards_state(user) {
            true => share_balance,
            false => 0,
        };

        // Leave `total_share` unchanged so a user's working balance depends only on their
        // position and boost, not on other accounts' opt-out status. Exclusion removes the
        // account from rewards by zeroing `effective_balance`; working supply is adjusted
        // through each user's own checkpoint instead of mutating the shared total.
        (effective_balance, total_share)
    }
}
