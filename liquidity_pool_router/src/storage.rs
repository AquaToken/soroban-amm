use crate::constants::{MAX_POOLS_FOR_PAIR, STABLE_SWAP_MAX_POOLS};
use crate::pool_utils::pool_salt;
use soroban_sdk::{contracterror, contracttype, Address, BytesN, Env, Map, Vec};
use utils::bump::{bump_instance, bump_persistent};

// todo: replace `as u32` usages with something more meaningful
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
    ConstantPoolHash,
    StableSwapPoolHash(u32),
    StableSwapCounter,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum PoolError {
    PoolNotFound = 400,
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

pub fn set_constant_product_pool_hash(e: &Env, pool_hash: &BytesN<32>) {
    bump_instance(e);
    e.storage()
        .instance()
        .set(&DataKey::ConstantPoolHash, pool_hash)
}

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

// token hash

pub fn get_token_hash(e: &Env) -> BytesN<32> {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::TokenHash)
        .expect("Token hash not initialized")
}

pub fn set_stableswap_pool_hash(e: &Env, num_tokens: u32, pool_hash: &BytesN<32>) {
    bump_instance(e);
    e.storage()
        .instance()
        .set(&DataKey::StableSwapPoolHash(num_tokens), pool_hash)
}

// token hash
pub fn set_token_hash(e: &Env, token_hash: &BytesN<32>) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::TokenHash, token_hash)
}

// pool payment config

pub fn get_init_pool_payment_token(e: &Env) -> Address {
    bump_instance(&e);
    let token = e.storage().instance().get(&DataKey::InitPoolPaymentToken);
    match token {
        Some(value) => value,
        None => {
            panic!("init pool payment token not initialized")
        }
    }
}

pub fn set_init_pool_payment_token(e: &Env, token: &Address) {
    bump_instance(&e);
    e.storage()
        .instance()
        .set(&DataKey::InitPoolPaymentToken, token)
}

pub fn get_init_pool_payment_amount(e: &Env) -> i128 {
    bump_instance(&e);
    let token = e.storage().instance().get(&DataKey::InitPoolPaymentAmount);
    match token {
        Some(value) => value,
        None => {
            panic!("init pool payment token not initialized")
        }
    }
}

pub fn set_init_pool_payment_amount(e: &Env, amount: &i128) {
    bump_instance(&e);
    e.storage()
        .instance()
        .set(&DataKey::InitPoolPaymentAmount, amount)
}

// pool
pub fn get_constant_product_pool_hash(e: &Env) -> BytesN<32> {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::ConstantPoolHash)
        .expect("Pool hash not initialized")
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

pub fn get_pool_safe(e: &Env, salt: &BytesN<32>, pool_index: BytesN<32>) -> Address {
    let pools = get_pools(e, salt);
    pools
        .get(pool_index)
        .unwrap_or(LiquidityPoolData {
            pool_type: LiquidityPoolType::MissingPool,
            address: Address::from_contract_id(&BytesN::from_array(&e, &[0; 32])),
        })
        .address
}

pub fn get_pool(
    e: &Env,
    tokens: Vec<Address>,
    pool_index: BytesN<32>,
) -> Result<Address, PoolError> {
    let salt = pool_salt(&e, tokens);
    match has_pool(&e, &salt, pool_index.clone()) {
        true => Ok(get_pool_safe(&e, &salt, pool_index)),
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
        let mut stable_swap_pools_amt = 0;
        for (_key, value) in pools.clone() {
            if value.pool_type == LiquidityPoolType::StableSwap {
                stable_swap_pools_amt += 1;
            }
        }
        if stable_swap_pools_amt >= STABLE_SWAP_MAX_POOLS {
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

pub fn get_stable_swap_next_counter(e: &Env) -> u128 {
    bump_instance(&e);
    let value = e
        .storage()
        .instance()
        .get(&DataKey::StableSwapCounter)
        .unwrap_or(0);
    e.storage()
        .instance()
        .set(&DataKey::StableSwapCounter, &(value.clone() + 1));
    value
}
