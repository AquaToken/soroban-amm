use soroban_sdk::{Address, BytesN, Env, Symbol, Vec};

pub trait PlaneInterface {
    // Initializes the admin user.
    fn init_admin(e: Env, account: Address);

    // update pool stored data. any pool can use it to store it's information
    fn update(e: Env, pool: Address, pool_type: Symbol, init_args: Vec<u128>, reserves: Vec<u128>);

    // get details for many pools: type string representation, pool parameters and reserves amount
    fn get(e: Env, pools: Vec<Address>) -> Vec<(Symbol, Vec<u128>, Vec<u128>)>;
}

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Upgrade contract with new wasm code
    fn commit_upgrade(e: Env, admin: Address, new_wasm_hash: BytesN<32>);
    fn apply_upgrade(e: Env, admin: Address) -> BytesN<32>;
    fn revert_upgrade(e: Env, admin: Address);

    // Emergency mode - bypass upgrade deadline
    fn set_emergency_mode(e: Env, admin: Address, value: bool);
    fn get_emergency_mode(e: Env) -> bool;
}
