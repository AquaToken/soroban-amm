use soroban_sdk::{contracttype, Address, Env};

pub const DAY_IN_LEDGERS: u32 = 17280;

pub const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Plane,
}

pub(crate) fn set_plane(e: &Env, plane: &Address) {
    let key = DataKey::Plane;
    e.storage()
        .instance()
        .extend_ttl(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    e.storage().instance().set(&key, plane);
}

pub(crate) fn get_plane(e: &Env) -> Address {
    let key = DataKey::Plane;
    e.storage()
        .instance()
        .get(&key)
        .expect("unable to get plane")
}
