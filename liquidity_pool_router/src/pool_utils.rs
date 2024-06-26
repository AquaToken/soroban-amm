use crate::events::{Events, LiquidityPoolRouterEvents};
use crate::liquidity_calculator::LiquidityCalculatorClient;
use crate::pool_contract::StandardLiquidityPoolClient;
use crate::rewards::get_rewards_manager;
use crate::storage::{
    add_pool, add_tokens_set, get_constant_product_pool_hash, get_pool_next_counter,
    get_pool_plane, get_pools_plain, get_stableswap_pool_hash, get_token_hash, LiquidityPoolType,
};
use access_control::access::{AccessControl, AccessControlTrait};
use liquidity_pool_validation_errors::LiquidityPoolValidationError;
use rewards::storage::RewardsStorageTrait;
use soroban_sdk::{
    panic_with_error, symbol_short, xdr::ToXdr, Address, Bytes, BytesN, Env, IntoVal, Map, Symbol,
    Val, Vec, U256,
};

pub fn get_standard_pool_salt(e: &Env, fee_fraction: &u32) -> BytesN<32> {
    let mut salt = Bytes::new(e);
    salt.append(&symbol_short!("standard").to_xdr(e));
    salt.append(&symbol_short!("0x00").to_xdr(e));
    salt.append(&fee_fraction.to_xdr(e));
    salt.append(&symbol_short!("0x00").to_xdr(e));
    e.crypto().sha256(&salt).to_bytes()
}

pub fn get_stableswap_pool_salt(e: &Env) -> BytesN<32> {
    let mut salt = Bytes::new(e);
    salt.append(&symbol_short!("stable").to_xdr(e));
    salt.append(&symbol_short!("0x00").to_xdr(e));
    // no constant pool parameters, though hash should be different, so we add pool counter
    salt.append(&get_pool_next_counter(e).to_xdr(e));
    salt.append(&symbol_short!("0x00").to_xdr(e));
    e.crypto().sha256(&salt).to_bytes()
}

pub fn get_pool_counter_salt(e: &Env) -> BytesN<32> {
    let mut salt = Bytes::new(e);
    salt.append(&symbol_short!("0x00").to_xdr(e));
    salt.append(&get_pool_next_counter(e).to_xdr(e));
    salt.append(&symbol_short!("0x00").to_xdr(e));
    e.crypto().sha256(&salt).to_bytes()
}

pub fn merge_salt(e: &Env, left: BytesN<32>, right: BytesN<32>) -> BytesN<32> {
    let mut salt = Bytes::new(e);
    salt.append(&left.to_xdr(e));
    salt.append(&right.to_xdr(e));
    e.crypto().sha256(&salt).to_bytes()
}

