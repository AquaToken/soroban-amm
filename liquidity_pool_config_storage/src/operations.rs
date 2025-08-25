use crate::storage;
use soroban_sdk::{panic_with_error, Address, Env, Symbol, Val, Vec};
use utils::storage_errors::StorageError;

pub fn init_config_storage(e: &Env, value: &Address) {
    if storage::has_config_storage(e) {
        panic_with_error!(e, StorageError::AlreadyInitialized);
    }
    storage::set_config_storage(e, value);
}

pub fn get_config_storage(e: &Env) -> Address {
    storage::get_config_storage(e)
}

pub fn get_value_safe(e: &Env, key: Val) -> Option<Val> {
    e.invoke_contract(
        &storage::get_config_storage(e),
        &Symbol::new(&e, "get_value"),
        Vec::from_array(&e, [key]),
    )
}

pub fn get_value(e: &Env, key: Val) -> Val {
    match get_value_safe(e, key) {
        Some(val) => val,
        None => panic_with_error!(e, StorageError::ValueMissing),
    }
}

pub fn set_value(e: &Env, admin: &Address, key: Val, value: Val) {
    e.invoke_contract::<Val>(
        &storage::get_config_storage(e),
        &Symbol::new(&e, "set_value"),
        Vec::from_array(&e, [admin.to_val(), key, value]),
    );
}
