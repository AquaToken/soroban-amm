use crate::constants::{MAX_POOLS_FOR_PAIR, STABLESWAP_MAX_POOLS, USER_POOLS_PAGE_SIZE};
use crate::errors::LiquidityPoolRouterError;
use crate::pool_utils::get_tokens_salt;
use paste::paste;
use soroban_sdk::{contracterror, contracttype, panic_with_error, Address, BytesN, Env, Map, Vec};
use utils::bump::{bump_instance, bump_persistent};
use utils::storage_errors::StorageError;
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
    TokensSet(u128),
    TokensSetCounter,
    TokensSetPools(BytesN<32>),
    TokenHash,
    InitPoolPaymentToken,
    InitPoolPaymentAmount,
    InitPoolPaymentAddress,
    ConstantPoolHash,
    StableSwapPoolHash,
    PoolCounter,
    PoolPlane,
    SwapRouter,
    UserPools(Address, u32),
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PoolError {
    PoolAlreadyExists = 401,
    PoolNotFound = 404,
}

fn get_pools(e: &Env, salt: &BytesN<32>) -> Map<BytesN<32>, LiquidityPoolData> {
    let key = DataKey::TokensSetPools(salt.clone());
    match e.storage().persistent().get(&key) {
        Some(value) => {
            bump_persistent(e, &key);
            value
        }
        None => Map::new(e),
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
    pool_counter,
    DataKey::PoolCounter,
    u128,
    0
);
generate_instance_storage_getter_and_setter_with_default!(
    tokens_set_count,
    DataKey::TokensSetCounter,
    u128,
    0
);
generate_instance_storage_getter_and_setter!(pool_plane, DataKey::PoolPlane, Address);
generate_instance_storage_getter_and_setter!(swap_router, DataKey::SwapRouter, Address);

// pool hash
pub fn get_stableswap_pool_hash(e: &Env) -> BytesN<32> {
    bump_instance(e);
    match e.storage().instance().get(&DataKey::StableSwapPoolHash) {
        Some(v) => v,
        None => panic_with_error!(&e, LiquidityPoolRouterError::StableswapHashMissing),
    }
}

pub fn set_stableswap_pool_hash(e: &Env, pool_hash: &BytesN<32>) {
    bump_instance(e);
    e.storage()
        .instance()
        .set(&DataKey::StableSwapPoolHash, pool_hash)
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
    let key = DataKey::TokensSetPools(salt.clone());
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
    let salt = get_tokens_salt(e, tokens);
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
            panic_with_error!(&e, LiquidityPoolRouterError::StableswapPoolsOverMax);
        }
    }

    if pools.len() > MAX_POOLS_FOR_PAIR {
        panic_with_error!(&e, LiquidityPoolRouterError::PoolsOverMax);
    }
    put_pools(e, salt, &pools);
}

// remember unique tokens set
pub fn add_tokens_set(e: &Env, tokens: &Vec<Address>) {
    let salt = get_tokens_salt(e, tokens.clone());
    let pools = get_pools(e, &salt);
    if pools.len() > 0 {
        return;
    }

    let tokens_set_count = get_tokens_set_count(e);
    put_tokens_set(e, tokens_set_count, &tokens);
    set_tokens_set_count(e, &(tokens_set_count + 1));
}

pub fn remove_pool(e: &Env, salt: &BytesN<32>, pool_index: BytesN<32>) {
    let mut pools = get_pools(e, salt);
    pools.remove(pool_index);
    put_pools(e, salt, &pools);
}

pub fn get_pool_next_counter(e: &Env) -> u128 {
    let value = get_pool_counter(e);
    set_pool_counter(e, &(value + 1));
    value
}

pub fn get_tokens_set(e: &Env, index: u128) -> Vec<Address> {
    let key = DataKey::TokensSet(index);
    match e.storage().persistent().get(&key) {
        Some(v) => {
            bump_persistent(e, &key);
            v
        }
        None => panic_with_error!(&e, StorageError::ValueNotInitialized),
    }
}

pub fn put_tokens_set(e: &Env, index: u128, tokens: &Vec<Address>) {
    let key = DataKey::TokensSet(index);
    e.storage().persistent().set(&key, tokens);
    bump_persistent(e, &key);
}

pub fn get_user_pools(
    e: &Env,
    user: &Address,
    page: u32,
) -> Vec<(Vec<Address>, BytesN<32>, Address)> {
    let key = DataKey::UserPools(user.clone(), page);
    match e.storage().persistent().get(&key) {
        Some(v) => {
            bump_persistent(e, &key);
            v
        }
        None => Vec::new(e),
    }
}

pub fn set_user_pools(
    e: &Env,
    user: &Address,
    page: u32,
    user_pools: &Vec<(Vec<Address>, BytesN<32>, Address)>,
) {
    let key = DataKey::UserPools(user.clone(), page);
    e.storage().persistent().set(&key, user_pools);
    bump_persistent(e, &key);
}

pub fn add_user_pool(
    e: &Env,
    user: &Address,
    page: u32,
    tokens: &Vec<Address>,
    pool_index: &BytesN<32>,
    pool_address: &Address,
) {
    let mut user_pools = get_user_pools(e, user, page.clone());
    let pool_data = (tokens.clone(), pool_index.clone(), pool_address.clone());

    user_pools.push_back(pool_data);
    if user_pools.len() > USER_POOLS_PAGE_SIZE {
        panic_with_error!(&e, LiquidityPoolRouterError::UserPoolsPageFull);
    }
    set_user_pools(e, user, page, &user_pools);
}

pub fn remove_user_pool(
    e: &Env,
    user: &Address,
    page: u32,
    tokens: &Vec<Address>,
    pool_index: &BytesN<32>,
    pool_address: &Address,
) {
    let mut user_pools = get_user_pools(e, user, page.clone());
    let pool_data = (tokens.clone(), pool_index.clone(), pool_address.clone());
    let pool_index = user_pools.first_index_of(&pool_data);

    match pool_index {
        Some(i) => {
            user_pools.remove(i);
            set_user_pools(e, user, page, &user_pools);
        }
        None => panic_with_error!(&e, LiquidityPoolRouterError::UserPoolsNothingToLeave),
    }
}
