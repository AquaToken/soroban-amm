use crate::events::{Events, LiquidityPoolRouterEvents};
use crate::pool_contract::StandardLiquidityPoolClient;
use crate::rewards::get_rewards_manager;
use crate::storage::{
    add_pool, get_constant_product_pool_hash, get_pools_plain, get_stableswap_next_counter,
    get_stableswap_pool_hash, get_token_hash, LiquidityPoolType,
};
use access_control::access::{AccessControl, AccessControlTrait};
use rewards::storage::RewardsStorageTrait;
use soroban_sdk::{
    symbol_short, xdr::ToXdr, Address, Bytes, BytesN, Env, IntoVal, Map, Symbol, Val, Vec,
};

pub fn get_standard_pool_salt(e: &Env, fee_fraction: &u32) -> BytesN<32> {
    // fixme: fee_fraction is mutable for pool. hash collision is possible to happen
    let mut salt = Bytes::new(e);
    salt.append(&symbol_short!("standard").to_xdr(&e));
    salt.append(&symbol_short!("0x00").to_xdr(&e));
    salt.append(&fee_fraction.to_xdr(&e));
    salt.append(&symbol_short!("0x00").to_xdr(&e));
    e.crypto().sha256(&salt)
}

pub fn get_stableswap_pool_salt(e: &Env) -> BytesN<32> {
    let mut salt = Bytes::new(e);
    salt.append(&symbol_short!("0x00").to_xdr(e));
    salt.append(&get_stableswap_next_counter(e).to_xdr(&e));
    salt.append(&symbol_short!("0x00").to_xdr(e));
    e.crypto().sha256(&salt)
}

pub fn get_custom_salt(e: &Env, pool_type: &Symbol, init_args: &Vec<Val>) -> BytesN<32> {
    let mut salt = Bytes::new(e);
    salt.append(&pool_type.to_xdr(e));
    salt.append(&symbol_short!("0x00").to_xdr(e));
    for arg in init_args.clone().into_iter() {
        salt.append(&arg.to_xdr(e));
        salt.append(&symbol_short!("0x00").to_xdr(e));
    }
    e.crypto().sha256(&salt)
}

pub fn merge_salt(e: &Env, left: BytesN<32>, right: BytesN<32>) -> BytesN<32> {
    let mut salt = Bytes::new(e);
    salt.append(&left.to_xdr(e));
    salt.append(&right.to_xdr(e));
    e.crypto().sha256(&salt)
}

pub fn deploy_standard_pool(
    e: &Env,
    tokens: Vec<Address>,
    fee_fraction: u32,
) -> (BytesN<32>, Address) {
    let salt = pool_salt(e, tokens.clone());
    let liquidity_pool_wasm_hash = get_constant_product_pool_hash(&e);
    let subpool_salt = get_standard_pool_salt(e, &fee_fraction);

    let pool_contract_id = e
        .deployer()
        .with_current_contract(merge_salt(e, salt.clone(), subpool_salt.clone()))
        .deploy(liquidity_pool_wasm_hash);
    init_standard_pool(e, &tokens, &pool_contract_id, fee_fraction);

    add_pool(
        e,
        &salt,
        subpool_salt.clone(),
        LiquidityPoolType::ConstantProduct,
        pool_contract_id.clone(),
    );

    Events::new(&e).add_pool(
        tokens,
        pool_contract_id.clone(),
        symbol_short!("constant"),
        subpool_salt.clone(),
        Vec::<Val>::from_array(e, [fee_fraction.into_val(e)]),
    );

    (subpool_salt, pool_contract_id)
}

