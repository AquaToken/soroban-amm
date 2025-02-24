#![cfg(test)]
extern crate std;

use crate::constants::{CONSTANT_PRODUCT_FEE_AVAILABLE, STABLESWAP_MAX_POOLS};
use crate::testutils;
use crate::testutils::{create_plane_contract, test_token, Setup};
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::{
    AuthorizedFunction, AuthorizedInvocation, Events, MockAuth, MockAuthInvoke,
};
use soroban_sdk::{
    symbol_short, testutils::Address as _, vec, Address, FromVal, IntoVal, Map, Symbol, Val, Vec,
    U256,
};
use utils::test_utils::{
    assert_approx_eq_abs, assert_approx_eq_abs_u256, install_dummy_wasm, jump,
};

#[test]
#[should_panic(expected = "Error(Contract, #103)")]
fn test_init_admin_twice() {
    let setup = Setup::default();
    setup.router.init_admin(&setup.admin);
}

#[test]
fn test_total_liquidity() {
    let setup = Setup::default();
    let e = setup.env;
    let user1 = Address::generate(&e);
    setup.reward_token.mint(&user1, &10_0000000);
    let [token1, token2, _, _] = setup.tokens;

    token1.mint(&user1, &1000000);
    token2.mint(&user1, &1000000);

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    for pool_fee in CONSTANT_PRODUCT_FEE_AVAILABLE {
        let (pool_hash, _pool_address) =
            setup.router.init_standard_pool(&user1, &tokens, &pool_fee);
        setup.router.deposit(
            &user1,
            &tokens,
            &pool_hash,
            &Vec::from_array(&e, [30000, 30000]),
            &0,
        );
    }

    e.cost_estimate().budget().reset_unlimited();
    e.cost_estimate().budget().reset_default();
    assert_eq!(
        setup.router.get_total_liquidity(&tokens),
        U256::from_u32(&e, 3228)
    );
    e.cost_estimate().budget().print();
    e.cost_estimate().budget().reset_unlimited();

    for pool_fee in [10, 30, 100] {
        let (pool_hash, _pool_address) = setup
            .router
            .init_stableswap_pool(&user1, &tokens, &pool_fee);
        setup.router.deposit(
            &user1,
            &tokens,
            &pool_hash,
            &Vec::from_array(&e, [30000, 30000]),
            &0,
        );
    }

    e.cost_estimate().budget().reset_unlimited();
    e.cost_estimate().budget().reset_default();
    assert_eq!(
        setup.router.get_total_liquidity(&tokens),
        U256::from_u32(&e, 33600)
    );
    e.cost_estimate().budget().print();
    assert!(
        e.cost_estimate().budget().cpu_instruction_cost() < 100_000_000,
        "budget exceed"
    );
}

#[test]
fn test_constant_product_pool() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let [token1, token2, _, _] = setup.tokens;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);
    let user1 = Address::generate(&e);
    setup.reward_token.mint(&user1, &10_0000000);

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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let [token1, token2, _, _] = setup.tokens;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);
    let user1 = Address::generate(&e);
    setup.reward_token.mint(&user1, &10_0000000);

    let (pool_hash, pool_address) = router.init_standard_pool(&user1, &tokens, &30);
    assert!(router.try_remove_pool(&user1, &tokens, &pool_hash).is_err());
    assert!(router
        .try_remove_pool(&setup.rewards_admin, &tokens, &pool_hash)
        .is_err());
    router.remove_pool(&setup.operations_admin, &tokens, &pool_hash);
    let (pool_hash_new, pool_address_new) = router.init_standard_pool(&user1, &tokens, &30);
    assert_eq!(pool_hash, pool_hash_new);
    assert_ne!(pool_address, pool_address_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #306)")]
fn test_stableswap_pools_amount_over_max() {
    let setup = Setup::default();
    let e = setup.env;
    let admin = setup.admin;
    let router = setup.router;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);
    let user1 = Address::generate(&e);

    let payment_for_creation_address = router.get_init_pool_payment_address();
    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &payment_for_creation_address,
    );
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    // init constant product pools to make sure we don't affect stableswap counter
    for fee_fraction in CONSTANT_PRODUCT_FEE_AVAILABLE {
        router.init_standard_pool(&user1, &tokens, &fee_fraction);
    }
    reward_token.mint(&user1, &10000000_0000000);
    for i in 0..STABLESWAP_MAX_POOLS + 1 {
        router.init_stableswap_pool(&user1, &tokens, &30);
        assert_eq!(
            reward_token.balance(&payment_for_creation_address),
            1000_0000000i128 * ((i + 1) as i128)
        );
    }
}

#[test]
fn test_stableswap_pools_amount_ok() {
    let setup = Setup::default();
    let e = setup.env;
    let admin = setup.admin;
    let router = setup.router;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);
    let user1 = Address::generate(&e);

    let payment_for_creation_address = router.get_init_pool_payment_address();
    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &payment_for_creation_address,
    );
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    // init constant product pools to make sure we don't affect stableswap counter
    for fee_fraction in CONSTANT_PRODUCT_FEE_AVAILABLE {
        router.init_standard_pool(&user1, &tokens, &fee_fraction);
    }
    reward_token.mint(&user1, &10000000_0000000);
    for i in 0..STABLESWAP_MAX_POOLS {
        router.init_stableswap_pool(&user1, &tokens, &30);
        assert_eq!(
            reward_token.balance(&payment_for_creation_address),
            1000_0000000i128 * ((i + 1) as i128)
        );
    }
}

