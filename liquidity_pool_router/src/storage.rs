use crate::constants::{MAX_POOLS_FOR_PAIR, STABLESWAP_MAX_POOLS};
use crate::pool_utils::pool_salt;
use paste::paste;
use soroban_sdk::{contracterror, contracttype, Address, BytesN, Env, Map, Vec};
use utils::bump::{bump_instance, bump_persistent};
use utils::{
    generate_instance_storage_getter, generate_instance_storage_getter_and_setter,
    generate_instance_storage_getter_and_setter_with_default,
    generate_instance_storage_getter_with_default, generate_instance_storage_setter,
};

#[contracttype]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum LiquidityPoolType {
    MissingPool = 0,
    ConstantProduct = 1,
    StableSwap = 2,
    Custom = 3,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LiquidityPoolData {
    pub pool_type: LiquidityPoolType,
    pub address: Address,
}

#[derive(Clone)]
#[contracttype]
enum DataKey {
    TokensPairPools(BytesN<32>),
    TokenHash,
    InitPoolPaymentToken,
    InitPoolPaymentAmount,
    InitPoolPaymentAddress,
    ConstantPoolHash,
    StableSwapPoolHash(u32),
    StableSwapCounter,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PoolError {
    PoolAlreadyExists = 401,
    PoolNotFound = 404,
}

fn get_pools(e: &Env, salt: &BytesN<32>) -> Map<BytesN<32>, LiquidityPoolData> {
    let key = DataKey::TokensPairPools(salt.clone());
    match e.storage().persistent().get(&key) {
        Some(value) => {
            bump_persistent(e, &key);
            value
        }
        None => Map::new(&e),
    }
}

generate_instance_storage_getter_and_setter!(
    constant_product_pool_hash,
    DataKey::ConstantPoolHash,
    BytesN<32>
);
generate_instance_storage_getter_and_setter!(token_hash, DataKey::TokenHash, BytesN<32>);
generate_instance_storage_getter_and_setter!(
    init_pool_payment_token,
    DataKey::InitPoolPaymentToken,
    Address
);
generate_instance_storage_getter_and_setter!(
    init_pool_payment_amount,
    DataKey::InitPoolPaymentAmount,
    u128
);
generate_instance_storage_getter_and_setter!(
    init_pool_payment_address,
    DataKey::InitPoolPaymentAddress,
    Address
);
generate_instance_storage_getter_and_setter_with_default!(
    stableswap_counter,
    DataKey::StableSwapCounter,
    u128,
    0
);

// pool hash
pub fn get_stableswap_pool_hash(e: &Env, num_tokens: u32) -> BytesN<32> {
    if num_tokens == 1 || num_tokens > 3 {
        panic!("unable to find hash for this amount of tokens")
    }
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::StableSwapPoolHash(num_tokens))
        .expect("StableSwapPoolHash hash not initialized")
}

pub fn set_stableswap_pool_hash(e: &Env, num_tokens: u32, pool_hash: &BytesN<32>) {
    bump_instance(e);
    e.storage()
        .instance()
        .set(&DataKey::StableSwapPoolHash(num_tokens), pool_hash)
}

pub fn get_pools_plain(e: &Env, salt: &BytesN<32>) -> Map<BytesN<32>, Address> {
    let pools = get_pools(e, salt);
    let mut pools_plain = Map::new(e);
    for (key, value) in pools {
        pools_plain.set(key, value.address);
    }
    pools_plain
}

pub fn put_pools(e: &Env, salt: &BytesN<32>, pools: &Map<BytesN<32>, LiquidityPoolData>) {
    let key = DataKey::TokensPairPools(salt.clone());
    e.storage().persistent().set(&key, pools);
    bump_persistent(e, &key);
}

pub fn has_pool(e: &Env, salt: &BytesN<32>, pool_index: BytesN<32>) -> bool {
    get_pools(e, salt).contains_key(pool_index)
}

pub fn get_pool(
    e: &Env,
    tokens: Vec<Address>,
    pool_index: BytesN<32>,
) -> Result<Address, PoolError> {
    let salt = pool_salt(&e, tokens);
    let pools = get_pools(e, &salt);
    match pools.contains_key(pool_index.clone()) {
        true => Ok(pools.get(pool_index).unwrap().address),
        false => Err(PoolError::PoolNotFound),
    }
}

pub fn add_pool(
    e: &Env,
    salt: &BytesN<32>,
    pool_index: BytesN<32>,
    pool_type: LiquidityPoolType,
    pool_address: Address,
) {
    let mut pools = get_pools(e, salt);
    pools.set(
        pool_index,
        LiquidityPoolData {
            pool_type,
            address: pool_address,
        },
    );

    if pool_type == LiquidityPoolType::StableSwap {
        let mut stableswap_pools_amt = 0;
        for (_key, value) in pools.clone() {
            if value.pool_type == LiquidityPoolType::StableSwap {
                stableswap_pools_amt += 1;
            }
        }
        if stableswap_pools_amt > STABLESWAP_MAX_POOLS {
            panic!("stableswap pools amount is over max")
        }
    }

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

pub fn get_stableswap_next_counter(e: &Env) -> u128 {
    let value = get_stableswap_counter(e);
    set_stableswap_counter(e, &(value.clone() + 1));
    value
}
