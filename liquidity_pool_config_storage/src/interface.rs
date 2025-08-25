use soroban_sdk::{Address, Env};

pub trait ConfigStorageInterface {
    fn init_config_storage(e: Env, admin: Address, config_storage: Address);
}