#[test]
#[should_panic(expected = "zero balance is not sufficient to spend")]
fn test_stableswap_pool_no_balance() {
    let setup = Setup::default();
    let e = setup.env;
    let admin = setup.admin;
    let router = setup.router;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);
    let user1 = Address::generate(&e);

    let payment_for_creation_address = router.get_init_pool_payment_address();
    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &payment_for_creation_address,
    );

    router.init_stableswap_pool(&user1, &tokens, &30);
    assert_eq!(
        reward_token.balance(&payment_for_creation_address),
        1000_0000000
    );
}

#[test]
fn test_stableswap_pool() {
    let setup = Setup::default();
    let e = setup.env;
    let admin = setup.admin;
    let router = setup.router;
    let [token1, token2, _, _] = setup.tokens;

    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);
    let user1 = Address::generate(&e);

    let payment_for_creation_address = router.get_init_pool_payment_address();
    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &payment_for_creation_address,
    );
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    reward_token.mint(&user1, &10000000_0000000);
    e.cost_estimate().budget().reset_default();
    let (pool_hash, pool_address) = router.init_stableswap_pool(&user1, &tokens, &30);
    e.cost_estimate().budget().print();
    assert!(e.cost_estimate().budget().cpu_instruction_cost() < 100_000_000);
    e.cost_estimate().budget().reset_unlimited();
    assert_eq!(
        router.pool_type(&tokens, &pool_hash),
        Symbol::new(&e, "stable")
    );
    assert_eq!(
        reward_token.balance(&payment_for_creation_address),
        1000_0000000
    );
    assert_eq!(
        testutils::stableswap_pool::Client::new(&e, &pool_address).a(),
        1500,
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
        U256::from_u32(&e, 337515788)
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
        95_8920895
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
        95_8920895
    );

    assert_eq!(token1.balance(&user1), 803_0000000);
    assert_eq!(token1.balance(&pool_address), 197_0000000);
    assert_eq!(token2.balance(&user1), 995_8920895);
    assert_eq!(token2.balance(&pool_address), 4_1079105);
    assert_eq!(
        router.get_reserves(&tokens, &pool_hash),
        Vec::from_array(&e, [197_0000000, 4_1079105])
    );

    router.withdraw(
        &user1,
        &tokens,
        &pool_hash,
        &200_0000000_u128,
        &Vec::from_array(&e, [197_0000000_u128, 4_1079105_u128]),
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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let [token1, token2, token3, _] = setup.tokens;
    let reward_token = setup.reward_token;
    let payment_for_creation_address = router.get_init_pool_payment_address();

    let tokens = Vec::from_array(
        &e,
        [
            token1.address.clone(),
            token2.address.clone(),
            token3.address.clone(),
        ],
    );

    let user1 = Address::generate(&e);

    reward_token.mint(&user1, &10000000_0000000);
    let (pool_hash, pool_address) = router.init_stableswap_pool(&user1, &tokens, &30);
    assert_eq!(
        router.pool_type(&tokens, &pool_hash),
        Symbol::new(&e, "stable")
    );
    assert_eq!(
        reward_token.balance(&payment_for_creation_address),
        1_0000000
    );
    assert_eq!(
        testutils::stableswap_pool::Client::new(&e, &pool_address).a(),
        6750,
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
        U256::from_u32(&e, 1038936246)
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
        router.swap(
            &user1,
            &tokens,
            &token1.address,
            &token2.address,
            &pool_hash,
            &97_0000000_u128,
            &80_4573705_u128,
        ),
        96_4908385
    );
    assert_eq!(
        router.swap(
            &user1,
            &tokens,
            &token2.address,
            &token3.address,
            &pool_hash,
            &20_0000000_u128,
            &20_1144225_u128,
        ),
        20_1144225
    );

    assert_eq!(token1.balance(&user1), 803_0000000);
    assert_eq!(token1.balance(&pool_address), 197_0000000);
    assert_eq!(token2.balance(&user1), 97_64908385);
    assert_eq!(token2.balance(&pool_address), 23_5091615);
    assert_eq!(token3.balance(&user1), 920_1144225);
    assert_eq!(token3.balance(&pool_address), 79_8855775);
    assert_eq!(
        router.get_reserves(&tokens, &pool_hash),
        Vec::from_array(&e, [197_0000000, 23_5091615, 79_8855775])
    );

    router.withdraw(
        &user1,
        &tokens,
        &pool_hash,
        &300_0000000_u128,
        &Vec::from_array(&e, [197_0000000, 23_5091615, 79_8855775]),
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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);
    reward_token.mint(&user1, &10_0000000);

    let (pool_hash1, pool_address1) = router.init_standard_pool(&user1, &tokens, &30);
    let (pool_hash2, pool_address2) = router.init_standard_pool(&user1, &tokens, &30);
    assert_eq!(pool_hash1, pool_hash2);
    assert_eq!(pool_address1, pool_address2);

    let pools = router.get_pools(&tokens);
    assert_eq!(pools.len(), 1);

    router.init_standard_pool(&user1, &tokens, &10);
    assert_eq!(router.get_pools(&tokens).len(), 2);

    router.init_standard_pool(&user1, &tokens, &100);
    assert_eq!(router.get_pools(&tokens).len(), 3);

    router.init_standard_pool(&user1, &tokens, &10);
    assert_eq!(router.get_pools(&tokens).len(), 3);
}

