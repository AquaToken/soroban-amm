#![cfg(test)]
extern crate std;

use crate::constants::{CONSTANT_PRODUCT_FEE_AVAILABLE, MAX_POOLS_FOR_PAIR, STABLESWAP_MAX_POOLS};
use crate::LiquidityPoolRouterClient;
use soroban_sdk::testutils::{Events, Ledger, LedgerInfo};
use soroban_sdk::{
    symbol_short, testutils::Address as _, vec, Address, BytesN, Env, FromVal, IntoVal, Symbol,
    Val, Vec,
};

pub(crate) mod test_token {
    use soroban_sdk::contractimport;
    contractimport!(file = "../target/wasm32-unknown-unknown/release/soroban_token_contract.wasm");
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
        file = "../target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

fn install_liq_pool_hash(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_contract.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

fn install_stableswap_two_tokens_liq_pool_hash(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_stableswap_contract_2_tokens.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

fn install_stableswap_three_tokens_liq_pool_hash(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_stableswap_contract_3_tokens.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

mod pool_plane {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_plane_contract.wasm"
    );
}

fn create_plane_contract<'a>(e: &Env) -> pool_plane::Client<'a> {
    pool_plane::Client::new(e, &e.register_contract_wasm(None, pool_plane::WASM))
}

mod swap_router {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_swap_router_contract.wasm"
    );
}

fn create_swap_router_contract<'a>(e: &Env) -> swap_router::Client<'a> {
    swap_router::Client::new(e, &e.register_contract_wasm(None, swap_router::WASM))
}

fn jump(e: &Env, time: u64) {
    e.ledger().set(LedgerInfo {
        timestamp: e.ledger().timestamp().saturating_add(time),
        protocol_version: 20,
        sequence_number: e.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 999999,
        min_persistent_entry_ttl: 999999,
        max_entry_ttl: u32::MAX,
    });
}

#[test]
fn test_constant_product_pool() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash_2 = install_stableswap_two_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_hash_2);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);

    let (pool_hash, pool_address) = router.init_standard_pool(&user1, &tokens, &30);
    assert_eq!(
        router.pool_type(&tokens, &pool_hash),
        Symbol::new(&e, "constant_product")
    );
    let pool_info = router.get_info(&tokens, &pool_hash);
    assert_eq!(
        Symbol::from_val(&e, &pool_info.get(Symbol::new(&e, "pool_type")).unwrap()),
        Symbol::new(&e, "constant_product")
    );

    let pools = router.get_pools(&tokens);

    assert!(pools.contains_key(pool_hash.clone()));
    assert_eq!(pools.get(pool_hash.clone()).unwrap(), pool_address);

    let token_share = test_token::Client::new(&e, &router.share_id(&tokens, &pool_hash));

    token1.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1), 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1), 1000);
    token1.approve(&user1, &pool_address, &1000, &99999);
    token2.approve(&user1, &pool_address, &1000, &99999);

    assert_eq!(token_share.balance(&user1), 0);

    let desired_amounts = Vec::from_array(&e, [100, 100]);
    router.deposit(&user1, &tokens, &pool_hash, &desired_amounts);

    assert_eq!(token_share.balance(&user1), 100);
    assert_eq!(token_share.balance(&pool_address), 0);
    assert_eq!(token1.balance(&user1), 900);
    assert_eq!(token1.balance(&pool_address), 100);
    assert_eq!(token2.balance(&user1), 900);
    assert_eq!(token2.balance(&pool_address), 100);

    assert_eq!(
        router.get_reserves(&tokens, &pool_hash),
        Vec::from_array(&e, [100, 100])
    );

    assert_eq!(
        router.estimate_swap(&tokens, &token1.address, &token2.address, &pool_hash, &97),
        49
    );
    assert_eq!(
        router.estimate_swap_routed(&tokens, &token1.address, &token2.address, &97),
        (pool_hash.clone(), pool_address.clone(), 49),
    );
    assert_eq!(
        router.swap(
            &user1,
            &tokens,
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
        router.get_reserves(&tokens, &pool_hash),
        Vec::from_array(&e, [197, 51])
    );

    token_share.approve(&user1, &pool_address, &100, &99999);

    router.withdraw(
        &user1,
        &tokens,
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
#[should_panic(expected = "stableswap pools amount is over max")]
fn test_stableswap_pools_amount_over_max() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);
    let payment_for_creation_address = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_two_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(
        &reward_token.address,
        &1000_0000000,
        &payment_for_creation_address,
    );
    router.set_pools_plane(&admin, &plane.address);
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    // init constant product pools to make sure we don't affect stableswap counter
    for fee_fraction in CONSTANT_PRODUCT_FEE_AVAILABLE {
        router.init_standard_pool(&admin, &tokens, &fee_fraction);
    }
    reward_token.mint(&admin, &10000000_0000000);
    reward_token.approve(&admin, &router.address, &10000000_0000000, &99999);
    for i in 0..STABLESWAP_MAX_POOLS + 1 {
        router.init_stableswap_pool(&admin, &tokens, &10, &30, &0);
        assert_eq!(
            reward_token.balance(&payment_for_creation_address),
            1000_0000000i128 * ((i + 1) as i128)
        );
    }
}

