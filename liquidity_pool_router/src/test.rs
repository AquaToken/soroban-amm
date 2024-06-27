#![cfg(test)]
extern crate std;

use crate::constants::{CONSTANT_PRODUCT_FEE_AVAILABLE, STABLESWAP_MAX_POOLS};
use crate::LiquidityPoolRouterClient;
use soroban_sdk::testutils::{
    AuthorizedFunction, AuthorizedInvocation, Events, Ledger, LedgerInfo, MockAuth, MockAuthInvoke,
};
use soroban_sdk::{
    symbol_short, testutils::Address as _, vec, Address, BytesN, Env, FromVal, IntoVal, Map,
    Symbol, Val, Vec, U256,
};
use utils::test_utils::{assert_approx_eq_abs, assert_approx_eq_abs_u256};

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

fn install_stableswap_liq_pool_hash(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_stableswap_contract.wasm"
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

mod liquidity_calculator {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_liquidity_calculator_contract.wasm"
    );
}

fn create_liquidity_calculator_contract<'a>(e: &Env) -> liquidity_calculator::Client<'a> {
    liquidity_calculator::Client::new(
        e,
        &e.register_contract_wasm(None, liquidity_calculator::WASM),
    )
}

fn jump(e: &Env, time: u64) {
    e.ledger().set(LedgerInfo {
        timestamp: e.ledger().timestamp().saturating_add(time),
        protocol_version: e.ledger().protocol_version(),
        sequence_number: e.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 999999,
        min_persistent_entry_ttl: 999999,
        max_entry_ttl: u32::MAX,
    });
}
#[test]
#[should_panic(expected = "Error(Contract, #103)")]
fn test_init_admin_twice() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.init_admin(&admin);
}

#[test]
fn test_total_liquidity() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_wasm_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_wasm_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    reward_token.mint(&user1, &3_0000000);

    token1.mint(&user1, &1000000);
    token2.mint(&user1, &1000000);

    for pool_fee in CONSTANT_PRODUCT_FEE_AVAILABLE {
        let (pool_hash, _pool_address) = router.init_standard_pool(&user1, &tokens, &pool_fee);
        router.deposit(
            &user1,
            &tokens,
            &pool_hash,
            &Vec::from_array(&e, [30000, 30000]),
            &0,
        );
    }

    e.budget().reset_unlimited();
    e.budget().reset_default();
    assert_eq!(
        router.get_total_liquidity(&tokens),
        U256::from_u32(&e, 3276)
    );
    e.budget().print();
    e.budget().reset_unlimited();

    for pool_fee in [10, 30, 100] {
        let (pool_hash, _pool_address) =
            router.init_stableswap_pool(&user1, &tokens, &85, &pool_fee, &0);
        router.deposit(
            &user1,
            &tokens,
            &pool_hash,
            &Vec::from_array(&e, [30000, 30000]),
            &0,
        );
    }

    e.budget().reset_unlimited();
    e.budget().reset_default();
    assert_eq!(
        router.get_total_liquidity(&tokens),
        U256::from_u32(&e, 28494)
    );
    e.budget().print();
    assert!(
        e.budget().cpu_instruction_cost() < 100_000_000,
        "budget exceed"
    );
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
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

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

    assert_eq!(token_share.balance(&user1), 0);

    let desired_amounts = Vec::from_array(&e, [100, 100]);
    router.deposit(&user1, &tokens, &pool_hash, &desired_amounts, &0);
    assert_eq!(router.get_total_liquidity(&tokens), U256::from_u32(&e, 2));

    assert_eq!(token_share.balance(&user1), 100);
    assert_eq!(router.get_total_shares(&tokens, &pool_hash), 100);
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
        48
    );
    assert_eq!(
        router.estimate_swap_routed(&tokens, &token1.address, &token2.address, &97),
        (pool_hash.clone(), pool_address.clone(), 48),
    );
    assert_eq!(
        router.swap(
            &user1,
            &tokens,
            &token1.address,
            &token2.address,
            &pool_hash,
            &97_u128,
            &48_u128,
        ),
        48
    );

    assert_eq!(token1.balance(&user1), 803);
    assert_eq!(token1.balance(&pool_address), 197);
    assert_eq!(token2.balance(&user1), 948);
    assert_eq!(token2.balance(&pool_address), 52);
    assert_eq!(
        router.get_reserves(&tokens, &pool_hash),
        Vec::from_array(&e, [197, 52])
    );

    router.withdraw(
        &user1,
        &tokens,
        &pool_hash,
        &100_u128,
        &Vec::from_array(&e, [197_u128, 52_u128]),
    );

    assert_eq!(token1.balance(&user1), 1000);
    assert_eq!(token2.balance(&user1), 1000);
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token1.balance(&pool_address), 0);
    assert_eq!(token2.balance(&pool_address), 0);
    assert_eq!(token_share.balance(&pool_address), 0);
}

