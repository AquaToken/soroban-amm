use soroban_sdk::{Address, BytesN, Env, Vec};

pub trait RouterInterface {
    // Initialize admin user. Will panic if called twice
    fn init_admin(e: Env, account: Address);

    // configure pools plane address to be used as lightweight proxy to optimize instructions & batch operations
    fn set_pools_plane(e: Env, admin: Address, plane: Address);

    // get pools plane address
    fn get_pools_plane(e: Env) -> Address;

    // Estimate best swap among provided and amount of coins to retrieve using swap function
    fn estimate_swap(
        e: Env,
        pools: Vec<Address>,
        in_idx: u32,
        out_idx: u32,
        in_amount: u128,
    ) -> (Address, u128);
}

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Upgrade contract with new wasm code
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
}
