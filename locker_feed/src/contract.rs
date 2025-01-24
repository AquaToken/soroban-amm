use soroban_sdk::{contract, contractimpl, contracttype, Address, Env};
use utils::bump::bump_instance;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    TotalSupply,
}

#[contract]
pub struct LockerFeed;

#[contractimpl]
impl LockerFeed {
    // to be updated periodically by admin
    pub fn set_total_supply(e: Env, _admin: Address, total_supply: u128) {
        bump_instance(&e);
        e.storage()
            .instance()
            .set(&DataKey::TotalSupply, &total_supply);
    }

    // getter
    pub fn total_supply(e: Env) -> u128 {
        e.storage()
            .instance()
            .get(&DataKey::TotalSupply)
            .unwrap_or(0)
    }
}
