use crate::DataKey;
use soroban_sdk::{Address, BytesN, Env, Vec};

// pool hash

pub fn get_pool_hash(e: &Env) -> BytesN<32> {
    e.storage().instance().bump(6_312_000);
    let hash = e.storage().instance().get(&DataKey::PoolHash);
    match hash {
        Some(value) => value,
        None => {
            panic!("pool hash not initialized")
        }
    }
}

pub fn set_pool_hash(e: &Env, pool_hash: &BytesN<32>) {
    e.storage().instance().bump(6_312_000);
    e.storage().instance().set(&DataKey::PoolHash, pool_hash)
}

// token hash

pub fn get_token_hash(e: &Env) -> BytesN<32> {
    e.storage().instance().bump(6_312_000);
    let hash = e.storage().instance().get(&DataKey::TokenHash);
    match hash {
        Some(value) => value,
        None => {
            panic!("token hash not initialized")
        }
    }
}

pub fn set_token_hash(e: &Env, token_hash: &BytesN<32>) {
    e.storage().instance().bump(6_312_000);
    e.storage().instance().set(&DataKey::TokenHash, token_hash)
}

// pool

pub fn get_pool_id(e: &Env, salt: &BytesN<32>) -> Address {
    let key = DataKey::Pool(salt.clone());
    e.storage().persistent().bump(&key, 6_312_000);
    e.storage().persistent().get(&key).unwrap()
}

pub fn put_pool(e: &Env, salt: &BytesN<32>, pool: &Address) {
    let key = DataKey::Pool(salt.clone());
    e.storage().persistent().set(&key, pool);
    e.storage().persistent().bump(&key, 6_312_000);
}

pub fn has_pool(e: &Env, salt: &BytesN<32>) -> bool {
    e.storage().persistent().has(&DataKey::Pool(salt.clone()))
}

pub fn add_pool_to_list(e: &Env, pool: &Address) {
    // todo: improve pairs storage or get rid of them
    e.storage()
        .persistent()
        .bump(&DataKey::PoolsList, 6_312_000);
    let pairs_list: Option<Vec<Address>> = e.storage().persistent().get(&DataKey::PoolsList);
    match pairs_list {
        Some(value) => {
            let mut new_value = value.clone();
            new_value.append(&Vec::from_array(&e, [pool.clone()]));
            e.storage()
                .persistent()
                .set(&DataKey::PoolsList, &new_value);
        }
        None => {
            let new_value = Vec::from_array(&e, [pool.clone()]);
            e.storage()
                .persistent()
                .set(&DataKey::PoolsList, &new_value);
        }
    }
}

pub fn get_pools_list(e: &Env) -> Vec<Address> {
    e.storage()
        .persistent()
        .bump(&DataKey::PoolsList, 6_312_000);
    e.storage().persistent().get(&DataKey::PoolsList).unwrap()
}