#[should_panic(expected = "Error(WasmVm, MissingValue)")]
#[test]
fn test_init_pool_bad_tokens() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let [token1, _, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(
        &e,
        [
            token1.address.clone(),
            create_plane_contract(&e).address.clone(),
        ],
    );

    let user1 = Address::generate(&e);
    reward_token.mint(&user1, &10_0000000);

    router.init_standard_pool(&user1, &tokens, &30);
}

#[should_panic(expected = "Error(WasmVm, MissingValue)")]
#[test]
fn test_init_standard_pool_bad_tokens() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let [token1, _, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(
        &e,
        [
            token1.address.clone(),
            create_plane_contract(&e).address.clone(),
        ],
    );

    let user1 = Address::generate(&e);
    reward_token.mint(&user1, &10_0000000);

    router.init_standard_pool(&user1, &tokens, &30);
}

#[should_panic(expected = "Error(WasmVm, MissingValue)")]
#[test]
fn test_init_stable_pool_bad_tokens() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let [token1, _, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let user1 = Address::generate(&e);
    reward_token.mint(&user1, &10_0000000);

    let tokens = Vec::from_array(
        &e,
        [
            token1.address.clone(),
            create_plane_contract(&e).address.clone(),
        ],
    );

    router.init_stableswap_pool(&user1, &tokens, &30);
}

