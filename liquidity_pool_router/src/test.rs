#![cfg(test)]
extern crate std;

use crate::LiquidityPoolRouterClient;
use soroban_sdk::testutils::{Ledger, LedgerInfo};
use soroban_sdk::{symbol_short, testutils::Address as _, Address, BytesN, Env, IntoVal, Val, Vec};

pub(crate) mod test_token {
    use soroban_sdk::contractimport;
    contractimport!(
        file = "../token/target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
}

pub(crate) mod liq_pool {
    use soroban_sdk::contractimport;
    contractimport!(
        file = "../liquidity_pool/target/wasm32-unknown-unknown/release/soroban_liquidity_pool_contract.wasm"
    );
}

fn create_token_contract<'a>(e: &Env, admin: &Address) -> test_token::Client<'a> {
    test_token::Client::new(e, &e.register_stellar_asset_contract(admin.clone()))
}

fn create_liqpool_router_contract<'a>(e: &Env) -> LiquidityPoolRouterClient<'a> {
    let router = LiquidityPoolRouterClient::new(
        e,
        &e.register_contract(None, crate::LiquidityPoolRouter {}),
    );
    router
}

fn install_token_wasm(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../token/target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

fn install_liq_pool_hash(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../liquidity_pool/target/wasm32-unknown-unknown/release/soroban_liquidity_pool_contract.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

fn install_stableswap_liq_pool_hash(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../liquidity_pool_stableswap/target/wasm32-unknown-unknown/release/soroban_liquidity_pool_stableswap_contract.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

fn jump(e: &Env, time: u64) {
    e.ledger().set(LedgerInfo {
        timestamp: e.ledger().timestamp().saturating_add(time),
        protocol_version: 20,
        sequence_number: e.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_expiration: 999999,
        min_persistent_entry_expiration: 999999,
        max_entry_expiration: u32::MAX,
    });
}

#[test]
fn test_constant_product_pool() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }

    let reward_admin = Address::random(&e);
    let admin = Address::random(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::random(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);

    let (pool_hash, pool_address) =
        router.init_standard_pool(&token1.address, &token2.address, &30);

    let pools = router.get_pools(&token1.address, &token2.address);

    assert!(pools.contains_key(pool_hash.clone()));
    assert_eq!(pools.get(pool_hash.clone()).unwrap(), pool_address);

    let token_share = test_token::Client::new(
        &e,
        &router.share_id(&token1.address, &token2.address, &pool_hash),
    );

    token1.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1), 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1), 1000);
    token1.approve(&user1, &pool_address, &1000, &99999);
    token2.approve(&user1, &pool_address, &1000, &99999);

    assert_eq!(token_share.balance(&user1), 0);

    let desired_amounts = Vec::from_array(&e, [100, 100]);
    router.deposit(
        &user1,
        &token1.address,
        &token2.address,
        &pool_hash,
        &desired_amounts,
    );

    assert_eq!(token_share.balance(&user1), 100);
    assert_eq!(token_share.balance(&pool_address), 0);
    assert_eq!(token1.balance(&user1), 900);
    assert_eq!(token1.balance(&pool_address), 100);
    assert_eq!(token2.balance(&user1), 900);
    assert_eq!(token2.balance(&pool_address), 100);

    assert_eq!(
        router.get_reserves(&token1.address, &token2.address, &pool_hash),
        Vec::from_array(&e, [100, 100])
    );

    assert_eq!(
        router.estimate_swap(&token1.address, &token2.address, &pool_hash, &97),
        49
    );
    assert_eq!(
        router.swap(
            &user1,
            &token1.address,
            &token2.address,
            &pool_hash,
            &97_u128,
            &49_u128,
        ),
        49
    );

    assert_eq!(token1.balance(&user1), 803);
    assert_eq!(token1.balance(&pool_address), 197);
    assert_eq!(token2.balance(&user1), 949);
    assert_eq!(token2.balance(&pool_address), 51);
    assert_eq!(
        router.get_reserves(&token1.address, &token2.address, &pool_hash),
        Vec::from_array(&e, [197, 51])
    );

