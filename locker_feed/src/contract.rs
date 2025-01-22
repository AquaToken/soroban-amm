use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};
use utils::bump::bump_instance;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    TotalLocked,
}

#[contract]
pub struct LockerFeed;

#[contractimpl]
impl LockerFeed {
    // to be updated periodically by admin
    pub fn set_total_locked(e: Env, _admin: Address, total_locked: u128) {
        bump_instance(&e);
        e.storage()
            .instance()
            .set(&DataKey::TotalLocked, &total_locked);
    }

    // getter
    pub fn get_total_locked(e: Env) -> u128 {
        e.storage()
            .instance()
            .get(&DataKey::TotalLocked)
            .unwrap_or(0)
    }
}