#[test]
fn test_simple_ongoing_reward() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);
    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

    reward_token.mint(&user1, &1000_0000000);
    reward_token.mint(&router.address, &2_000_000_0000000);
    reward_token.mint(&admin, &2_000_000_0000000);

    let (standard_pool_hash, standard_pool_address) =
        router.init_standard_pool(&user1, &tokens, &30);
    let (stable_pool_hash, stable_pool_address) = router.init_stableswap_pool(&user1, &tokens, &10);

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
        router.get_total_outstanding_reward(&tokens, &standard_pool_hash),
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
    assert_eq!(
        router.get_total_outstanding_reward(&tokens, &stable_pool_hash),
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
    assert_eq!(standard_liquidity, U256::from_u32(&e, 34));
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
        U256::from_u32(&e, 370)
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
    e.cost_estimate().budget().reset_default();
    router.fill_liquidity(&tokens);
    e.cost_estimate().budget().print();
    e.cost_estimate().budget().reset_default();
    let standard_pool_tps = router.config_pool_rewards(&tokens, &standard_pool_hash);
    e.cost_estimate().budget().print();
    e.cost_estimate().budget().reset_unlimited();
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
        router.get_total_outstanding_reward(&tokens, &standard_pool_hash),
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
        router.get_total_outstanding_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 60
    );

    assert_eq!(reward_token.balance(&standard_pool_address), 0);
    assert_eq!(reward_token.balance(&stable_pool_address), 0);
    assert_eq!(
        router.distribute_outstanding_reward(&admin, &router.address, &tokens, &standard_pool_hash),
        standard_pool_tps * 60
    );
    // distribute second part from admin's balance
    assert!(router
        .try_distribute_outstanding_reward(&admin, &admin, &tokens, &stable_pool_hash)
        .is_err());
    reward_token.approve(
        &admin,
        &router.address,
        &(stable_pool_tps as i128 * 60),
        &9999,
    );
    assert_eq!(
        router.distribute_outstanding_reward(&admin, &admin, &tokens, &stable_pool_hash),
        stable_pool_tps * 60
    );
    assert_eq!(
        router.distribute_outstanding_reward(&admin, &router.address, &tokens, &standard_pool_hash),
        0
    );
    assert_eq!(
        router.distribute_outstanding_reward(&admin, &router.address, &tokens, &stable_pool_hash),
        0
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens, &stable_pool_hash),
        0
    );
    assert_eq!(
        reward_token.balance(&standard_pool_address) as u128,
        standard_pool_tps * 60
    );
    assert_eq!(
        reward_token.balance(&stable_pool_address) as u128,
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
fn test_rewards_distribution() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let user1 = Address::generate(&e);
    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

    let tokens1 = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);
    let tokens2 = Vec::from_array(&e, [token1.address.clone(), reward_token.address.clone()]);

    reward_token.mint(&user1, &2000_0000000);
    reward_token.mint(&router.address, &2_000_000_0000000);

    let (standard_pool_hash1, standard_pool_address1) =
        router.init_standard_pool(&user1, &tokens1, &30);
    let (standard_pool_hash2, standard_pool_address2) =
        router.init_standard_pool(&user1, &tokens2, &30);
    let (stable_pool_hash1, stable_pool_address1) =
        router.init_stableswap_pool(&user1, &tokens1, &10);
    let (stable_pool_hash2, stable_pool_address2) =
        router.init_stableswap_pool(&user1, &tokens2, &10);

    let reward_tps = 10_5000000_u128;

    token1.mint(&user1, &4000);
    token2.mint(&user1, &2000);
    reward_token.mint(&user1, &2000);

    assert_eq!(
        router.get_total_outstanding_reward(&tokens1, &standard_pool_hash1),
        0
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens1, &stable_pool_hash1),
        0
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens2, &standard_pool_hash2),
        0
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens2, &stable_pool_hash2),
        0
    );

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    router.deposit(
        &user1,
        &tokens1,
        &standard_pool_hash1,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    router.deposit(
        &user1,
        &tokens2,
        &standard_pool_hash2,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    let standard_liquidity1 = router.get_total_liquidity(&tokens1);
    let standard_liquidity2 = router.get_total_liquidity(&tokens2);
    assert_eq!(standard_liquidity1, U256::from_u32(&e, 34));
    assert_eq!(standard_liquidity2, U256::from_u32(&e, 34));
    router.deposit(
        &user1,
        &tokens1,
        &stable_pool_hash1,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    router.deposit(
        &user1,
        &tokens2,
        &stable_pool_hash2,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    let stable_liquidity1 = router
        .get_total_liquidity(&tokens1)
        .sub(&standard_liquidity1);
    let stable_liquidity2 = router
        .get_total_liquidity(&tokens2)
        .sub(&standard_liquidity2);
    assert_eq!(
        standard_liquidity1.add(&stable_liquidity1),
        U256::from_u32(&e, 370)
    );
    assert_eq!(
        standard_liquidity2.add(&stable_liquidity2),
        U256::from_u32(&e, 370)
    );

    let rewards = Vec::from_array(
        &e,
        [(tokens1.clone(), 0_5000000), (tokens2.clone(), 0_5000000)],
    );
    router.config_global_rewards(
        &admin,
        &reward_tps,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    router.fill_liquidity(&tokens1);
    router.fill_liquidity(&tokens2);
    let standard_pool_tps1 = router.config_pool_rewards(&tokens1, &standard_pool_hash1);
    let stable_pool_tps1 = router.config_pool_rewards(&tokens1, &stable_pool_hash1);
    let standard_pool_tps2 = router.config_pool_rewards(&tokens2, &standard_pool_hash2);
    let stable_pool_tps2 = router.config_pool_rewards(&tokens2, &stable_pool_hash2);
    assert_eq!(standard_pool_tps1, standard_pool_tps2);
    assert_eq!(stable_pool_tps1, stable_pool_tps2);
    let standard_pool_tps = standard_pool_tps1;
    let stable_pool_tps = stable_pool_tps1;

    assert_eq!(reward_token.balance(&user1), 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);

    assert_eq!(
        router.get_total_accumulated_reward(&tokens1, &standard_pool_hash1),
        standard_pool_tps * 30
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens1, &standard_pool_hash1),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens1, &standard_pool_hash1),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.get_total_accumulated_reward(&tokens1, &stable_pool_hash1),
        stable_pool_tps * 30
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens1, &stable_pool_hash1),
        stable_pool_tps * 60
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens1, &stable_pool_hash1),
        stable_pool_tps * 60
    );
    assert_eq!(
        router.get_total_accumulated_reward(&tokens2, &standard_pool_hash2),
        standard_pool_tps * 30
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens2, &standard_pool_hash2),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens2, &standard_pool_hash2),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.get_total_accumulated_reward(&tokens2, &stable_pool_hash2),
        stable_pool_tps * 30
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens2, &stable_pool_hash2),
        stable_pool_tps * 60
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens2, &stable_pool_hash2),
        stable_pool_tps * 60
    );

    assert_eq!(reward_token.balance(&standard_pool_address1), 0);
    assert_eq!(reward_token.balance(&stable_pool_address1), 0);
    assert_eq!(reward_token.balance(&standard_pool_address2), 1000);
    assert_eq!(reward_token.balance(&stable_pool_address2), 1000);
    assert_eq!(
        router.distribute_outstanding_reward(
            &admin,
            &router.address,
            &tokens1,
            &standard_pool_hash1
        ),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.distribute_outstanding_reward(&admin, &router.address, &tokens1, &stable_pool_hash1),
        stable_pool_tps * 60
    );
    assert_eq!(
        router.distribute_outstanding_reward(
            &admin,
            &router.address,
            &tokens2,
            &standard_pool_hash2
        ),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.distribute_outstanding_reward(&admin, &router.address, &tokens2, &stable_pool_hash2),
        stable_pool_tps * 60
    );
    assert_eq!(
        router.distribute_outstanding_reward(
            &admin,
            &router.address,
            &tokens1,
            &standard_pool_hash1
        ),
        0
    );
    assert_eq!(
        router.distribute_outstanding_reward(&admin, &router.address, &tokens1, &stable_pool_hash1),
        0
    );
    assert_eq!(
        router.distribute_outstanding_reward(
            &admin,
            &router.address,
            &tokens2,
            &standard_pool_hash2
        ),
        0
    );
    assert_eq!(
        router.distribute_outstanding_reward(&admin, &router.address, &tokens2, &stable_pool_hash2),
        0
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens1, &stable_pool_hash1),
        0
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens2, &stable_pool_hash2),
        0
    );

    // deposit again to check how reserves being calculated
    token1.mint(&user1, &4000);
    token2.mint(&user1, &2000);
    reward_token.mint(&user1, &2000);
    router.deposit(
        &user1,
        &tokens1,
        &standard_pool_hash1,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    router.deposit(
        &user1,
        &tokens2,
        &standard_pool_hash2,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    router.deposit(
        &user1,
        &tokens1,
        &stable_pool_hash1,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    router.deposit(
        &user1,
        &tokens2,
        &stable_pool_hash2,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );

    // reward balance of pools2 equals to total reward + reserves
    assert_eq!(
        reward_token.balance(&standard_pool_address1) as u128,
        standard_pool_tps * 60
    );
    assert_eq!(
        reward_token.balance(&stable_pool_address1) as u128,
        stable_pool_tps * 60
    );
    assert_eq!(
        reward_token.balance(&standard_pool_address2) as u128,
        standard_pool_tps * 60 + 2000
    );
    assert_eq!(
        reward_token.balance(&stable_pool_address2) as u128,
        stable_pool_tps * 60 + 2000
    );

    // reserves don't include rewards
    assert_eq!(
        router.get_reserves(&tokens1, &standard_pool_hash1),
        Vec::from_array(&e, [2000, 2000])
    );
    assert_eq!(
        router.get_reserves(&tokens2, &standard_pool_hash2),
        Vec::from_array(&e, [2000, 2000])
    );
    assert_eq!(
        router.get_reserves(&tokens1, &stable_pool_hash1),
        Vec::from_array(&e, [2000, 2000])
    );
    assert_eq!(
        router.get_reserves(&tokens2, &stable_pool_hash2),
        Vec::from_array(&e, [2000, 2000])
    );
}

#[test]
fn test_rewards_distribution_as_operator() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);
    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

    reward_token.mint(&user1, &1000_0000000);
    reward_token.mint(&router.address, &2_000_000_0000000);
    reward_token.mint(&admin, &2_000_000_0000000);

    let (standard_pool_hash, _standard_pool_address) =
        router.init_standard_pool(&user1, &tokens, &30);
    let (stable_pool_hash, _stable_pool_address) =
        router.init_stableswap_pool(&user1, &tokens, &10);

    let reward_1_tps = 10_5000000_u128;

    token1.mint(&user1, &2000);
    token2.mint(&user1, &2000);

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    router.deposit(
        &user1,
        &tokens,
        &standard_pool_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    router.deposit(
        &user1,
        &tokens,
        &stable_pool_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );

    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &admin,
        &reward_1_tps,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    router.fill_liquidity(&tokens);
    let standard_pool_tps = router.config_pool_rewards(&tokens, &standard_pool_hash);
    let stable_pool_tps = router.config_pool_rewards(&tokens, &stable_pool_hash);

    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);

    // operator not set yet. admin should be able to distribute rewards but no one else should
    let operator = Address::generate(&e);
    assert!(router
        .try_distribute_outstanding_reward(&user1, &router.address, &tokens, &standard_pool_hash)
        .is_err(),);
    assert!(router
        .try_distribute_outstanding_reward(&operator, &router.address, &tokens, &standard_pool_hash)
        .is_err(),);
    router.set_privileged_addrs(
        &admin,
        &operator,
        &admin,
        &admin,
        &Vec::from_array(&e, [admin.clone()]),
    );
    assert!(router
        .try_distribute_outstanding_reward(&user1, &router.address, &tokens, &standard_pool_hash)
        .is_err(),);
    assert_eq!(
        router.distribute_outstanding_reward(
            &operator,
            &router.address,
            &tokens,
            &standard_pool_hash
        ),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.distribute_outstanding_reward(
            &operator,
            &router.address,
            &tokens,
            &stable_pool_hash
        ),
        stable_pool_tps * 60
    );
}