#[test]
fn test_add_pool_after_removal() {
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
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);

    let (pool_hash, pool_address) = router.init_standard_pool(&user1, &tokens, &30);
    router.remove_pool(&admin, &tokens, &pool_hash);
    let (pool_hash_new, pool_address_new) = router.init_standard_pool(&user1, &tokens, &30);
    assert_eq!(pool_hash, pool_hash_new);
    assert_ne!(pool_address, pool_address_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #306)")]
fn test_stableswap_pools_amount_over_max() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);
    let payment_for_creation_address = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
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
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);
    let payment_for_creation_address = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
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
    for i in 0..STABLESWAP_MAX_POOLS {
        router.init_stableswap_pool(&admin, &tokens, &10, &30, &0);
        assert_eq!(
            reward_token.balance(&payment_for_creation_address),
            1000_0000000i128 * ((i + 1) as i128)
        );
    }
}

#[test]
#[should_panic(expected = "zero balance is not sufficient to spend")]
fn test_stableswap_pool_no_balance() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);
    let payment_for_creation_address = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
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
    if token2.address < token1.address {
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
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(
        &reward_token.address,
        &1000_0000000,
        &payment_for_creation_address,
    );
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    reward_token.mint(&user1, &10000000_0000000);
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

    assert_eq!(token_share.balance(&user1), 0);

    let desired_amounts = Vec::from_array(&e, [100_0000000, 100_0000000]);
    router.deposit(&user1, &tokens, &pool_hash, &desired_amounts, &0);
    assert_eq!(
        router.get_total_liquidity(&tokens),
        U256::from_u32(&e, 177169768)
    );

    assert_eq!(token_share.balance(&user1), 200_0000000);
    assert_eq!(router.get_total_shares(&tokens, &pool_hash), 200_0000000);
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
        80_4573705
    );
    assert_eq!(
        router.swap(
            &user1,
            &tokens,
            &token1.address,
            &token2.address,
            &pool_hash,
            &97_0000000_u128,
            &80_4573705_u128,
        ),
        80_4573705
    );

    assert_eq!(token1.balance(&user1), 803_0000000);
    assert_eq!(token1.balance(&pool_address), 197_0000000);
    assert_eq!(token2.balance(&user1), 980_4573705);
    assert_eq!(token2.balance(&pool_address), 19_5426295);
    assert_eq!(
        router.get_reserves(&tokens, &pool_hash),
        Vec::from_array(&e, [197_0000000, 19_5426295])
    );

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
        if token2.address < token1.address {
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
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(
        &reward_token.address,
        &1000_0000000,
        &payment_for_creation_address,
    );
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    reward_token.mint(&user1, &10000000_0000000);
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

    assert_eq!(token_share.balance(&user1), 0);

    let desired_amounts = Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000]);
    router.deposit(&user1, &tokens, &pool_hash, &desired_amounts, &0);
    assert_eq!(
        router.get_total_liquidity(&tokens),
        U256::from_u32(&e, 531509304)
    );

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
        80_4573705
    );
    assert_eq!(
        router.estimate_swap_routed(&tokens, &token1.address, &token2.address, &97_0000000,),
        (pool_hash.clone(), pool_address.clone(), 80_4573705),
    );
    assert_eq!(
        router.swap(
            &user1,
            &tokens,
            &token1.address,
            &token2.address,
            &pool_hash,
            &97_0000000_u128,
            &80_4573705_u128,
        ),
        80_4573705
    );
    assert_eq!(
        router.estimate_swap_routed(&tokens, &token2.address, &token3.address, &20_0000000,),
        (pool_hash.clone(), pool_address.clone(), 28_0695119),
    );
    assert_eq!(
        router.swap(
            &user1,
            &tokens,
            &token2.address,
            &token3.address,
            &pool_hash,
            &20_0000000_u128,
            &28_0695119_u128,
        ),
        28_0695119
    );

    assert_eq!(token1.balance(&user1), 803_0000000);
    assert_eq!(token1.balance(&pool_address), 197_0000000);
    assert_eq!(token2.balance(&user1), 960_4573705);
    assert_eq!(token2.balance(&pool_address), 39_5426295);
    assert_eq!(token3.balance(&user1), 928_0695119);
    assert_eq!(token3.balance(&pool_address), 71_9304881);
    assert_eq!(
        router.get_reserves(&tokens, &pool_hash),
        Vec::from_array(&e, [197_0000000, 39_5426295, 71_9304881])
    );

    router.withdraw(
        &user1,
        &tokens,
        &pool_hash,
        &300_0000000_u128,
        &Vec::from_array(&e, [197_0000000_u128, 39_5426295, 71_9304881]),
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
    if token2.address < token1.address {
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
fn test_simple_ongoing_reward() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1000_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    reward_token.mint(&user1, &1000_0000000);

    let (standard_pool_hash, standard_pool_address) =
        router.init_standard_pool(&user1, &tokens, &30);
    let (stable_pool_hash, stable_pool_address) =
        router.init_stableswap_pool(&user1, &tokens, &10, &30, &0);

    reward_token.mint(&standard_pool_address, &1_000_000_0000000);
    reward_token.mint(&stable_pool_address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;

    token1.mint(&user1, &2000);
    assert_eq!(token1.balance(&user1), 2000);

    token2.mint(&user1, &2000);
    assert_eq!(token2.balance(&user1), 2000);

    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &standard_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_claimed_reward(&tokens, &standard_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &standard_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &stable_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_claimed_reward(&tokens, &stable_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &stable_pool_hash),
        0
    );

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    router.deposit(
        &user1,
        &tokens,
        &standard_pool_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    let standard_liquidity = router.get_total_liquidity(&tokens);
    assert_eq!(standard_liquidity, U256::from_u32(&e, 36));
    router.deposit(
        &user1,
        &tokens,
        &stable_pool_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    let stable_liquidity = router.get_total_liquidity(&tokens).sub(&standard_liquidity);
    assert_eq!(
        standard_liquidity.add(&stable_liquidity),
        U256::from_u32(&e, 212)
    );

    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &standard_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_claimed_reward(&tokens, &standard_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &standard_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &stable_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_claimed_reward(&tokens, &stable_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &stable_pool_hash),
        0
    );

    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &admin,
        &reward_1_tps,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    e.budget().reset_default();
    router.fill_liquidity(&tokens);
    e.budget().print();
    e.budget().reset_default();
    let standard_pool_tps = router.config_pool_rewards(&tokens, &standard_pool_hash);
    e.budget().print();
    e.budget().reset_unlimited();
    let stable_pool_tps = router.config_pool_rewards(&tokens, &stable_pool_hash);

    assert_approx_eq_abs_u256(
        U256::from_u128(&e, total_reward_1)
            .mul(&standard_liquidity)
            .div(&(standard_liquidity.add(&stable_liquidity))),
        U256::from_u128(&e, standard_pool_tps * 60),
        U256::from_u32(&e, 100),
    );
    assert_approx_eq_abs_u256(
        U256::from_u128(&e, total_reward_1)
            .mul(&stable_liquidity)
            .div(&(standard_liquidity.add(&stable_liquidity))),
        U256::from_u128(&e, stable_pool_tps * 60),
        U256::from_u32(&e, 100),
    );

    assert_eq!(reward_token.balance(&user1), 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);

    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 30
    );
    assert_eq!(
        router.get_total_claimed_reward(&tokens, &standard_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 30
    );
    assert_eq!(
        router.get_total_claimed_reward(&tokens, &stable_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 60
    );

    assert_eq!(
        router.claim(&user1, &tokens, &standard_pool_hash),
        standard_pool_tps * 30
    );
    assert_eq!(
        router.claim(&user1, &tokens, &stable_pool_hash),
        stable_pool_tps * 30
    );

    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 30
    );
    assert_eq!(
        router.get_total_claimed_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 30
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 30
    );
    assert_eq!(
        router.get_total_claimed_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 30
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 60
    );

    assert_approx_eq_abs(
        reward_token.balance(&user1) as u128,
        total_reward_1 / 2,
        100,
    );
    jump(&e, 60);
    router.claim(&user1, &tokens, &standard_pool_hash);
    router.claim(&user1, &tokens, &stable_pool_hash);
    assert_approx_eq_abs(reward_token.balance(&user1) as u128, total_reward_1, 100);

    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.get_total_claimed_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 60
    );
    assert_eq!(
        router.get_total_claimed_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 60
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 60
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #309)")]
fn test_liqidity_not_filled() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1000_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    let (standard_pool_hash, _standard_pool_address) =
        router.init_standard_pool(&user1, &tokens, &30);

    token1.mint(&user1, &2000);
    token2.mint(&user1, &2000);

    router.deposit(
        &user1,
        &tokens,
        &standard_pool_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &admin,
        &1,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    router.config_pool_rewards(&tokens, &standard_pool_hash);
}

#[test]
#[should_panic(expected = "Error(Contract, #310)")]
fn test_fill_liqidity_reentrancy() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1000_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    let (standard_pool_hash, _standard_pool_address) =
        router.init_standard_pool(&user1, &tokens, &30);
    token1.mint(&user1, &2000);
    token2.mint(&user1, &2000);

    router.deposit(
        &user1,
        &tokens,
        &standard_pool_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &admin,
        &1,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    router.fill_liquidity(&tokens);
    router.fill_liquidity(&tokens);
}

#[test]
#[should_panic(expected = "Error(Contract, #314)")]
fn test_config_pool_rewards_reentrancy() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1000_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    let (standard_pool_hash, _standard_pool_address) =
        router.init_standard_pool(&user1, &tokens, &30);
    token1.mint(&user1, &2000);
    token2.mint(&user1, &2000);

    router.deposit(
        &user1,
        &tokens,
        &standard_pool_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &admin,
        &1,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    router.fill_liquidity(&tokens);
    router.config_pool_rewards(&tokens, &standard_pool_hash);
    router.config_pool_rewards(&tokens, &standard_pool_hash);
}

#[test]
fn test_config_pool_rewards_after_new_global_config() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1000_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    let (standard_pool_hash, _standard_pool_address) =
        router.init_standard_pool(&user1, &tokens, &30);
    token1.mint(&user1, &2000);
    token2.mint(&user1, &2000);

    router.deposit(
        &user1,
        &tokens,
        &standard_pool_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &admin,
        &1,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    router.fill_liquidity(&tokens);
    assert_eq!(router.config_pool_rewards(&tokens, &standard_pool_hash), 1);

    jump(&e, 300);
    router.config_global_rewards(
        &admin,
        &1,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    router.fill_liquidity(&tokens);
    assert_eq!(router.config_pool_rewards(&tokens, &standard_pool_hash), 1);
}

#[test]
fn test_config_pool_after_liquidity_fill() {
    // if pool is created after liquidity filled for tokens, it may be configured, but should receive no rewards
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1000_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    token1.mint(&user1, &2000);
    token2.mint(&user1, &2000);

    let (standard_pool_1_hash, _standard_pool_1_address) =
        router.init_standard_pool(&user1, &tokens, &30);
    router.deposit(
        &user1,
        &tokens,
        &standard_pool_1_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );

    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &admin,
        &1_0000000,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    router.fill_liquidity(&tokens);
    assert_eq!(
        router.config_pool_rewards(&tokens, &standard_pool_1_hash),
        1_0000000
    );

    let (standard_pool_2_hash, _standard_pool_2_address) =
        router.init_standard_pool(&user1, &tokens, &10);
    router.deposit(
        &user1,
        &tokens,
        &standard_pool_2_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    assert_eq!(
        router.config_pool_rewards(&tokens, &standard_pool_2_hash),
        0
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #313)")]
fn test_fill_liquidity_no_config() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1000_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    let (standard_pool_hash, _standard_pool_address) =
        router.init_standard_pool(&user1, &tokens, &30);
    token1.mint(&user1, &2000);
    token2.mint(&user1, &2000);

    router.deposit(
        &user1,
        &tokens,
        &standard_pool_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    router.fill_liquidity(&tokens);
}

#[test]
#[should_panic(expected = "Error(Contract, #102)")]
fn test_config_rewards_not_admin() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin);
    let mut token2 = create_token_contract(&e, &admin);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_token = create_token_contract(&e, &admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1000_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    reward_token.mint(&user1, &1000_0000000);
    router.init_standard_pool(&user1, &tokens, &30);
    router.init_stableswap_pool(&user1, &tokens, &10, &30, &0);

    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &user1,
        &1,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #315)")]
fn test_config_rewards_duplicated_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin);
    let mut token2 = create_token_contract(&e, &admin);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_token = create_token_contract(&e, &admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1000_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    reward_token.mint(&user1, &1000_0000000);
    router.init_standard_pool(&user1, &tokens, &30);
    router.init_stableswap_pool(&user1, &tokens, &10, &30, &0);

    let rewards = Vec::from_array(
        &e,
        [(
            Vec::from_array(&e, [token1.address.clone(), token1.address.clone()]),
            1_0000000,
        )],
    );
    router.config_global_rewards(
        &admin,
        &1,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2002)")]
fn test_config_rewards_tokens_not_sorted() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin);
    let mut token2 = create_token_contract(&e, &admin);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_token = create_token_contract(&e, &admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1000_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    reward_token.mint(&user1, &1000_0000000);
    router.init_standard_pool(&user1, &tokens, &30);
    router.init_stableswap_pool(&user1, &tokens, &10, &30, &0);

    let rewards = Vec::from_array(
        &e,
        [(
            Vec::from_array(&e, [token2.address, token1.address]),
            1_0000000,
        )],
    );
    router.config_global_rewards(
        &admin,
        &1,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
}

#[test]
fn test_config_rewards_no_pools_for_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin);
    let mut token2 = create_token_contract(&e, &admin);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_token = create_token_contract(&e, &admin);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let router = create_liqpool_router_contract(&e);
    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(&reward_token.address, &1000_0000000, &router.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &admin,
        &1,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    assert_eq!(
        router.get_tokens_for_reward(),
        Map::from_array(
            &e,
            [(tokens.clone(), (1_0000000, false, U256::from_u32(&e, 0)))],
        ),
    );
    router.fill_liquidity(&tokens);
    assert_eq!(
        router.get_tokens_for_reward(),
        Map::from_array(
            &e,
            [(tokens.clone(), (1_0000000, true, U256::from_u32(&e, 0)))],
        ),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #302)")]
fn test_unexpected_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
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
    if token2.address < token1.address {
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
    let token_hash = install_token_wasm(&e);
    let contract_id = e.register_contract(None, crate::LiquidityPoolRouter {});
    let plane = create_plane_contract(&e);

    let liquidity_calculator = create_liquidity_calculator_contract(&e);
    liquidity_calculator.init_admin(&admin);
    liquidity_calculator.set_pools_plane(&admin, &plane.address);

    let router = LiquidityPoolRouterClient::new(&e, &contract_id.clone());
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.configure_init_pool_payment(
        &reward_token.address,
        &1000_0000000,
        &payment_for_creation_address,
    );
    router.set_pools_plane(&admin, &plane.address);
    router.set_liquidity_calculator(&admin, &liquidity_calculator.address);
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    reward_token.mint(&user1, &10000000_0000000);
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

    reward_token.mint(&router.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &admin,
        &reward_1_tps,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    router.fill_liquidity(&tokens);
    router.config_pool_rewards(&tokens, &pool_hash);

    token1.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1), 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1), 1000);

    // 10 seconds passed since config, user depositing
    jump(&e, 10);

    let desired_amounts = Vec::from_array(&e, [100, 100]);

    let (amounts, share_amount) = router.deposit(&user1, &tokens, &pool_hash, &desired_amounts, &0);
    assert_eq!(router.get_total_liquidity(&tokens), U256::from_u32(&e, 2));

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
        &48_u128,
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
    if token2.address < token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let reward_admin = Address::generate(&e);
    let admin = Address::generate(&e);

    let reward_token = create_token_contract(&e, &reward_admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);
    router.configure_init_pool_payment(&reward_token.address, &1_0000000, &router.address);

    reward_token.mint(&user1, &3_0000000);
    token1.mint(&user1, &100000_0000000);
    token2.mint(&user1, &100000_0000000);

    let (standard1_pool_hash, _standard1_pool_address) =
        router.init_standard_pool(&user1, &tokens, &10);
    router.deposit(
        &user1,
        &tokens,
        &standard1_pool_hash,
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
        &0,
    );

    let (standard2_pool_hash, _standard2_pool_address) =
        router.init_standard_pool(&user1, &tokens, &30);
    router.deposit(
        &user1,
        &tokens,
        &standard2_pool_hash,
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
        &0,
    );

    let (standard3_pool_hash, _standard3_pool_address) =
        router.init_standard_pool(&user1, &tokens, &100);
    router.deposit(
        &user1,
        &tokens,
        &standard3_pool_hash,
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
        &0,
    );

    let (stable1_pool_hash, stable1_pool_address) =
        router.init_stableswap_pool(&user1, &tokens, &85, &6, &0);
    router.deposit(
        &user1,
        &tokens,
        &stable1_pool_hash,
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
        &0,
    );

    let (stable2_pool_hash, _stable2_pool_address) =
        router.init_stableswap_pool(&user1, &tokens, &85, &6, &0);
    router.deposit(
        &user1,
        &tokens,
        &stable2_pool_hash,
        &Vec::from_array(&e, [100_0000000_u128, 100_0000000_u128]),
        &0,
    );

    let (stable3_pool_hash, _stable3_pool_address) =
        router.init_stableswap_pool(&user1, &tokens, &85, &6, &0);
    router.deposit(
        &user1,
        &tokens,
        &stable3_pool_hash,
        &Vec::from_array(&e, [100_0000000_u128, 100_0000000_u128]),
        &0,
    );

    e.budget().reset_default();
    let (best_pool, best_pool_address, best_result) =
        router.estimate_swap_routed(&tokens, &token1.address, &token2.address, &9_0000000);
    e.budget().print();
    assert_eq!(best_pool, stable1_pool_hash);
    assert_eq!(best_pool_address, stable1_pool_address);
    assert_eq!(best_result, 8_9936585);
}

#[test]
#[should_panic(expected = "Error(Contract, #302)")]
fn test_stableswap_validation_fee_out_of_bounds() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let mut token1 = create_token_contract(&e, &admin);
    let mut token2 = create_token_contract(&e, &admin);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }

    let reward_token = create_token_contract(&e, &admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);
    router.configure_init_pool_payment(&reward_token.address, &1_0000000, &router.address);

    reward_token.mint(&user1, &1_0000000);

    router.init_stableswap_pool(
        &user1,
        &Vec::from_array(&e, [token1.address, token2.address]),
        &85,
        &101,
        &0,
    );
}

#[test]
fn test_tokens_storage() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
    ];
    tokens.sort();

    let reward_token = create_token_contract(&e, &admin);

    let user1 = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let router = create_liqpool_router_contract(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&reward_token.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);
    router.configure_init_pool_payment(&reward_token.address, &1_0000000, &router.address);

    reward_token.mint(&user1, &10_0000000);

    let pairs = [
        Vec::from_array(&e, [tokens[0].clone(), tokens[1].clone()]),
        Vec::from_array(&e, [tokens[1].clone(), tokens[2].clone()]),
        Vec::from_array(&e, [tokens[0].clone(), tokens[2].clone()]),
        Vec::from_array(
            &e,
            [tokens[0].clone(), tokens[1].clone(), tokens[2].clone()],
        ),
    ];
    for pair in pairs.clone() {
        router.init_stableswap_pool(&user1, &pair, &100, &0, &0);
        router.init_stableswap_pool(&user1, &pair, &100, &0, &0);
        if pair.len() == 2 {
            router.init_standard_pool(&user1, &pair, &30);
        }
    }
    let counter = router.get_tokens_sets_count();
    assert_eq!(counter, 4);
    let mut pools_full_list = Vec::new(&e);
    for i in 0..counter {
        assert_eq!(router.get_tokens(&i), pairs[i as usize]);
        let pools = (
            pairs[i as usize].clone(),
            router.get_pools(&pairs[i as usize]),
        );
        assert_eq!(
            router.get_pools_for_tokens_range(&i, &(i + 1)),
            Vec::from_array(&e, [pools.clone()])
        );
        pools_full_list.push_back(pools);
    }
    assert_eq!(
        router.get_pools_for_tokens_range(&0, &counter),
        pools_full_list,
    );
}

#[test]
fn test_chained_swap() {
    let e = Env::default();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address
    ];
    tokens.sort();
    let token1 = test_token::Client::new(&e, &tokens[0]);
    let token2 = test_token::Client::new(&e, &tokens[1]);
    let token3 = test_token::Client::new(&e, &tokens[2]);

    let tokens1 = Vec::from_array(&e, [tokens[0].clone(), tokens[1].clone()]);
    let tokens2 = Vec::from_array(&e, [tokens[1].clone(), tokens[2].clone()]);

    let swapper = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router
        .mock_all_auths()
        .set_pools_plane(&admin, &plane.address);
    let router = create_liqpool_router_contract(&e);
    router.mock_all_auths().init_admin(&admin);
    router.mock_all_auths().set_pool_hash(&pool_hash);
    router
        .mock_all_auths()
        .set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.mock_all_auths().set_token_hash(&token_hash);
    router.mock_all_auths().set_reward_token(&token1.address);
    router
        .mock_all_auths()
        .set_pools_plane(&admin, &plane.address);
    router
        .mock_all_auths()
        .set_swap_router(&admin, &swap_router.address);

    let (pool_index1, _pool_address1) = router
        .mock_all_auths()
        .init_standard_pool(&swapper, &tokens1, &30);
    let (pool_index2, _pool_address2) = router
        .mock_all_auths()
        .init_standard_pool(&swapper, &tokens2, &30);
    token1.mock_all_auths().mint(&admin, &10000);
    token2.mock_all_auths().mint(&admin, &20000);
    token3.mock_all_auths().mint(&admin, &10000);
    router.mock_all_auths().deposit(
        &admin,
        &tokens1,
        &pool_index1,
        &Vec::from_array(&e, [10000, 10000]),
        &0,
    );
    router.mock_all_auths().deposit(
        &admin,
        &tokens2,
        &pool_index2,
        &Vec::from_array(&e, [10000, 10000]),
        &0,
    );

    // swapping token 1 to 3 through combination of 2 pools as we don't have pool (1, 3)
    token1.mock_all_auths().mint(&swapper, &1000);

    let swap_root_args = vec![
        &e,
        swapper.clone().to_val(),
        vec![
            &e,
            (tokens1.clone(), pool_index1.clone(), tokens[1].clone()),
            (tokens2.clone(), pool_index2.clone(), tokens[2].clone()),
        ]
        .into_val(&e),
        tokens[0].clone().to_val(),
        100_u128.into_val(&e),
        95_u128.into_val(&e),
    ];

    assert_eq!(token1.balance(&swapper), 1000);
    assert_eq!(token2.balance(&swapper), 0);
    assert_eq!(token3.balance(&swapper), 0);
    assert_eq!(token1.balance(&router.address), 0);
    assert_eq!(token2.balance(&router.address), 0);
    assert_eq!(token3.balance(&router.address), 0);
    assert_eq!(
        router
            .mock_auths(&[MockAuth {
                address: &swapper,
                invoke: &MockAuthInvoke {
                    contract: &router.address,
                    fn_name: "swap_chained",
                    args: swap_root_args.into_val(&e),
                    sub_invokes: &[MockAuthInvoke {
                        contract: &token1.address.clone(),
                        fn_name: "transfer",
                        args: Vec::from_array(
                            &e,
                            [
                                swapper.to_val(),
                                router.address.to_val(),
                                100_i128.into_val(&e),
                            ]
                        )
                        .into_val(&e),
                        sub_invokes: &[],
                    }],
                },
            }])
            .swap_chained(
                &swapper,
                &vec![
                    &e,
                    (tokens1.clone(), pool_index1.clone(), tokens[1].clone()),
                    (tokens2.clone(), pool_index2.clone(), tokens[2].clone()),
                ],
                &tokens[0],
                &100,
                &95,
            ),
        96
    );
    assert_eq!(
        e.auths(),
        std::vec![(
            swapper.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    router.address.clone(),
                    Symbol::new(&e, "swap_chained"),
                    swap_root_args.into_val(&e)
                )),
                sub_invocations: std::vec![AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        token1.address.clone(),
                        Symbol::new(&e, "transfer"),
                        Vec::from_array(
                            &e,
                            [
                                swapper.to_val(),
                                router.address.to_val(),
                                100_i128.into_val(&e),
                            ]
                        ),
                    )),
                    sub_invocations: std::vec![],
                },],
            }
        ),]
    );
    assert_eq!(token1.balance(&swapper), 900);
    assert_eq!(token2.balance(&swapper), 0);
    assert_eq!(token3.balance(&swapper), 96);
    assert_eq!(token1.balance(&router.address), 0);
    assert_eq!(token2.balance(&router.address), 0);
    assert_eq!(token3.balance(&router.address), 0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2006)")]
