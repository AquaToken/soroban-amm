use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

const DAY_IN_LEDGERS: u32 = 17280;

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
    let max_ttl = e.storage().max_ttl();
    e.storage()
        .persistent()
        .extend_ttl(key, max_ttl - DAY_IN_LEDGERS, max_ttl);
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