#[test]
fn test_rewards_distribution_override() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);

    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

    reward_token.mint(&user1, &1000_0000000);
    reward_token.mint(&router.address, &2_000_000_0000000);
    reward_token.mint(&admin, &2_000_000_0000000);

    let (standard_pool_hash, _standard_pool_address) =
        router.init_standard_pool(&user1, &tokens, &30);
    let (stable_pool_hash, _stable_pool_address) =
        router.init_stableswap_pool(&user1, &tokens, &10);

    let reward_1_tps = 10_5000000_u128;

    token1.mint(&user1, &2000);
    token2.mint(&user1, &2000);

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    router.deposit(
        &user1,
        &tokens,
        &standard_pool_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );
    router.deposit(
        &user1,
        &tokens,
        &stable_pool_hash,
        &Vec::from_array(&e, [1000, 1000]),
        &0,
    );

    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &admin,
        &reward_1_tps,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );
    router.fill_liquidity(&tokens);
    let standard_pool_tps = router.config_pool_rewards(&tokens, &standard_pool_hash);
    let stable_pool_tps = router.config_pool_rewards(&tokens, &stable_pool_hash);

    // 30 seconds passed, half of the reward is available
    jump(&e, 30);

    // tps * 60 configured in total & outstanding since there were no claims
    assert_eq!(
        router.get_total_configured_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 60
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 60
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 60
    );

    // however since just 30 seconds passed, only half of the reward accumulated
    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 30
    );
    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 30
    );

    router.config_global_rewards(
        &admin,
        &0,
        &e.ledger().timestamp().saturating_add(10),
        &rewards,
    );
    router.fill_liquidity(&tokens);
    router.config_pool_rewards(&tokens, &standard_pool_hash);
    router.config_pool_rewards(&tokens, &stable_pool_hash);

    // half of the reward accumulated
    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 30
    );
    assert_eq!(
        router.get_total_accumulated_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 30
    );

    // but since we've re-configured reward in the middle, the total configured reward should be tps * 30 as well as outstanding balance
    assert_eq!(
        router.get_total_configured_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 30
    );
    assert_eq!(
        router.get_total_configured_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 30
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens, &standard_pool_hash),
        standard_pool_tps * 30
    );
    assert_eq!(
        router.get_total_outstanding_reward(&tokens, &stable_pool_hash),
        stable_pool_tps * 30
    );

    // operator not set yet. admin should be able to distribute rewards but no one else should
    let rewards_admin = Address::generate(&e);
    router.set_privileged_addrs(
        &admin,
        &rewards_admin,
        &admin,
        &admin,
        &Vec::from_array(&e, [admin.clone()]),
    );
    assert_eq!(
        router.distribute_outstanding_reward(
            &rewards_admin,
            &router.address,
            &tokens,
            &standard_pool_hash
        ),
        standard_pool_tps * 30
    );
    assert_eq!(
        router.distribute_outstanding_reward(
            &rewards_admin,
            &router.address,
            &tokens,
            &stable_pool_hash
        ),
        stable_pool_tps * 30
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #309)")]
fn test_liqidity_not_filled() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);
    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);

    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);

    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);

    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

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

    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);

    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);

    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);

    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

    reward_token.mint(&user1, &1000_0000000);
    router.init_standard_pool(&user1, &tokens, &30);
    router.init_stableswap_pool(&user1, &tokens, &30);

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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);

    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

    reward_token.mint(&user1, &1000_0000000);
    router.init_standard_pool(&user1, &tokens, &30);
    router.init_stableswap_pool(&user1, &tokens, &30);

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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);

    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

    reward_token.mint(&user1, &1000_0000000);
    router.init_standard_pool(&user1, &tokens, &30);
    router.init_stableswap_pool(&user1, &tokens, &30);

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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &router.address,
    );

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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);
    reward_token.mint(&user1, &10_0000000);

    let fee = CONSTANT_PRODUCT_FEE_AVAILABLE[1] + 1;
    router.init_standard_pool(&user1, &tokens, &fee);
}

