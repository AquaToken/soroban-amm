use paste::paste;
use soroban_sdk::panic_with_error;
use soroban_sdk::{contracttype, Address, Env};
use utils::generate_instance_storage_getter;
use utils::storage_errors::StorageError;

use utils::bump::bump_instance;
use utils::{generate_instance_storage_getter_and_setter, generate_instance_storage_setter};

#[derive(Clone)]
#[contracttype]
enum DataKey {
    ConfigStorage,
}

generate_instance_storage_getter_and_setter!(config_storage, DataKey::ConfigStorage, Address);

pub fn has_config_storage(e: &Env) -> bool {
    bump_instance(e);
    e.storage().instance().has(&DataKey::ConfigStorage)
}