pub fn deploy_stableswap_pool(
    e: &Env,
    tokens: Vec<Address>,
    a: u128,
    fee_fraction: u32,
    admin_fee: u32,
) -> (BytesN<32>, Address) {
    let salt = pool_salt(&e, tokens.clone());

    let liquidity_pool_wasm_hash = get_stableswap_pool_hash(&e, tokens.len());
    let subpool_salt = get_stableswap_pool_salt(&e);

    let pool_contract_id = e
        .deployer()
        .with_current_contract(merge_salt(&e, salt.clone(), subpool_salt.clone()))
        .deploy(liquidity_pool_wasm_hash);
    init_stableswap_pool(e, &tokens, &pool_contract_id, a, fee_fraction, admin_fee);

    // if STABLESWAP_MAX_POOLS
    add_pool(
        &e,
        &salt,
        subpool_salt.clone(),
        LiquidityPoolType::StableSwap,
        pool_contract_id.clone(),
    );

    Events::new(&e).add_pool(
        tokens,
        pool_contract_id.clone(),
        symbol_short!("stable"),
        subpool_salt.clone(),
        Vec::<Val>::from_array(
            e,
            [
                fee_fraction.into_val(e),
                a.into_val(e),
                admin_fee.into_val(e),
            ],
        ),
    );

    (subpool_salt, pool_contract_id)
}

fn init_standard_pool(
    e: &Env,
    tokens: &Vec<Address>,
    pool_contract_id: &Address,
    fee_fraction: u32,
) {
    let token_wasm_hash = get_token_hash(&e);
    let rewards = get_rewards_manager(e);
    let reward_token = rewards.storage().get_reward_token();
    let access_control = AccessControl::new(&e);
    let admin = access_control.get_admin().unwrap();
    let liq_pool_client = StandardLiquidityPoolClient::new(&e, pool_contract_id);
    liq_pool_client.initialize(&admin, &token_wasm_hash, tokens, &fee_fraction);
    liq_pool_client.initialize_rewards_config(&reward_token, &e.current_contract_address());
}

fn init_stableswap_pool(
    e: &Env,
    tokens: &Vec<Address>,
    pool_contract_id: &Address,
    a: u128,
    fee_fraction: u32,
    admin_fee_fraction: u32,
) {
    let token_wasm_hash = get_token_hash(&e);
    let rewards = get_rewards_manager(e);
    let reward_token = rewards.storage().get_reward_token();
    let access_control = AccessControl::new(&e);
    let admin = access_control.get_admin().unwrap();
    e.invoke_contract::<bool>(
        pool_contract_id,
        &Symbol::new(&e, "initialize"),
        Vec::from_array(
            &e,
            [
                admin.into_val(e),
                token_wasm_hash.into_val(e),
                tokens.clone().into_val(e),
                a.into_val(e),
                (fee_fraction as u128).into_val(e),
                (admin_fee_fraction as u128).into_val(e),
            ],
        ),
    );
    e.invoke_contract::<bool>(
        pool_contract_id,
        &Symbol::new(&e, "initialize_rewards_config"),
        Vec::from_array(
            &e,
            [
                reward_token.into_val(e),
                e.current_contract_address().into_val(e),
            ],
        ),
    );
}

pub fn pool_salt(e: &Env, tokens: Vec<Address>) -> BytesN<32> {
    for i in 0..tokens.len() - 1 {
        if tokens.get_unchecked(i) >= tokens.get_unchecked(i + 1) {
            panic!("tokens must be sorted by ascending");
        }
    }

    let mut salt = Bytes::new(e);
    for token in tokens.into_iter() {
        salt.append(&token.to_xdr(e));
    }
    e.crypto().sha256(&salt)
}

pub fn get_total_liquidity(e: &Env, tokens: Vec<Address>) -> (Map<BytesN<32>, u128>, u128) {
    let salt = pool_salt(&e, tokens);
    let mut result = 0;
    let mut pools = Map::new(&e);
    for (hash, pool_id) in get_pools_plain(&e, &salt) {
        let liquidity =
            e.invoke_contract::<u128>(&pool_id, &Symbol::new(&e, "get_liquidity"), Vec::new(&e));
        result += liquidity;
        pools.set(hash, liquidity);
    }
    (pools, result)
}
