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
        let effective_total_share = total_share - self.get_total_excluded_shares(storage);
        (effective_balance, effective_total_share)
    }
}
