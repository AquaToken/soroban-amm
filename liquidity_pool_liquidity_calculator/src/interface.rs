use soroban_sdk::{Address, BytesN, Env, Vec, U256};

pub trait Calculator {
    fn init_admin(e: Env, account: Address);
    fn set_pools_plane(e: Env, admin: Address, plane: Address);
    fn get_pools_plane(e: Env) -> Address;
    fn get_liquidity(e: Env, pools: Vec<Address>) -> Vec<U256>;
}

pub trait UpgradeableContractTrait {
    // Get contract version
    fn version() -> u32;

    // Upgrade contract with new wasm code
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
}