#[test]
fn test_stableswap_pools_amount_ok() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);
    let payment_for_creation_address = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_two_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(
        &reward_token.address,
        &1000_0000000,
        &payment_for_creation_address,
    );
    router.set_pools_plane(&admin, &plane.address);
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    // init constant product pools to make sure we don't affect stableswap counter
    for fee_fraction in CONSTANT_PRODUCT_FEE_AVAILABLE {
        router.init_standard_pool(&admin, &tokens, &fee_fraction);
    }
    reward_token.mint(&admin, &10000000_0000000);
    reward_token.approve(&admin, &router.address, &10000000_0000000, &99999);
    for i in 0..STABLESWAP_MAX_POOLS {
        router.init_stableswap_pool(&admin, &tokens, &10, &30, &0);
        assert_eq!(
            reward_token.balance(&payment_for_creation_address),
            1000_0000000i128 * ((i + 1) as i128)
        );
    }
}

#[test]
#[should_panic(expected = "not enough allowance to spend")]
fn test_stableswap_pool_no_allowance() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);
    let payment_for_creation_address = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_two_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(
        &reward_token.address,
        &1000_0000000,
        &payment_for_creation_address,
    );

    router.init_stableswap_pool(&admin, &tokens, &10, &30, &0);
    assert_eq!(
        reward_token.balance(&payment_for_creation_address),
        1000_0000000
    );
}