pub fn deploy_standard_pool(
    e: &Env,
    tokens: Vec<Address>,
    fee_fraction: u32,
) -> (BytesN<32>, Address) {
    let tokens_salt = get_tokens_salt(e, tokens.clone());
    let liquidity_pool_wasm_hash = get_constant_product_pool_hash(e);
    let subpool_salt = get_standard_pool_salt(e, &fee_fraction);

    let pool_contract_id = e
        .deployer()
        .with_current_contract(merge_salt(
            e,
            merge_salt(e, tokens_salt.clone(), subpool_salt.clone()),
            get_pool_counter_salt(e),
        ))
        .deploy(liquidity_pool_wasm_hash);
    init_standard_pool(e, &tokens, &pool_contract_id, fee_fraction);

    add_tokens_set(e, &tokens);
    add_pool(
        e,
        &tokens_salt,
        subpool_salt.clone(),
        LiquidityPoolType::ConstantProduct,
        pool_contract_id.clone(),
    );

    Events::new(e).add_pool(
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
    let tokens_salt = get_tokens_salt(e, tokens.clone());

    let liquidity_pool_wasm_hash = get_stableswap_pool_hash(e);
    let subpool_salt = get_stableswap_pool_salt(e);

    // pools counter already incorporated into subpool_salt - no need to add it again
    let pool_contract_id = e
        .deployer()
        .with_current_contract(merge_salt(e, tokens_salt.clone(), subpool_salt.clone()))
        .deploy(liquidity_pool_wasm_hash);
    init_stableswap_pool(e, &tokens, &pool_contract_id, a, fee_fraction, admin_fee);

    add_tokens_set(e, &tokens);
    add_pool(
        e,
        &tokens_salt,
        subpool_salt.clone(),
        LiquidityPoolType::StableSwap,
        pool_contract_id.clone(),
    );

    Events::new(e).add_pool(
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
    let token_wasm_hash = get_token_hash(e);
    let rewards = get_rewards_manager(e);
    let reward_token = rewards.storage().get_reward_token();
    let access_control = AccessControl::new(e);
    let admin = access_control.get_admin().unwrap();
    let liq_pool_client = StandardLiquidityPoolClient::new(e, pool_contract_id);
    let plane = get_pool_plane(e);
    liq_pool_client.initialize_all(
        &admin,
        &e.current_contract_address(),
        &token_wasm_hash,
        tokens,
        &fee_fraction,
        &reward_token,
        &plane,
    );
}

fn init_stableswap_pool(
    e: &Env,
    tokens: &Vec<Address>,
    pool_contract_id: &Address,
    a: u128,
    fee_fraction: u32,
    admin_fee_fraction: u32,
) {
    let token_wasm_hash = get_token_hash(e);
    let rewards = get_rewards_manager(e);
    let reward_token = rewards.storage().get_reward_token();
    let access_control = AccessControl::new(e);
    let admin = access_control.get_admin().unwrap();
    let plane = get_pool_plane(e);
    e.invoke_contract::<()>(
        pool_contract_id,
        &Symbol::new(e, "initialize_all"),
        Vec::from_array(
            e,
            [
                admin.into_val(e),
                e.current_contract_address().to_val(),
                token_wasm_hash.into_val(e),
                tokens.clone().into_val(e),
                a.into_val(e),
                fee_fraction.into_val(e),
                admin_fee_fraction.into_val(e),
                reward_token.into_val(e),
                plane.into_val(e),
            ],
        ),
    );
}

pub fn get_tokens_salt(e: &Env, tokens: Vec<Address>) -> BytesN<32> {
    for i in 0..tokens.len() - 1 {
        if tokens.get_unchecked(i) >= tokens.get_unchecked(i + 1) {
            panic_with_error!(e, LiquidityPoolValidationError::TokensNotSorted);
        }
    }

    let mut salt = Bytes::new(e);
    for token in tokens.into_iter() {
        salt.append(&token.to_xdr(e));
    }
    e.crypto().sha256(&salt).to_bytes()
}

pub fn get_total_liquidity(
    e: &Env,
    tokens: Vec<Address>,
    calculator: Address,
) -> (Map<BytesN<32>, U256>, U256) {
    let tokens_salt = get_tokens_salt(e, tokens.clone());
    let pools = get_pools_plain(&e, &tokens_salt);
    let pools_count = pools.len();
    let mut pools_map: Map<BytesN<32>, U256> = Map::new(&e);

    let mut pools_vec: Vec<Address> = Vec::new(&e);
    let mut hashes_vec: Vec<BytesN<32>> = Vec::new(&e);
    for (key, value) in pools {
        pools_vec.push_back(value.clone());
        hashes_vec.push_back(key.clone());
    }

    let pools_liquidity = LiquidityCalculatorClient::new(&e, &calculator).get_liquidity(&pools_vec);
    let mut result = U256::from_u32(&e, 0);
    for i in 0..pools_count {
        let value = pools_liquidity.get(i).unwrap();
        pools_map.set(hashes_vec.get(i).unwrap(), value.clone());
        result = result.add(&value);
    }
    (pools_map, result)
}