#[test]
fn test_event_correct() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);

    let payment_for_creation_address = router.get_init_pool_payment_address();
    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &0,
        &1000_0000000,
        &payment_for_creation_address,
    );
    assert_eq!(reward_token.balance(&payment_for_creation_address), 0);

    reward_token.mint(&user1, &10000000_0000000);
    let fee = CONSTANT_PRODUCT_FEE_AVAILABLE[1];

    let (pool_hash, pool_address) = router.init_stableswap_pool(&user1, &tokens, &fee);
    let init_stableswap_pool_event = e.events().all().last().unwrap();
    assert_eq!(
        reward_token.balance(&payment_for_creation_address),
        1000_0000000
    );

    assert_eq!(
        vec![&e, init_stableswap_pool_event],
        vec![
            &e,
            (
                router.address.clone(),
                (Symbol::new(&e, "add_pool"), tokens.clone()).into_val(&e),
                (
                    pool_address.clone(),
                    symbol_short!("stable"),
                    pool_hash.clone(),
                    Vec::<Val>::from_array(&e, [fee.into_val(&e), 1500_u128.into_val(&e)],),
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
                router.address.clone(),
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
    let deposit_event = e.events().all().last().unwrap();
    assert_eq!(router.get_total_liquidity(&tokens), U256::from_u32(&e, 2));

    let pool_id = router.get_pool(&tokens, &pool_hash);

    assert_eq!(
        vec![&e, deposit_event],
        vec![
            &e,
            (
                router.address.clone(),
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
                router.address.clone(),
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
                router.address.clone(),
                (Symbol::new(&e, "withdraw"), tokens.clone(), user1.clone()).into_val(&e),
                (pool_id.clone(), 100_u128, amounts).into_val(&e)
            ),
        ]
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #302)")]
fn test_stableswap_validation_fee_out_of_bounds() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let user1 = Address::generate(&e);
    reward_token.mint(&user1, &10_0000000);

    router.init_stableswap_pool(
        &user1,
        &Vec::from_array(&e, [token1.address, token2.address]),
        &101,
    );
}

#[test]
fn test_tokens_storage() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let [token1, token2, token3, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = [
        token1.address.clone(),
        token2.address.clone(),
        token3.address.clone(),
    ];

    let user1 = Address::generate(&e);
    reward_token.mint(&user1, &100_0000000);

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
        router.init_stableswap_pool(&user1, &pair, &0);
        router.init_stableswap_pool(&user1, &pair, &0);
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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, token3, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let user1 = Address::generate(&e);
    reward_token.mint(&user1, &10_0000000);
    e.mock_auths(&[]);

    let tokens1 = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);
    let tokens2 = Vec::from_array(&e, [token2.address.clone(), token3.address.clone()]);

    let swapper = Address::generate(&e);

    router.mock_all_auths().configure_init_pool_payment(
        &admin,
        &testutils::create_token_contract(&e, &admin).address,
        &0,
        &0,
        &router.address,
    );

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
            (tokens1.clone(), pool_index1.clone(), token2.address.clone()),
            (tokens2.clone(), pool_index2.clone(), token3.address.clone()),
        ]
        .into_val(&e),
        token1.address.clone().clone().to_val(),
        100_u128.into_val(&e),
        96_u128.into_val(&e),
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
                    (tokens1.clone(), pool_index1.clone(), token2.address.clone()),
                    (tokens2.clone(), pool_index2.clone(), token3.address.clone()),
                ],
                &token1.address.clone(),
                &100,
                &96,
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
fn test_chained_swap_strict_receive() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, token3, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let user1 = Address::generate(&e);
    reward_token.mint(&user1, &10_0000000);
    e.mock_auths(&[]);

    let tokens1 = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);
    let tokens2 = Vec::from_array(&e, [token2.address.clone(), token3.address.clone()]);

    let swapper = Address::generate(&e);

    router.mock_all_auths().configure_init_pool_payment(
        &admin,
        &testutils::create_token_contract(&e, &admin).address,
        &0,
        &0,
        &router.address,
    );

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
            (tokens1.clone(), pool_index1.clone(), token2.address.clone()),
            (tokens2.clone(), pool_index2.clone(), token3.address.clone()),
        ]
        .into_val(&e),
        token1.address.clone().clone().to_val(),
        96_u128.into_val(&e),
        100_u128.into_val(&e),
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
                    fn_name: "swap_chained_strict_receive",
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
            .swap_chained_strict_receive(
                &swapper,
                &vec![
                    &e,
                    (tokens1.clone(), pool_index1.clone(), token2.address.clone()),
                    (tokens2.clone(), pool_index2.clone(), token3.address.clone()),
                ],
                &token1.address.clone(),
                &96,
                &100,
            ),
        100
    );
    assert_eq!(
        e.auths(),
        std::vec![(
            swapper.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    router.address.clone(),
                    Symbol::new(&e, "swap_chained_strict_receive"),
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
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, token3, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = [
        token1.address.clone(),
        token2.address.clone(),
        token3.address.clone(),
    ];

    let tokens1 = Vec::from_array(&e, [tokens[0].clone(), tokens[1].clone()]);
    let tokens2 = Vec::from_array(&e, [tokens[1].clone(), tokens[2].clone()]);

    let swapper = Address::generate(&e);
    reward_token.mint(&swapper, &10_0000000);

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

#[test]
fn test_create_pool_payment() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;
    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);
    reward_token.mint(&user1, &10_0000000);

    let payments_destination = router.get_init_pool_payment_address();
    router.configure_init_pool_payment(
        &admin,
        &reward_token.address,
        &100,
        &1000,
        &payments_destination,
    );

    assert_eq!(reward_token.balance(&payments_destination), 0);
    router.init_standard_pool(&user1, &tokens, &30);
    assert_eq!(reward_token.balance(&payments_destination), 100);
    router.init_stableswap_pool(&user1, &tokens, &30);
    assert_eq!(reward_token.balance(&payments_destination), 1100);
}

#[test]
fn test_rewards_distribution_without_outstanding_rewards() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;
    let admin = setup.admin;

    let [token, _, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token.address.clone(), reward_token.address.clone()]);
    let user = Address::generate(&e);

    reward_token.mint(&user, &200000_0000000);
    reward_token.mint(&router.address, &20_000_000_0000000);

    let (standard_pool_hash1, standard_pool_address1) =
        router.init_standard_pool(&user, &tokens, &30);

    let reward_tps = 1_5000000_u128;

    token.mint(&user, &(i128::MAX / 100));
    reward_token.mint(&user, &(i128::MAX / 100));

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    router.deposit(
        &user,
        &tokens,
        &standard_pool_hash1,
        &Vec::from_array(&e, [30399483, 2420176738]),
        &0,
    );

    reward_token.mint(&standard_pool_address1, &(3888205486 - 2420176738));
    let rewards = Vec::from_array(&e, [(tokens.clone(), 1_0000000)]);
    router.config_global_rewards(
        &admin,
        &reward_tps,
        &e.ledger().timestamp().saturating_add(60),
        &rewards,
    );

    router.fill_liquidity(&tokens);
    router.config_pool_rewards(&tokens, &standard_pool_hash1);

    // check that we don't need to add rewards to pool
    assert_eq!(
        router.get_total_outstanding_reward(&tokens, &standard_pool_hash1),
        0
    );

    // check that it works without panicking
    assert_eq!(
        router.distribute_outstanding_reward(
            &admin,
            &router.address,
            &tokens,
            &standard_pool_hash1
        ),
        0
    );
}