#[test]
fn test_stableswap_pool() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);
    let payment_for_creation_address = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_two_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(
        &reward_token.address,
        &1000_0000000,
        &payment_for_creation_address,
    );
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    reward_token.mint(&user1, &10000000_0000000);
    reward_token.approve(&user1, &router.address, &10000000_0000000, &99999);
    e.budget().reset_default();
    let (pool_hash, pool_address) = router.init_stableswap_pool(&user1, &tokens, &10, &30, &0);
    e.budget().print();
    assert!(e.budget().cpu_instruction_cost() < 100_000_000);
    e.budget().reset_unlimited();
    assert_eq!(
        router.pool_type(&tokens, &pool_hash),
        Symbol::new(&e, "stable")
    );
    assert_eq!(
        reward_token.balance(&payment_for_creation_address),
        1000_0000000
    );

    let pools = router.get_pools(&tokens);

    assert!(pools.contains_key(pool_hash.clone()));
    assert_eq!(pools.get(pool_hash.clone()).unwrap(), pool_address);

    let token_share = test_token::Client::new(&e, &router.share_id(&tokens, &pool_hash));

    token1.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1), 1000_0000000);

    token2.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1), 1000_0000000);
    token1.approve(&user1, &pool_address, &1000_0000000, &99999);
    token2.approve(&user1, &pool_address, &1000_0000000, &99999);

    assert_eq!(token_share.balance(&user1), 0);

    let desired_amounts = Vec::from_array(&e, [100_0000000, 100_0000000]);
    router.deposit(&user1, &tokens, &pool_hash, &desired_amounts);

    assert_eq!(token_share.balance(&user1), 200_0000000);
    assert_eq!(token_share.balance(&pool_address), 0);
    assert_eq!(token1.balance(&user1), 900_0000000);
    assert_eq!(token1.balance(&pool_address), 100_0000000);
    assert_eq!(token2.balance(&user1), 900_0000000);
    assert_eq!(token2.balance(&pool_address), 100_0000000);

    assert_eq!(
        router.get_reserves(&tokens, &pool_hash),
        Vec::from_array(&e, [100_0000000, 100_0000000])
    );

    assert_eq!(
        router.estimate_swap(
            &tokens,
            &token1.address,
            &token2.address,
            &pool_hash,
            &97_0000000,
        ),
        80_4573706
    );
    assert_eq!(
        router.swap(
            &user1,
            &tokens,
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
        router.get_reserves(&tokens, &pool_hash),
        Vec::from_array(&e, [197_0000000, 19_5426294])
    );

    token_share.approve(&user1, &pool_address, &200_0000000, &99999);

    router.withdraw(
        &user1,
        &tokens,
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
fn test_stableswap_3_pool() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);
    let mut admin3 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let mut token3 = create_token_contract(&e, &admin3);

    for _i in 0..2 {
        if &token2.address < &token1.address {
            std::mem::swap(&mut token1, &mut token2);
            std::mem::swap(&mut admin1, &mut admin2);
        }
        if &token3.address < &token2.address {
            std::mem::swap(&mut token2, &mut token3);
            std::mem::swap(&mut admin2, &mut admin3);
        }
    }

    let tokens = Vec::from_array(
        &e,
        [
            token1.address.clone(),
            token2.address.clone(),
            token3.address.clone(),
        ],
    );

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);
    let payment_for_creation_address = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_2_hash = install_stableswap_two_tokens_liq_pool_hash(&e);
    let stableswap_pool_3_hash = install_stableswap_three_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_2_hash);
    router.set_stableswap_pool_hash(&3, &stableswap_pool_3_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(
        &reward_token.address,
        &1000_0000000,
        &payment_for_creation_address,
    );
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    reward_token.mint(&user1, &10000000_0000000);
    reward_token.approve(&user1, &router.address, &10000000_0000000, &99999);
    let (pool_hash, pool_address) = router.init_stableswap_pool(&user1, &tokens, &10, &30, &0);
    assert_eq!(
        router.pool_type(&tokens, &pool_hash),
        Symbol::new(&e, "stable")
    );
    assert_eq!(
        reward_token.balance(&payment_for_creation_address),
        1000_0000000
    );

    let pools = router.get_pools(&tokens);

    assert!(pools.contains_key(pool_hash.clone()));
    assert_eq!(pools.get(pool_hash.clone()).unwrap(), pool_address);

    let token_share = test_token::Client::new(&e, &router.share_id(&tokens, &pool_hash));

    token1.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1), 1000_0000000);
    token2.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1), 1000_0000000);
    token3.mint(&user1, &1000_0000000);
    assert_eq!(token3.balance(&user1), 1000_0000000);

    token1.approve(&user1, &pool_address, &1000_0000000, &99999);
    token2.approve(&user1, &pool_address, &1000_0000000, &99999);
    token3.approve(&user1, &pool_address, &1000_0000000, &99999);

    assert_eq!(token_share.balance(&user1), 0);

    let desired_amounts = Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000]);
    router.deposit(&user1, &tokens, &pool_hash, &desired_amounts);

    assert_eq!(token_share.balance(&user1), 300_0000000);
    assert_eq!(token_share.balance(&pool_address), 0);

    assert_eq!(token1.balance(&user1), 900_0000000);
    assert_eq!(token1.balance(&pool_address), 100_0000000);
    assert_eq!(token2.balance(&user1), 900_0000000);
    assert_eq!(token2.balance(&pool_address), 100_0000000);
    assert_eq!(token3.balance(&user1), 900_0000000);
    assert_eq!(token3.balance(&pool_address), 100_0000000);

    assert_eq!(
        router.get_reserves(&tokens, &pool_hash),
        Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000])
    );

    assert_eq!(
        router.estimate_swap(
            &tokens,
            &token1.address,
            &token2.address,
            &pool_hash,
            &97_0000000,
        ),
        80_4573706
    );
    assert_eq!(
        router.estimate_swap_routed(&tokens, &token1.address, &token2.address, &97_0000000,),
        (pool_hash.clone(), pool_address.clone(), 80_4573706),
    );
    assert_eq!(
        router.swap(
            &user1,
            &tokens,
            &token1.address,
            &token2.address,
            &pool_hash,
            &97_0000000_u128,
            &80_4573706_u128,
        ),
        80_4573706
    );
    assert_eq!(
        router.swap(
            &user1,
            &tokens,
            &token2.address,
            &token3.address,
            &pool_hash,
            &20_0000000_u128,
            &28_0695121_u128,
        ),
        28_0695121
    );

    assert_eq!(token1.balance(&user1), 803_0000000);
    assert_eq!(token1.balance(&pool_address), 197_0000000);
    assert_eq!(token2.balance(&user1), 960_4573706);
    assert_eq!(token2.balance(&pool_address), 39_5426294);
    assert_eq!(token3.balance(&user1), 928_0695121);
    assert_eq!(token3.balance(&pool_address), 71_9304879);
    assert_eq!(
        router.get_reserves(&tokens, &pool_hash),
        Vec::from_array(&e, [197_0000000, 39_5426294, 71_9304879])
    );

    token_share.approve(&user1, &pool_address, &300_0000000, &99999);

    router.withdraw(
        &user1,
        &tokens,
        &pool_hash,
        &300_0000000_u128,
        &Vec::from_array(&e, [197_0000000_u128, 39_5426294, 71_9304879]),
    );

    assert_eq!(token1.balance(&user1), 1000_0000000);
    assert_eq!(token2.balance(&user1), 1000_0000000);
    assert_eq!(token3.balance(&user1), 1000_0000000);
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

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.set_pools_plane(&admin, &plane.address);

    let (pool_hash1, pool_address1) = router.init_pool(&tokens);
    let (pool_hash2, pool_address2) = router.init_standard_pool(&admin, &tokens, &30);
    assert_eq!(pool_hash1, pool_hash2);
    assert_eq!(pool_address1, pool_address2);

    let pools = router.get_pools(&tokens);
    assert_eq!(pools.len(), 1);

    router.init_standard_pool(&admin, &tokens, &10);
    assert_eq!(router.get_pools(&tokens).len(), 2);

    router.init_standard_pool(&admin, &tokens, &100);
    assert_eq!(router.get_pools(&tokens).len(), 3);

    router.init_standard_pool(&admin, &tokens, &10);
    assert_eq!(router.get_pools(&tokens).len(), 3);
}

