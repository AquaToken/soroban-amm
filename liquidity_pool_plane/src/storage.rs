use soroban_sdk::{contracttype, symbol_short, Address, Env, Symbol, Vec};

pub const DAY_IN_LEDGERS: u32 = 17280;

pub const PERSISTENT_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub const PERSISTENT_LIFETIME_THRESHOLD: u32 = PERSISTENT_BUMP_AMOUNT - DAY_IN_LEDGERS;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    PoolData(Address),
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Pool {
    pub pool_type: Symbol,
    pub init_args: Vec<u128>,
    pub reserves: Vec<u128>,
}

pub(crate) fn update(e: &Env, contract: Address, pool: &Pool) {
    let key = DataKey::PoolData(contract);
    e.storage().persistent().set(&key, pool);
    e.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );
}

pub(crate) fn get(e: &Env, contract: Address) -> Pool {
    let key = DataKey::PoolData(contract);
    if !e.storage().persistent().has(&key) {
        return Pool {
            pool_type: symbol_short!("standard"),
            init_args: Vec::from_array(e, [30_u128]),
            reserves: Vec::from_array(e, [0_u128, 0_u128]),
        };
    }
    e.storage().persistent().extend_ttl(
        &key,
        PERSISTENT_LIFETIME_THRESHOLD,
        PERSISTENT_BUMP_AMOUNT,
    );

    e.storage().persistent().get(&key).unwrap()
}