    token_share.approve(&user1, &pool_address, &100, &99999);

    router.withdraw(
        &user1,
        &token1.address,
        &token2.address,
        &pool_hash,
        &100_u128,
        &Vec::from_array(&e, [197_u128, 51_u128]),
    );

    assert_eq!(token1.balance(&user1), 1000);
    assert_eq!(token2.balance(&user1), 1000);
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token1.balance(&pool_address), 0);
    assert_eq!(token2.balance(&pool_address), 0);
    assert_eq!(token_share.balance(&pool_address), 0);
}

#[test]
fn test_stableswap_pool() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }

    let reward_admin = Address::random(&e);
    let admin = Address::random(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::random(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);

    let (pool_hash, pool_address) =
        router.init_stableswap_pool(&token1.address, &token2.address, &10, &30, &0);

    let pools = router.get_pools(&token1.address, &token2.address);

    assert!(pools.contains_key(pool_hash.clone()));
    assert_eq!(pools.get(pool_hash.clone()).unwrap(), pool_address);

    let token_share = test_token::Client::new(
        &e,
        &router.share_id(&token1.address, &token2.address, &pool_hash),
    );

    token1.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1), 1000_0000000);

    token2.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1), 1000_0000000);
    token1.approve(&user1, &pool_address, &1000_0000000, &99999);
    token2.approve(&user1, &pool_address, &1000_0000000, &99999);

    assert_eq!(token_share.balance(&user1), 0);

    let desired_amounts = Vec::from_array(&e, [100_0000000, 100_0000000]);
    router.deposit(
        &user1,
        &token1.address,
        &token2.address,
        &pool_hash,
        &desired_amounts,
    );

    assert_eq!(token_share.balance(&user1), 200_0000000);
    assert_eq!(token_share.balance(&pool_address), 0);
    assert_eq!(token1.balance(&user1), 900_0000000);
    assert_eq!(token1.balance(&pool_address), 100_0000000);
    assert_eq!(token2.balance(&user1), 900_0000000);
    assert_eq!(token2.balance(&pool_address), 100_0000000);

    assert_eq!(
        router.get_reserves(&token1.address, &token2.address, &pool_hash),
        Vec::from_array(&e, [100_0000000, 100_0000000])
    );

    assert_eq!(
        router.estimate_swap(&token1.address, &token2.address, &pool_hash, &97_0000000),
        80_4573706
    );
    assert_eq!(
        router.swap(
            &user1,
            &token1.address,
            &token2.address,
            &pool_hash,
            &97_0000000_u128,
            &80_4573706_u128,
        ),
        80_4573706
    );

    assert_eq!(token1.balance(&user1), 803_0000000);
    assert_eq!(token1.balance(&pool_address), 197_0000000);
    assert_eq!(token2.balance(&user1), 980_4573706);
    assert_eq!(token2.balance(&pool_address), 19_5426294);
    assert_eq!(
        router.get_reserves(&token1.address, &token2.address, &pool_hash),
        Vec::from_array(&e, [197_0000000, 19_5426294])
    );

    token_share.approve(&user1, &pool_address, &200_0000000, &99999);

    router.withdraw(
        &user1,
        &token1.address,
        &token2.address,
        &pool_hash,
        &200_0000000_u128,
        &Vec::from_array(&e, [197_0000000_u128, 19_5426294_u128]),
    );

    assert_eq!(token1.balance(&user1), 1000_0000000);
    assert_eq!(token2.balance(&user1), 1000_0000000);
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token1.balance(&pool_address), 0);
    assert_eq!(token2.balance(&pool_address), 0);
    assert_eq!(token_share.balance(&pool_address), 0);
}

