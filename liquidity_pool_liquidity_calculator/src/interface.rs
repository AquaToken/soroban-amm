use soroban_sdk::{Address, BytesN, Env, Vec, U256};

pub trait Calculator {
    fn init_admin(e: Env, account: Address);
    fn set_pools_plane(e: Env, admin: Address, plane: Address);
    fn get_pools_plane(e: Env) -> Address;
    fn get_liquidity(e: Env, pools: Vec<Address>) -> Vec<U256>;
}

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Upgrade contract with new wasm code
    fn commit_upgrade(e: Env, admin: Address, new_wasm_hash: BytesN<32> );
    fn apply_upgrade(e: Env, admin: Address) -> BytesN<32>;
    fn revert_upgrade(e: Env, admin: Address);

    // Emergency mode - bypass upgrade deadline
    fn set_emergency_admin(e: Env, admin: Address, emergency_admin: Address);
    fn set_emergency_mode(e: Env, admin: Address, value: bool);
    fn get_emergency_mode(e: Env) -> bool;
}

