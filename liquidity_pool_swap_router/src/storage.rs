use soroban_sdk::{contracttype, panic_with_error, Address, Env};
use utils::constant::DAY_IN_LEDGERS;
use utils::storage_errors::StorageError;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Plane,
}

pub(crate) fn set_plane(e: &Env, plane: &Address) {
    let key = DataKey::Plane;
    let max_ttl = e.storage().max_ttl();
    e.storage()
        .instance()
        .extend_ttl(max_ttl - DAY_IN_LEDGERS, max_ttl);
    e.storage().instance().set(&key, plane);
}

pub(crate) fn get_plane(e: &Env) -> Address {
    let key = DataKey::Plane;
    match e.storage().instance().get(&key) {
        Some(v) => v,
        None => panic_with_error!(e, StorageError::ValueNotInitialized),
    }
}
