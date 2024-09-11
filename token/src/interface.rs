use soroban_sdk::{BytesN, Env};

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Upgrade contract with new wasm code
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
}