#[test]
fn test_custom_pool() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_two_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);

    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.set_pools_plane(&admin, &plane.address);

    let router_1 = create_liqpool_router_contract(&e);
    router_1.init_admin(&admin);
    router_1.set_pool_hash(&pool_hash);
    router_1.set_token_hash(&token_hash);
    router_1.set_reward_token(&reward_token.address);
    router_1.set_pools_plane(&admin, &plane.address);

    let (_original_pool_hash, custom_pool_address) =
        router_1.init_standard_pool(&user1, &tokens, &30);

    let pool_hash = router.add_custom_pool(
        &admin,
        &tokens,
        &custom_pool_address,
        &symbol_short!("custom"),
        &Vec::<Val>::from_array(&e, [42_i128.into_val(&e)]),
    );

    let pools = router.get_pools(&tokens);

    assert_eq!(pools.len(), 1);

    let token_share = test_token::Client::new(&e, &router.share_id(&tokens, &pool_hash));

    token1.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1), 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1), 1000);
    token1.approve(&user1, &custom_pool_address, &1000, &99999);
    token2.approve(&user1, &custom_pool_address, &1000, &99999);

    assert_eq!(token_share.balance(&user1), 0);

    let desired_amounts = Vec::from_array(&e, [100, 100]);
    router.deposit(&user1, &tokens, &pool_hash, &desired_amounts);

    assert_eq!(
        router.swap(
            &user1,
            &tokens,
            &token1.address,
            &token2.address,
            &pool_hash,
            &97_u128,
            &49_u128,
        ),
        49
    );
    token_share.approve(&user1, &custom_pool_address, &100, &99999);
    assert_eq!(
        router.withdraw(
            &user1,
            &tokens,
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

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_two_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.set_pools_plane(&admin, &plane.address);

    let (pool_hash, pool_address) = router.init_standard_pool(&user1, &tokens, &30);

    reward_token.mint(&pool_address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    router.set_rewards_config(
        &admin,
        &tokens,
        &pool_hash,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

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
        &tokens,
        &pool_hash,
        &Vec::from_array(&e, [100, 100]),
    );

    assert_eq!(reward_token.balance(&user1), 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);
    assert_eq!(
        router.claim(&user1, &tokens, &pool_hash),
        total_reward_1 / 2
    );
    assert_eq!(reward_token.balance(&user1) as u128, total_reward_1 / 2);
}

// need rewrite test for Vec<tokens>
#[test]
#[should_panic(expected = "pools amount is over max")]
fn test_max_pools_for_pair() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_two_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.set_pools_plane(&admin, &plane.address);

    let (_original_pool_hash, pool_address) = router.init_standard_pool(&user1, &tokens, &30);

    for n in 1..MAX_POOLS_FOR_PAIR {
        // 1 standard + 9 in cycle = 10 - inclusive
        let args = Vec::<Val>::from_array(&e, [(42_i128 + i128::from(n)).into_val(&e)]);

        router.add_custom_pool(
            &admin,
            &tokens,
            &pool_address,
            &symbol_short!("custom"),
            &args,
        );
    }
    // if add one more - error
    router.add_custom_pool(
        &admin,
        &tokens,
        &pool_address,
        &symbol_short!("custom"),
        &Vec::<Val>::from_array(&e, [42_i128.into_val(&e)]),
    );
}

#[test]
#[should_panic(expected = "non-standard fee")]
fn test_unexpected_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_two_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);

    let fee = CONSTANT_PRODUCT_FEE_AVAILABLE[1] + 1;
    router.init_standard_pool(&user1, &tokens, &fee);
}