fn test_chained_swap_min_not_met() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
    ];
    tokens.sort();
    let token1 = test_token::Client::new(&e, &tokens[0]);
    let token2 = test_token::Client::new(&e, &tokens[1]);
    let token3 = test_token::Client::new(&e, &tokens[2]);

    let tokens1 = Vec::from_array(&e, [tokens[0].clone(), tokens[1].clone()]);
    let tokens2 = Vec::from_array(&e, [tokens[1].clone(), tokens[2].clone()]);

    let swapper = Address::generate(&e);

    let pool_hash = install_liq_pool_hash(&e);
    let token_hash = install_token_wasm(&e);
    let plane = create_plane_contract(&e);
    let swap_router = create_swap_router_contract(&e);
    swap_router.init_admin(&admin);
    swap_router.set_pools_plane(&admin, &plane.address);
    let router = create_liqpool_router_contract(&e);
    router.init_admin(&admin);
    router.set_pool_hash(&pool_hash);
    router.set_stableswap_pool_hash(&install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&token_hash);
    router.set_reward_token(&token1.address);
    router.set_pools_plane(&admin, &plane.address);
    router.set_swap_router(&admin, &swap_router.address);

    let (pool_index1, _pool_address1) = router.init_standard_pool(&swapper, &tokens1, &30);
    let (pool_index2, _pool_address2) = router.init_standard_pool(&swapper, &tokens2, &30);
    token1.mint(&admin, &10000);
    token2.mint(&admin, &20000);
    token3.mint(&admin, &10000);
    router.deposit(
        &admin,
        &tokens1,
        &pool_index1,
        &Vec::from_array(&e, [10000, 10000]),
        &0,
    );
    router.deposit(
        &admin,
        &tokens2,
        &pool_index2,
        &Vec::from_array(&e, [10000, 10000]),
        &0,
    );

    token1.mint(&swapper, &20000);

    router.swap_chained(
        &swapper,
        &vec![
            &e,
            (tokens1.clone(), pool_index1.clone(), tokens[1].clone()),
            (tokens2.clone(), pool_index2.clone(), tokens[2].clone()),
        ],
        &tokens[0],
        &20,
        &95,
    );
}
