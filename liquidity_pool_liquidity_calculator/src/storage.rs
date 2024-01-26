use soroban_sdk::{contracttype, Address, Env};

pub const DAY_IN_LEDGERS: u32 = 17280;

pub const PERSISTENT_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - DAY_IN_LEDGERS;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Plane,
}

pub(crate) fn set_plane(e: &Env, plane: &Address) {
    let key = DataKey::Plane;
    e.storage().instance().set(&key, plane);
    e.storage()
        .instance()
        .extend_ttl(PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

pub(crate) fn get_plane(e: &Env) -> Address {
    let key = DataKey::Plane;
    e.storage()
        .instance()
        .get(&key)
        .expect("unable to get plane")
}
