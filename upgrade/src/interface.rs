use soroban_sdk::{Address, BytesN, Env, Symbol};

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Get contract type symbolic name
    fn contract_name(e: Env) -> Symbol;

    // Upgrade contract with new wasm code
    fn commit_upgrade(e: Env, admin: Address, new_wasm_hash: BytesN<32>);
    fn apply_upgrade(e: Env, admin: Address) -> BytesN<32>;
    fn revert_upgrade(e: Env, admin: Address);

    // Emergency mode - bypass upgrade deadline
    fn set_emergency_mode(e: Env, admin: Address, value: bool);
    fn get_emergency_mode(e: Env) -> bool;
}
