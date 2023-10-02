use crate::storage_types::{
    DataKey, INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, POOL_BUMP_AMOUNT,
    POOL_LIFETIME_THRESHOLD,
};
use soroban_sdk::{Address, BytesN, Env, Vec};

// pool hash

pub fn get_pool_hash(e: &Env) -> BytesN<32> {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    let hash = e.storage().instance().get(&DataKey::PoolHash);
    match hash {
        Some(value) => value,
        None => {
            panic!("pool hash not initialized")
        }
    }
}

pub fn set_pool_hash(e: &Env, pool_hash: &BytesN<32>) {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    e.storage().instance().set(&DataKey::PoolHash, pool_hash)
}

// token hash

pub fn get_token_hash(e: &Env) -> BytesN<32> {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    let hash = e.storage().instance().get(&DataKey::TokenHash);
    match hash {
        Some(value) => value,
        None => {
            panic!("token hash not initialized")
        }
    }
}

pub fn set_token_hash(e: &Env, token_hash: &BytesN<32>) {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    e.storage().instance().set(&DataKey::TokenHash, token_hash)
}

// reward token

pub fn get_reward_token(e: &Env) -> Address {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    let reward_token = e.storage().instance().get(&DataKey::RewardToken);
    match reward_token {
        Some(value) => value,
        None => {
            panic!("reward token not initialized")
        }
    }
}

pub fn set_reward_token(e: &Env, reward_token: &Address) {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    e.storage()
        .instance()
        .set(&DataKey::RewardToken, reward_token)
}

// pool

pub fn get_pool_id(e: &Env, salt: &BytesN<32>) -> Address {
    let key = DataKey::Pool(salt.clone());
    e.storage()
        .persistent()
        .bump(&key, POOL_LIFETIME_THRESHOLD, POOL_BUMP_AMOUNT);
    e.storage().persistent().get(&key).unwrap()
}

pub fn put_pool(e: &Env, salt: &BytesN<32>, pool: &Address) {
    let key = DataKey::Pool(salt.clone());
    e.storage().persistent().set(&key, pool);
    e.storage()
        .persistent()
        .bump(&key, POOL_LIFETIME_THRESHOLD, POOL_BUMP_AMOUNT);
}

pub fn has_pool(e: &Env, salt: &BytesN<32>) -> bool {
    e.storage().persistent().has(&DataKey::Pool(salt.clone()))
}

pub fn add_pool_to_list(e: &Env, pool: &Address) {
    // todo: improve pairs storage or get rid of them
    let pairs_list: Option<Vec<Address>> = e.storage().persistent().get(&DataKey::PoolsList);
    match pairs_list {
        Some(value) => {
            e.storage().persistent().bump(
                &DataKey::PoolsList,
                POOL_LIFETIME_THRESHOLD,
                POOL_BUMP_AMOUNT,
            );
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
    e.storage().persistent().bump(
        &DataKey::PoolsList,
        POOL_LIFETIME_THRESHOLD,
        POOL_BUMP_AMOUNT,
    );
    e.storage().persistent().get(&DataKey::PoolsList).unwrap()
}
