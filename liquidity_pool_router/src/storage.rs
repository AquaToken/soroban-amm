use crate::constants::{
    INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, MAX_POOLS_FOR_PAIR, POOL_BUMP_AMOUNT,
    POOL_LIFETIME_THRESHOLD,
};
use crate::storage_types::DataKey;
use soroban_sdk::{Address, BytesN, Env, Map};

// pool hash

pub fn get_constant_product_pool_hash(e: &Env) -> BytesN<32> {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    let hash = e.storage().instance().get(&DataKey::ConstantPoolHash);
    match hash {
        Some(value) => value,
        None => {
            panic!("pool hash not initialized")
        }
    }
}

pub fn set_constant_product_pool_hash(e: &Env, pool_hash: &BytesN<32>) {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    e.storage()
        .instance()
        .set(&DataKey::ConstantPoolHash, pool_hash)
}

// pool hash
pub fn get_stableswap_pool_hash(e: &Env, num_tokens: u32) -> BytesN<32> {
    if num_tokens == 1 || num_tokens > 3 {
        panic!("unable to find hash for this amount of tokens")
    }

    let key = DataKey::StableSwapPoolHash(num_tokens);

    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    let hash = e.storage().instance().get(&key);
    match hash {
        Some(value) => value,
        None => {
            panic!("pool hash not initialized")
        }
    }
}

pub fn set_stableswap_pool_hash(e: &Env, num_tokens: u32, pool_hash: &BytesN<32>) {
    let key = DataKey::StableSwapPoolHash(num_tokens);

    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
    e.storage().instance().set(&key, pool_hash)
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

pub fn get_pools(e: &Env, salt: &BytesN<32>) -> Map<BytesN<32>, Address> {
    let key = DataKey::TokensPairPools(salt.clone());
    match e.storage().persistent().get(&key) {
        Some(value) => {
            e.storage()
                .persistent()
                .bump(&key, POOL_LIFETIME_THRESHOLD, POOL_BUMP_AMOUNT);
            value
        }
        None => Map::new(&e),
    }
}

pub fn put_pools(e: &Env, salt: &BytesN<32>, pools: &Map<BytesN<32>, Address>) {
    let key = DataKey::TokensPairPools(salt.clone());
    e.storage().persistent().set(&key, pools);
    e.storage()
        .persistent()
        .bump(&key, POOL_LIFETIME_THRESHOLD, POOL_BUMP_AMOUNT);
}

pub fn has_pools(e: &Env, salt: &BytesN<32>) -> bool {
    let pools = get_pools(e, salt);
    pools.len() > 0
}

pub fn has_pool(e: &Env, salt: &BytesN<32>, pool_index: BytesN<32>) -> bool {
    let pools = get_pools(e, salt);
    pools.contains_key(pool_index)
}

pub fn get_pool(e: &Env, salt: &BytesN<32>, pool_index: BytesN<32>) -> Address {
    let pools = get_pools(e, salt);
    pools
        .get(pool_index)
        .unwrap_or(Address::from_contract_id(&BytesN::from_array(&e, &[0; 32])))
}

pub fn add_pool(e: &Env, salt: &BytesN<32>, pool_index: BytesN<32>, pool_address: Address) {
    let mut pools = get_pools(e, salt);
    pools.set(pool_index, pool_address);
    if pools.len() > MAX_POOLS_FOR_PAIR {
        panic!("pools amount is over max")
    }
    put_pools(e, salt, &pools);
}

pub fn remove_pool(e: &Env, salt: &BytesN<32>, pool_index: BytesN<32>) {
    let mut pools = get_pools(e, salt);
    pools.remove(pool_index);
    put_pools(e, salt, &pools);
}
