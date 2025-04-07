use soroban_sdk::{Address, Env, Map, Symbol, Vec};

pub trait AdminInterfaceTrait {
    // Set privileged addresses
    fn set_privileged_addrs(e: Env, admin: Address, operations_admin: Address);

    // Get map of privileged roles
    fn get_privileged_addrs(e: Env) -> Map<Symbol, Vec<Address>>;
}