#[test]
fn test_event_correct() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);
    let payment_for_creation_address = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash = install_stableswap_two_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let contract_id = e.register_contract(None, crate::LiquidityPoolRouter {});
    let plane = create_plane_contract(&e);

    let router = LiquidityPoolRouterClient::new(&e, &contract_id.clone());
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_hash);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(
        &reward_token.address,
        &1000_0000000,
        &payment_for_creation_address,
    );
    router.set_pools_plane(&admin, &plane.address);
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    let router_1 = create_liqpool_router_contract(&e);
    router_1.init_admin(&admin);
    router_1.set_pool_hash(&pool_hash);
    router_1.set_token_hash(&token_hash);
    router_1.set_reward_token(&reward_token.address);
    router_1.set_pools_plane(&admin, &plane.address);
    let (_pool_hash, custom_pool_address) = router_1.init_standard_pool(&user1, &tokens, &30);
    reward_token.mint(&user1, &10000000_0000000);
    reward_token.approve(&user1, &router.address, &10000000_0000000, &99999);
    let fee = CONSTANT_PRODUCT_FEE_AVAILABLE[1];
    let admin_fee = 0;

    let (pool_hash, pool_address) =
        router.init_stableswap_pool(&user1, &tokens, &10, &fee, &admin_fee);
    assert_eq!(
        reward_token.balance(&payment_for_creation_address),
        1000_0000000
    );

    let init_stableswap_pool_event = e.events().all().last().unwrap();

    assert_eq!(
        vec![&e, init_stableswap_pool_event],
        vec![
            &e,
            (
                contract_id.clone(),
                (Symbol::new(&e, "add_pool"), tokens.clone()).into_val(&e),
                (
                    pool_address.clone(),
                    symbol_short!("stable"),
                    pool_hash.clone(),
                    Vec::<Val>::from_array(
                        &e,
                        [
                            fee.into_val(&e),
                            10_u128.into_val(&e),
                            admin_fee.into_val(&e)
                        ],
                    ),
                )
                    .into_val(&e)
            ),
        ]
    );

    let (pool_hash, pool_address) = router.init_standard_pool(&user1, &tokens, &fee);

    let init_pool_event = e.events().all().last().unwrap();

    assert_eq!(
        vec![&e, init_pool_event],
        vec![
            &e,
            (
                contract_id.clone(),
                (Symbol::new(&e, "add_pool"), tokens.clone(),).into_val(&e),
                (
                    pool_address.clone(),
                    symbol_short!("constant"),
                    pool_hash.clone(),
                    Vec::<Val>::from_array(&e, [fee.into_val(&e)]),
                )
                    .into_val(&e)
            ),
        ]
    );

    let subpool_salt = router.add_custom_pool(
        &admin,
        &tokens,
        &custom_pool_address,
        &symbol_short!("custom"),
        &Vec::<Val>::from_array(&e, [42_i128.into_val(&e)]),
    );

    let add_custom_pool_event = e.events().all().last().unwrap();

    assert_eq!(
        vec![&e, add_custom_pool_event],
        vec![
            &e,
            (
                contract_id.clone(),
                (Symbol::new(&e, "add_pool"), tokens.clone()).into_val(&e),
                (
                    custom_pool_address.clone(),
                    symbol_short!("custom"),
                    subpool_salt.clone(),
                    Vec::<Val>::from_array(&e, [42_i128.into_val(&e)]),
                )
                    .into_val(&e)
            ),
        ]
    );

    reward_token.mint(&router.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    router.set_rewards_config(
        &admin,
        &tokens,
        &pool_hash,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
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

    let desired_amounts = Vec::from_array(&e, [100, 100]);

    let (amounts, share_amount) = router.deposit(&user1, &tokens, &pool_hash, &desired_amounts);

    let pool_id = router.get_pool(&tokens, &pool_hash);

    let deposit_event = e.events().all().last().unwrap();

    assert_eq!(
        vec![&e, deposit_event],
        vec![
            &e,
            (
                contract_id.clone(),
                (Symbol::new(&e, "deposit"), tokens.clone(), user1.clone()).into_val(&e),
                (pool_id.clone(), amounts, share_amount).into_val(&e)
            ),
        ]
    );

    let out_amt = router.swap(
        &user1,
        &tokens,
        &token1.address,
        &token2.address,
        &pool_hash,
        &97_u128,
        &49_u128,
    );
    let swap_event = e.events().all().last().unwrap();

    assert_eq!(
        vec![&e, swap_event],
        vec![
            &e,
            (
                contract_id.clone(),
                (Symbol::new(&e, "swap"), tokens.clone(), user1.clone()).into_val(&e),
                (
                    pool_id.clone(),
                    &token1.address,
                    &token2.address,
                    97_u128,
                    out_amt
                )
                    .into_val(&e)
            ),
        ]
    );

    let token_share = test_token::Client::new(&e, &router.share_id(&tokens, &pool_hash));
    token_share.approve(&user1, &pool_address, &100, &99999);

    let amounts = router.withdraw(
        &user1,
        &tokens,
        &pool_hash,
        &100_u128,
        &Vec::from_array(&e, [197_u128, 51_u128]),
    );
    let withdraw_event = e.events().all().last().unwrap();

    assert_eq!(
        vec![&e, withdraw_event],
        vec![
            &e,
            (
                contract_id.clone(),
                (Symbol::new(&e, "withdraw"), tokens.clone(), user1.clone()).into_val(&e),
                (pool_id.clone(), 100_u128, amounts).into_val(&e)
            ),
        ]
    );
}

#[test]
fn test_swap_routed() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let stableswap_pool_hash_2 = install_stableswap_two_tokens_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&2, &stableswap_pool_hash_2);
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);
    router.configure_init_pool_payment(&reward_token.address, &1_0000000, &router.address);

    reward_token.mint(&user1, &3_0000000);
    reward_token.approve(&user1, &router.address, &3_0000000, &99999);
    token1.mint(&user1, &100000_0000000);
    token2.mint(&user1, &100000_0000000);

    let (standard1_pool_hash, standard1_pool_address) =
        router.init_standard_pool(&user1, &tokens, &10);
    token1.approve(&user1, &standard1_pool_address, &1000_0000000, &99999);
    token2.approve(&user1, &standard1_pool_address, &1000_0000000, &99999);
    router.deposit(
        &user1,
        &tokens,
        &standard1_pool_hash,
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
    );

    let (standard2_pool_hash, standard2_pool_address) =
        router.init_standard_pool(&user1, &tokens, &30);
    token1.approve(&user1, &standard2_pool_address, &1000_0000000, &99999);
    token2.approve(&user1, &standard2_pool_address, &1000_0000000, &99999);
    router.deposit(
        &user1,
        &tokens,
        &standard2_pool_hash,
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
    );

    let (standard3_pool_hash, standard3_pool_address) =
        router.init_standard_pool(&user1, &tokens, &100);
    token1.approve(&user1, &standard3_pool_address, &1000_0000000, &99999);
    token2.approve(&user1, &standard3_pool_address, &1000_0000000, &99999);
    router.deposit(
        &user1,
        &tokens,
        &standard3_pool_hash,
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
    );

    let (stable1_pool_hash, stable1_pool_address) =
        router.init_stableswap_pool(&user1, &tokens, &85, &6, &0);
    token1.approve(&user1, &stable1_pool_address, &1000_0000000, &99999);
    token2.approve(&user1, &stable1_pool_address, &1000_0000000, &99999);
    router.deposit(
        &user1,
        &tokens,
        &stable1_pool_hash,
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
    );

    let (stable2_pool_hash, stable2_pool_address) =
        router.init_stableswap_pool(&user1, &tokens, &85, &6, &0);
    token1.approve(&user1, &stable2_pool_address, &100_0000000, &99999);
    token2.approve(&user1, &stable2_pool_address, &100_0000000, &99999);
    router.deposit(
        &user1,
        &tokens,
        &stable2_pool_hash,
        &Vec::from_array(&e, [100_0000000_u128, 100_0000000_u128]),
    );

    let (stable3_pool_hash, stable3_pool_address) =
        router.init_stableswap_pool(&user1, &tokens, &85, &6, &0);
    token1.approve(&user1, &stable3_pool_address, &100_0000000, &99999);
    token2.approve(&user1, &stable3_pool_address, &100_0000000, &99999);
    router.deposit(
        &user1,
        &tokens,
        &stable3_pool_hash,
        &Vec::from_array(&e, [100_0000000_u128, 100_0000000_u128]),
    );

    e.budget().reset_default();
    let (best_pool, best_pool_address, best_result) =
        router.estimate_swap_routed(&tokens, &token1.address, &token2.address, &9_0000000);
    e.budget().print();
    assert_eq!(best_pool, stable1_pool_hash);
    assert_eq!(best_pool_address, stable1_pool_address);
    assert_eq!(best_result, 8_9936586);

    e.budget().reset_default();
    let swap_result = router.swap_routed(
        &user1,
        &tokens,
        &token1.address,
        &token2.address,
        &9_0000000,
        &(best_result - 1),
    );
    e.budget().print();
    assert_eq!(swap_result, best_result);
}
