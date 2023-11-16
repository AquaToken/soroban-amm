use crate::constants::{
    INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, MAX_POOLS_FOR_PAIR, POOL_BUMP_AMOUNT,
    POOL_LIFETIME_THRESHOLD, STABLE_SWAP_MAX_POOLS,
};
use crate::storage_types::DataKey;
use paste::paste;
use soroban_sdk::{contracttype, Address, BytesN, Env, Map, Vec};

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

fn bump_instance_storage(e: &Env) {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

// todo: try to move macros & utils to separate file
macro_rules! generate_instance_storage_setter {
    ($attr_name:ident, $key:expr, $data_type:ty) => {
        paste! {
            pub fn [<set_ $attr_name>](e: &Env, $attr_name: &$data_type) {
                bump_instance_storage(e);
                e.storage()
                    .instance()
                    .set(&$key, $attr_name)
            }
        }
    };
}

macro_rules! generate_instance_storage_getter {
    ($attr_name:ident, $key:expr, $data_type:ty) => {
        paste! {
            pub fn [<get_ $attr_name>](e: &Env) -> $data_type {
                    bump_instance_storage(e);
                let value_result = e.storage().instance().get(&$key);
                match value_result {
                    Some(value) => value,
                    None => {
                        panic!("{} not initialized", stringify!($attr_name))
                    }
                }
            }
        }
    };
}

macro_rules! generate_instance_storage_getter_with_default {
    ($attr_name:ident, $key:expr, $data_type:ty, $default:expr) => {
        paste! {
            pub fn [<get_ $attr_name>](e: &Env) -> $data_type {
                    bump_instance_storage(e);
                e.storage().instance().get(&$key).unwrap_or($default)
            }
        }
    };
}

macro_rules! generate_instance_storage_getter_and_setter {
    ($attr_name:ident, $key:expr, $data_type:ty) => {
        generate_instance_storage_getter!($attr_name, $key, $data_type);
        generate_instance_storage_setter!($attr_name, $key, $data_type);
    };
}

macro_rules! generate_instance_storage_getter_and_setter_with_default {
    ($attr_name:ident, $key:expr, $data_type:ty, $default:expr) => {
        generate_instance_storage_getter_with_default!($attr_name, $key, $data_type, $default);
        generate_instance_storage_setter!($attr_name, $key, $data_type);
    };
}

generate_instance_storage_getter_and_setter!(
    constant_product_pool_hash,
    DataKey::ConstantPoolHash,
    BytesN<32>
);
generate_instance_storage_getter_and_setter!(token_hash, DataKey::TokenHash, BytesN<32>);
generate_instance_storage_getter_and_setter!(reward_token, DataKey::RewardToken, Address);
generate_instance_storage_getter_and_setter!(
    init_pool_payment_token,
    DataKey::InitPoolPaymentToken,
    Address
);
generate_instance_storage_getter_and_setter!(
    init_pool_payment_amount,
    DataKey::InitPoolPaymentAmount,
    i128
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

    let key = DataKey::StableSwapPoolHash(num_tokens);

    bump_instance_storage(e);
    let hash = e.storage().instance().get(&key);
    match hash {
        Some(value) => value,
        None => {
            panic!("pool hash not initialized")
        }
    }
}

pub fn set_stableswap_pool_hash(e: &Env, num_tokens: u32, pool_hash: &BytesN<32>) {
    bump_instance_storage(e);
    e.storage()
        .instance()
        .set(&DataKey::StableSwapPoolHash(num_tokens), pool_hash)
}

// pool

pub fn get_pools(e: &Env, salt: &BytesN<32>) -> Map<BytesN<32>, LiquidityPoolData> {
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

pub fn get_pool(e: &Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address {
    let salt = crate::utils::pool_salt(&e, tokens);
    if !has_pool(&e, &salt, pool_index.clone()) {
        panic!("pool not exists")
    }
    get_pool_safe(&e, &salt, pool_index)
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
    let value = get_stableswap_counter(e);
    set_stableswap_counter(e, &(value.clone() + 1));
    value
}