#[test]
fn test_privileged_users() {
    let setup = Setup::default();
    let e = setup.env;
    let router = setup.router;

    let [token1, token2, _, _] = setup.tokens;
    let reward_token = setup.reward_token;

    let tokens = Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]);

    let user1 = Address::generate(&e);
    reward_token.mint(&user1, &10_0000000);

    let (_, standard_address) = router.init_standard_pool(&user1, &tokens, &30);
    let (_, stable_address) = router.init_stableswap_pool(&user1, &tokens, &30);
    let privileged_addrs: Map<Symbol, Vec<Address>> = Map::from_array(
        &e,
        [
            (Symbol::new(&e, "Admin"), Vec::from_array(&e, [setup.admin])),
            (
                Symbol::new(&e, "EmergencyAdmin"),
                Vec::from_array(&e, [setup.emergency_admin]),
            ),
            (
                Symbol::new(&e, "RewardsAdmin"),
                Vec::from_array(&e, [setup.rewards_admin]),
            ),
            (
                Symbol::new(&e, "OperationsAdmin"),
                Vec::from_array(&e, [setup.operations_admin]),
            ),
            (
                Symbol::new(&e, "PauseAdmin"),
                Vec::from_array(&e, [setup.pause_admin]),
            ),
            (
                Symbol::new(&e, "EmergencyPauseAdmin"),
                Vec::from_array(&e, [setup.emergency_pause_admin]),
            ),
        ],
    );
    assert_eq!(privileged_addrs, router.get_privileged_addrs());
    // test addresses inheritance
    assert_eq!(
        privileged_addrs,
        testutils::standard_pool::Client::new(&e, &standard_address).get_privileged_addrs()
    );
    assert_eq!(
        privileged_addrs,
        testutils::stableswap_pool::Client::new(&e, &stable_address).get_privileged_addrs()
    );
}

