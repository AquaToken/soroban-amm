use soroban_sdk::{Address, BytesN, Env, Symbol};

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Get contract type symbolic name
    fn contract_name(e: Env) -> Symbol;

    // Upgrade contract with new wasm code
    fn upgrade(e: Env, admin: Address, new_wasm_hash: BytesN<32>);
}
