use crate::admin::get_admin;
use crate::pool_contract::{StableSwapLiquidityPoolClient, StandardLiquidityPoolClient};
use crate::storage;
use crate::storage::{
    get_constant_product_pool_hash, get_reward_token, get_stableswap_pool_hash, get_token_hash,
};
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{symbol_short, Address, Bytes, BytesN, Env, Symbol, Vec};

pub fn get_standard_pool_hash(e: &Env, fee_fraction: u32) -> BytesN<32> {
    // fixme: fee_fraction is mutable for pool. hash collision is possible to happen
    let mut salt = Bytes::new(e);
    salt.append(&symbol_short!("standard").to_xdr(&e));
    salt.append(&symbol_short!("0x00").to_xdr(&e));
    salt.append(&fee_fraction.to_xdr(&e));
    salt.append(&symbol_short!("0x00").to_xdr(&e));
    e.crypto().sha256(&salt)
}

// pub fn get_stableswap_pool_hash(e: &Env, a: u32, fee_fraction: u32) -> BytesN<32> {
//     let mut salt = Bytes::new(e);
//     salt.append(&symbol_short!("standard").to_xdr(&e));
//     salt.append(&symbol_short!("0x00").to_xdr(&e));
//     salt.append(&fee_fraction.to_xdr(&e));
//     salt.append(&symbol_short!("0x00").to_xdr(&e));
//     e.crypto().sha256(&salt)
// }

pub fn deploy_standard_pool(
    e: &Env,
    token_a: &Address,
    token_b: &Address,
    fee_fraction: u32,
) -> (BytesN<32>, Address) {
    let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
    let liquidity_pool_wasm_hash = get_constant_product_pool_hash(&e);

    let pool_contract_id = e
        .deployer()
        .with_current_contract(salt.clone())
        .deploy(liquidity_pool_wasm_hash);
    init_standard_pool(e, token_a, token_b, &pool_contract_id, fee_fraction);

    let pool_hash = get_standard_pool_hash(&e, fee_fraction);

    storage::add_pool(&e, &salt, pool_hash.clone(), pool_contract_id.clone());

    e.events().publish(
        (
            Symbol::new(&e, "init_pool"),
            token_a.clone(),
            token_b.clone(),
        ),
        (
            &pool_contract_id,
            symbol_short!("constant"),
            fee_fraction,
            pool_hash.clone(),
        ),
    );

    (pool_hash, pool_contract_id)
}

// pub fn deploy_stableswap_pool(
//     e: &Env,
//     token_a: &Address,
//     token_b: &Address,
//     a: u128,
//     fee_fraction: u32,
//     admin_fee: u32,
// ) {
//     let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
//     let liquidity_pool_wasm_hash = get_stableswap_pool_hash(&e);
//
//     let pool_contract_id = e
//         .deployer()
//         .with_current_contract(salt.clone())
//         .deploy(liquidity_pool_wasm_hash);
//     init_stableswap_pool(e, token_a, token_b, a, &pool_contract_id, fee_fraction, admin_fee);
//
//     storage::put_pool(&e, &salt, &pool_contract_id);
//
//     // TODO: NOT FOR PRODUCTION
//     //  this is unsafe as we can store limited amount of records
//     // add_pool_to_list(&e, &get_pool_id(&e, &salt));
//
//     e.events().publish(
//         (
//             Symbol::new(&e, "init_pool"),
//             token_a.clone(),
//             token_b.clone(),
//         ),
//         (&pool_contract_id, &symbol_short!("stable"), fee_fraction),
//     );
// }

fn init_standard_pool(
    e: &Env,
    token_a: &Address,
    token_b: &Address,
    pool_contract_id: &Address,
    fee_fraction: u32,
) {
    let token_wasm_hash = get_token_hash(&e);
    let reward_token = get_reward_token(&e);
    let admin = get_admin(&e);
    let liq_pool_client = StandardLiquidityPoolClient::new(&e, pool_contract_id);
    liq_pool_client.initialize(
        &admin,
        &token_wasm_hash,
        &Vec::from_array(&e, [token_a.clone(), token_b.clone()]),
        &fee_fraction,
    );
    liq_pool_client.initialize_rewards_config(&reward_token, &e.current_contract_address());
}

// fn init_stableswap_pool(
//     e: &Env,
//     token_a: &Address,
//     token_b: &Address,
//     a: u128,
//     pool_contract_id: &Address,
//     fee_fraction: u32,
//     admin_fee_fraction: u32,
// ) {
//     let token_wasm_hash = get_token_hash(&e);
//     let reward_token = get_reward_token(&e);
//     let admin = get_admin(&e);
//     let liq_pool_client = StableSwapLiquidityPoolClient::new(&e, pool_contract_id);
//     liq_pool_client.initialize(
//         &admin,
//         &token_wasm_hash,
//         &Vec::from_array(&e, [token_a.clone(), token_b.clone()]),
//         &a,
//         &(fee_fraction as u128),
//         &(admin_fee_fraction as u128),
//     );
//     liq_pool_client.initialize_rewards_config(&admin, &reward_token, &e.current_contract_address());
// }