#[test]
fn test_set_privileged_addresses_event() {
    let setup = Setup::default();
    let router = setup.router;

    router.set_privileged_addrs(
        &setup.admin.clone(),
        &setup.rewards_admin.clone(),
        &setup.operations_admin.clone(),
        &setup.pause_admin.clone(),
        &Vec::from_array(&setup.env, [setup.emergency_pause_admin.clone()]),
    );

    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                router.address.clone(),
                (Symbol::new(&setup.env, "set_privileged_addrs"),).into_val(&setup.env),
                (
                    setup.rewards_admin,
                    setup.operations_admin,
                    setup.pause_admin,
                    Vec::from_array(&setup.env, [setup.emergency_pause_admin]),
                )
                    .into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_transfer_ownership_events() {
    let setup = Setup::default();
    let router = setup.router;
    let new_admin = Address::generate(&setup.env);

    router.commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &new_admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                router.address.clone(),
                (
                    Symbol::new(&setup.env, "commit_transfer_ownership"),
                    symbol_short!("Admin")
                )
                    .into_val(&setup.env),
                (new_admin.clone(),).into_val(&setup.env),
            ),
        ]
    );

    router.revert_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                router.address.clone(),
                (
                    Symbol::new(&setup.env, "revert_transfer_ownership"),
                    symbol_short!("Admin")
                )
                    .into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );

    router.commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &new_admin);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    router.apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                router.address.clone(),
                (
                    Symbol::new(&setup.env, "apply_transfer_ownership"),
                    symbol_short!("Admin")
                )
                    .into_val(&setup.env),
                (new_admin.clone(),).into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_upgrade_events() {
    let setup = Setup::default();
    let contract = setup.router;
    let new_wasm_hash = install_dummy_wasm(&setup.env);

    contract.commit_upgrade(&setup.admin, &new_wasm_hash);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "commit_upgrade"),).into_val(&setup.env),
                (new_wasm_hash.clone(),).into_val(&setup.env),
            ),
        ]
    );

    contract.revert_upgrade(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "revert_upgrade"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );

    contract.commit_upgrade(&setup.admin, &new_wasm_hash);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    contract.apply_upgrade(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "apply_upgrade"),).into_val(&setup.env),
                (new_wasm_hash.clone(),).into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_emergency_mode_events() {
    let setup = Setup::default();
    let contract = setup.router;

    contract.set_emergency_mode(&setup.emergency_admin, &true);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "enable_emergency_mode"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );
    contract.set_emergency_mode(&setup.emergency_admin, &false);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "disable_emergency_mode"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_emergency_upgrade() {
    let setup = Setup::default();
    let contract = setup.router;
    let new_wasm = install_dummy_wasm(&setup.env);

    assert_eq!(contract.get_emergency_mode(), false);
    assert_ne!(contract.version(), 130);
    contract.set_emergency_mode(&setup.emergency_admin, &true);

    contract.commit_upgrade(&setup.admin, &new_wasm);
    contract.apply_upgrade(&setup.admin);

    assert_eq!(contract.version(), 130)
}

#[test]
fn test_regular_upgrade() {
    let setup = Setup::default();
    let contract = setup.router;
    let new_wasm = install_dummy_wasm(&setup.env);

    assert_eq!(contract.get_emergency_mode(), false);
    assert_ne!(contract.version(), 130);

    contract.commit_upgrade(&setup.admin, &new_wasm);
    assert!(contract.try_apply_upgrade(&setup.admin).is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    contract.apply_upgrade(&setup.admin);

    assert_eq!(contract.version(), 130)
}
