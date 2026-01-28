use soroban_sdk::{contracttype, BytesN, Env};

#[derive(Clone)]
#[contracttype]
enum DataKey {
    UpgradeDeadline,
    FutureWASM,
}

// upgrade deadline
pub fn get_upgrade_deadline(e: &Env) -> u64 {
    e.storage()
        .instance()
        .get(&DataKey::UpgradeDeadline)
        .unwrap_or(0)
}

pub fn put_upgrade_deadline(e: &Env, value: &u64) {
    e.storage().instance().set(&DataKey::UpgradeDeadline, value);
}

pub fn get_future_wasm(e: &Env) -> Option<BytesN<32>> {
    e.storage().instance().get(&DataKey::FutureWASM)
}

pub fn put_future_wasm(e: &Env, value: &BytesN<32>) {
    e.storage().instance().set(&DataKey::FutureWASM, value);
}