#[test]
fn test_init_pool_twice() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }

    let reward_admin = Address::random(&e);
    let admin = Address::random(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);

    let (pool_hash1, pool_address1) = router.init_pool(&token1.address, &token2.address);
    let (pool_hash2, pool_address2) =
        router.init_standard_pool(&token1.address, &token2.address, &30);
    assert_eq!(pool_hash1, pool_hash2);
    assert_eq!(pool_address1, pool_address2);

    let pools = router.get_pools(&token1.address, &token2.address);
    assert_eq!(pools.len(), 1);

    router.init_standard_pool(&token1.address, &token2.address, &10);
    assert_eq!(router.get_pools(&token1.address, &token2.address).len(), 2);

    router.init_standard_pool(&token1.address, &token2.address, &100);
    assert_eq!(router.get_pools(&token1.address, &token2.address).len(), 3);

    router.init_standard_pool(&token1.address, &token2.address, &10);
    assert_eq!(router.get_pools(&token1.address, &token2.address).len(), 3);
}

#[test]
fn test_custom_pool() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }

    let reward_admin = Address::random(&e);
    let admin = Address::random(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::random(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);

    let (_original_pool_hash, pool_address) =
        router.init_standard_pool(&token1.address, &token2.address, &30);

    let pool_hash = router.add_custom_pool(
        &token1.address,
        &token2.address,
        &pool_address,
        &symbol_short!("custom"),
        &Vec::<Val>::from_array(&e, [42_i128.into_val(&e)]),
    );

    let pools = router.get_pools(&token1.address, &token2.address);

    assert_eq!(pools.len(), 2);

    let token_share = test_token::Client::new(
        &e,
        &router.share_id(&token1.address, &token2.address, &pool_hash),
    );

    token1.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1), 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1), 1000);
    token1.approve(&user1, &pool_address, &1000, &99999);
    token2.approve(&user1, &pool_address, &1000, &99999);

    assert_eq!(token_share.balance(&user1), 0);

    let desired_amounts = Vec::from_array(&e, [100, 100]);
    router.deposit(
        &user1,
        &token1.address,
        &token2.address,
        &pool_hash,
        &desired_amounts,
    );

    assert_eq!(
        router.swap(
            &user1,
            &token1.address,
            &token2.address,
            &pool_hash,
            &97_u128,
            &49_u128,
        ),
        49
    );
    token_share.approve(&user1, &pool_address, &100, &99999);
    assert_eq!(
        router.withdraw(
            &user1,
            &token1.address,
            &token2.address,
            &pool_hash,
            &100_u128,
            &Vec::from_array(&e, [197_u128, 51_u128]),
        ),
        Vec::from_array(&e, [197_u128, 51_u128]),
    );
}

#[test]
fn test_simple_ongoing_reward() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }

    let reward_admin = Address::random(&e);
    let admin = Address::random(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::random(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);

    let (pool_hash, pool_address) =
        router.init_standard_pool(&token1.address, &token2.address, &30);

    reward_token.mint(&router.address, &1_000_000_0000000);
    let total_reward_1 = 10_5000000_u128 * 60;
    router.set_rewards_config(
        &admin,
        &token1.address,
        &token2.address,
        &pool_hash,
        &e.ledger().timestamp().saturating_add(60),
        &total_reward_1,
    );
    reward_token.approve(&router.address, &pool_address, &1_000_000_0000000, &99999);

    token1.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1), 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1), 1000);
    token1.approve(&user1, &pool_address, &1000, &99999);
    token2.approve(&user1, &pool_address, &1000, &99999);

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    router.deposit(
        &user1,
        &token1.address,
        &token2.address,
        &pool_hash,
        &Vec::from_array(&e, [100, 100]),
    );

    assert_eq!(reward_token.balance(&user1), 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);
    assert_eq!(
        router.claim(&user1, &token1.address, &token2.address, &pool_hash),
        total_reward_1 / 2
    );
    assert_eq!(reward_token.balance(&user1) as u128, total_reward_1 / 2);
}
