use soroban_sdk::{Address, BytesN, Env};

pub trait AdminInterface {
    // Initializes the admin user.
    fn init_admin(e: Env, account: Address);
}

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Upgrade contract with new wasm code
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
}
