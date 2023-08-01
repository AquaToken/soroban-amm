use crate::DataKey;
use soroban_sdk::{Address, Env};

pub fn has_admin(e: &Env) -> bool {
    e.storage().instance().bump(6_312_000);
    e.storage().instance().has(&DataKey::Admin)
}

pub fn get_admin(e: &Env) -> Address {
    e.storage().instance().bump(6_312_000);
    e.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_admin(e: &Env, admin: &Address) {
    e.storage().instance().bump(6_312_000);
    e.storage().instance().set(&DataKey::Admin, admin)
}

pub fn require_admin(e: &Env) {
    if !has_admin(&e) {
        panic!("admin not set")
    }
    let admin = get_admin(&e);
    admin.require_auth();
}
