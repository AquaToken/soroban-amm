use crate::constants::{INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD};
use crate::storage_types::DataKey;
use soroban_sdk::{Address, Env};

pub fn has_admin(e: &Env) -> bool {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    e.storage().instance().has(&DataKey::Admin)
}

pub fn get_admin(e: &Env) -> Address {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    e.storage().instance().get(&DataKey::Admin).unwrap()
}

pub fn set_admin(e: &Env, admin: &Address) {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    e.storage().instance().set(&DataKey::Admin, admin)
}

pub fn require_admin(e: &Env) {
    if !has_admin(&e) {
        panic!("admin not set")
    }
    let admin = get_admin(&e);
    admin.require_auth();
}

pub fn check_admin(e: &Env, user: &Address) {
    if !has_admin(&e) {
        panic!("admin not set")
    }
    let admin = get_admin(e);
    if admin != user.clone() {
        panic!("user is not admin")
    }
}
