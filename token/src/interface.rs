use soroban_sdk::{Address, BytesN, Env};

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Upgrade contract with new wasm code
    fn upgrade(e: Env, admin: Address, new_wasm_hash: BytesN<32>);
}
