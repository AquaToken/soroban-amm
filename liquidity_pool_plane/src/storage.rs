use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

const DAY_IN_LEDGERS: u32 = 17280;
pub const MONTH_IN_LEDGERS: u32 = DAY_IN_LEDGERS * 30;

// Persistent TTL
pub const MAX_PERSISTENT_TTL: u32 = MONTH_IN_LEDGERS * 6;
pub const PERSISTENT_TTL_THRESHOLD: u32 = MAX_PERSISTENT_TTL - MONTH_IN_LEDGERS;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    PoolData(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PoolPlane {
    pub pool_type: Symbol,
    pub init_args: Vec<u128>,
    pub reserves: Vec<u128>,
}

fn bump_persistent(e: &Env, key: &DataKey) {
    e.storage()
        .persistent()
        .extend_ttl(key, PERSISTENT_TTL_THRESHOLD, MAX_PERSISTENT_TTL);
}

fn get_default_pool(e: &Env) -> PoolPlane {
    PoolPlane {
        pool_type: symbol_short!("standard"),
        init_args: Vec::from_array(e, [30_u128]),
        reserves: Vec::from_array(e, [0_u128, 0_u128]),
    }
}

pub(crate) fn update(e: &Env, contract: Address, pool: &PoolPlane) {
    let key = DataKey::PoolData(contract);
    e.storage().persistent().set(&key, pool);
    bump_persistent(e, &key);
}

pub(crate) fn get(e: &Env, contract: Address) -> PoolPlane {
    let key = DataKey::PoolData(contract);

    // return standard pool with zero reserves if data not provided
    if !e.storage().persistent().has(&key) {
        return get_default_pool(e);
    }
    bump_persistent(e, &key);
    e.storage().persistent().get(&key).unwrap()
}
