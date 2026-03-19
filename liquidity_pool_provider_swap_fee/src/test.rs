#![cfg(test)]
extern crate std;

use crate::testutils::{
    create_contract, create_reward_boost_feed_contract, create_token_contract,
    deploy_plane_contract, get_token_admin_client, install_liq_pool_hash,
    install_stableswap_liq_pool_hash, install_token_wasm, liquidity_pool, swap_router, Setup,
};
use liquidity_pool_config_storage::testutils::deploy_config_storage;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{vec, Address, Env, Vec};
use utils::test_rebasing_token;

#[test]
fn test_strict_send() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    let result = setup.contract.swap_chained(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &9870299,
        &100,
    );
    assert_eq!(result, 9870299); // (10000000 - .3%) - 1%
    assert_eq!(setup.token_b.balance(&user), 9870299);
}

#[test]
fn test_strict_receive() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_2000000);
    let result = setup.contract.swap_chained_strict_receive(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &1_2000000,
        &100,
    );
    assert_eq!(result, 10131407); // ~ (10000000 + .3%) + 1%
    assert_eq!(setup.token_b.balance(&user), 1_0000000);
    assert_eq!(setup.token_a.balance(&setup.contract.address), 0);
    assert_eq!(setup.token_b.balance(&setup.contract.address), 101011);
}

#[test]
#[should_panic(expected = "Error(Contract, #2904)")]
fn test_strict_send_fee_over_max() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    setup.contract.swap_chained(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &9870300,
        &101,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2904)")]
fn test_strict_receive_fee_over_max() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_2000000);
    setup.contract.swap_chained_strict_receive(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &1_2000000,
        &101,
    );
}

#[test]
fn test_strict_send_bad_slippage() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    let swap_path = Vec::from_array(
        &setup.env,
        [(tokens, pool_index, setup.token_b.address.clone())],
    );
    assert!(setup
        .contract
        .try_swap_chained(
            &user,
            &swap_path,
            &setup.token_a.address,
            &1_0000000,
            &9870300, // value is not enough to cover provider fee
            &100,
        )
        .is_err());
    assert!(setup
        .contract
        .try_swap_chained(
            &user,
            &swap_path,
            &setup.token_a.address,
            &1_0000000,
            &9870299,
            &100,
        )
        .is_ok());
}

#[test]
fn test_strict_receive_bad_slippage() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_2000000);
    let swap_path = Vec::from_array(
        &setup.env,
        [(tokens, pool_index, setup.token_b.address.clone())],
    );
    assert!(setup
        .contract
        .try_swap_chained_strict_receive(
            &user,
            &swap_path,
            &setup.token_a.address,
            &1_0000000,
            &10131406,
            &100,
        )
        .is_err());
    assert!(setup
        .contract
        .try_swap_chained_strict_receive(
            &user,
            &swap_path,
            &setup.token_a.address,
            &1_0000000,
            &10131407,
            &100,
        )
        .is_ok());
}

#[test]
fn test_strict_send_no_fee() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    let result = setup.contract.swap_chained(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &0,
        &0,
    );
    assert_eq!(result, 9969999); // (10000000 - .3%)
}

#[test]
fn test_strict_receive_no_fee() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_2000000);
    let result = setup.contract.swap_chained_strict_receive(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &1_2000000,
        &0,
    );
    assert_eq!(result, 10030092); // ~ (10000000 + .3%)
}

#[test]
fn test_claim_fee() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    setup.contract.swap_chained(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &0,
        &100,
    );
    assert_eq!(
        setup
            .contract
            .claim_fees(&setup.operator, &setup.token_b.address),
        99700
    ); // ~ (10000000 - .3%) * 1%
    assert_eq!(
        setup
            .contract
            .claim_fees(&setup.operator, &setup.token_a.address),
        0
    );
    assert_eq!(setup.token_a.balance(&setup.fee_destination), 0);
    assert_eq!(setup.token_b.balance(&setup.fee_destination), 99700);
}

#[test]
fn test_claim_fee_and_swap() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    setup.contract.swap_chained(
        &user,
        &Vec::from_array(
            &setup.env,
            [(
                tokens.clone(),
                pool_index.clone(),
                setup.token_b.address.clone(),
            )],
        ),
        &setup.token_a.address,
        &1_0000000,
        &0,
        &100,
    );
    assert_eq!(
        setup.contract.claim_fees_and_swap(
            &setup.operator,
            &Vec::from_array(
                &setup.env,
                [(tokens, pool_index, setup.token_a.address.clone())],
            ),
            &setup.token_b.address,
            &0,
        ),
        99400
    ); // ~ (10000000 - .3%) * 1%
    assert_eq!(
        setup
            .contract
            .claim_fees(&setup.operator, &setup.token_a.address),
        0
    );
    assert_eq!(setup.token_a.balance(&setup.fee_destination), 99400);
    assert_eq!(setup.token_b.balance(&setup.fee_destination), 0);
}

#[test]
fn test_swap_equivalence_send_receive() {
    // Strict‐send
    let setup_send = Setup::default();
    let tokens = Vec::from_array(
        &setup_send.env,
        [
            setup_send.token_a.address.clone(),
            setup_send.token_b.address.clone(),
        ],
    );
    let (pool_index, _) = setup_send.router.get_pools(&tokens).iter().last().unwrap();

    let user_send = Address::generate(&setup_send.env);
    setup_send.token_a_admin_client.mint(&user_send, &1_0000000);

    let in_amount: u128 = 1_0000000;
    let fee_fraction: u32 = 100; // 1%
    let path_send = Vec::from_array(
        &setup_send.env,
        [(
            tokens.clone(),
            pool_index.clone(),
            setup_send.token_b.address.clone(),
        )],
    );

    let out_send = setup_send.contract.swap_chained(
        &user_send,
        &path_send,
        &setup_send.token_a.address,
        &in_amount,
        &0,
        &fee_fraction,
    );
    // Collected fee (held in contract)
    let fee_send = setup_send.token_b.balance(&setup_send.contract.address) as u128;

    // Strict‐receive: invert the same path using out_send
    let setup_receive = Setup::default();
    let tokens2 = Vec::from_array(
        &setup_receive.env,
        [
            setup_receive.token_a.address.clone(),
            setup_receive.token_b.address.clone(),
        ],
    );
    let (pool_index2, _) = setup_receive
        .router
        .get_pools(&tokens2)
        .iter()
        .last()
        .unwrap();

    let user_receive = Address::generate(&setup_receive.env);
    // Mint enough so strict‐receive's gross_in ≤ this amount
    setup_receive
        .token_a_admin_client
        .mint(&user_receive, &(in_amount as i128 * 2));

    let path_receive = Vec::from_array(
        &setup_receive.env,
        [(
            tokens2.clone(),
            pool_index2.clone(),
            setup_receive.token_b.address.clone(),
        )],
    );

    let in_receive = setup_receive.contract.swap_chained_strict_receive(
        &user_receive,
        &path_receive,
        &setup_receive.token_a.address,
        &out_send,
        &(in_amount * 2),
        &fee_fraction,
    );
    // Fee collected in strict‐receive (held in contract)
    let fee_receive = setup_receive
        .token_b
        .balance(&setup_receive.contract.address) as u128;

    // User net output matches
    assert_eq!(setup_send.token_b.balance(&user_send), out_send as i128);
    assert_eq!(
        setup_receive.token_b.balance(&user_receive),
        out_send as i128
    );

    // Input consumed matches (strict‐receive should use exactly in_amount)
    assert_eq!(in_receive, in_amount);

    // Fees match exactly
    assert_eq!(fee_receive, fee_send);

    assert_eq!(
        setup_send
            .contract
            .claim_fees(&setup_send.operator, &setup_send.token_b.address),
        fee_send
    );
    assert_eq!(
        setup_receive
            .contract
            .claim_fees(&setup_receive.operator, &setup_receive.token_b.address),
        fee_receive
    );
    assert_eq!(
        setup_send.token_b.balance(&setup_send.fee_destination) as u128,
        fee_send
    );
    assert_eq!(
        setup_receive
            .token_b
            .balance(&setup_receive.fee_destination) as u128,
        fee_receive
    );
}

/// Regression test: swap_chained_strict_receive in ProviderSwapFee with
/// a rebasing input token must not panic with InsufficientBalance.
/// The fix uses balance delta for refund instead of arithmetic surplus.
#[test]
fn test_strict_receive_rebasing_token() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);
    let operator = Address::generate(&e);
    let fee_destination = Address::generate(&e);

    // Deploy rebasing token (K=103, K_SCALE=100): ceil-rounds on transfer
    let rebasing_addr = e.register(test_rebasing_token::RebasingToken, ());
    let rebasing = test_rebasing_token::RebasingTokenClient::new(&e, &rebasing_addr);
    rebasing.initialize(&admin, &103, &100);

    let token_b = create_token_contract(&e, &admin);
    let token_b_admin = get_token_admin_client(&e, &token_b.address);

    let tokens = if rebasing_addr < token_b.address {
        Vec::from_array(&e, [rebasing_addr.clone(), token_b.address.clone()])
    } else {
        Vec::from_array(&e, [token_b.address.clone(), rebasing_addr.clone()])
    };

    // Init router
    let plane = deploy_plane_contract(&e);
    let boost_feed = create_reward_boost_feed_contract(&e, &admin);
    let router = swap_router::Client::new(&e, &e.register(swap_router::WASM, ()));
    router.init_admin(&admin);
    router.init_config_storage(&admin, &deploy_config_storage(&e, &admin, &admin).address);
    router.set_pool_hash(&admin, &install_liq_pool_hash(&e));
    router.set_stableswap_pool_hash(&admin, &install_stableswap_liq_pool_hash(&e));
    router.set_token_hash(&admin, &install_token_wasm(&e));
    router.set_reward_token(&admin, &tokens.get(0).unwrap());
    router.set_pools_plane(&admin, &plane);
    router.configure_init_pool_payment(
        &admin,
        &tokens.get(0).unwrap(),
        &0,
        &0,
        &0,
        &router.address,
    );
    router.set_reward_boost_config(&admin, &tokens.get(0).unwrap(), &boost_feed.address);
    router.set_protocol_fee_fraction(&admin, &5000);

    // Create pool & deposit liquidity
    let liq: i128 = 1_000_000_0000000;
    rebasing.mint(&admin, &liq);
    token_b_admin.mint(&admin, &liq);
    let (_, pool_address) = router.init_standard_pool(&admin, &tokens, &30);
    liquidity_pool::Client::new(&e, &pool_address).deposit(
        &admin,
        &Vec::from_array(&e, [liq as u128, liq as u128]),
        &1,
    );

    let contract = create_contract(
        &e,
        &router.address,
        &operator,
        &fee_destination,
        100,
        10_000,
    );

    // Swap: rebasing → token_b via provider fee contract
    let user = Address::generate(&e);
    rebasing.mint(&user, &10_0000000);

    let (pool_index, _) = router.get_pools(&tokens).iter().last().unwrap();
    let token_out = token_b.address.clone();
    let out_amount: u128 = 1_0000000;
    let in_max: u128 = 10_0000000;

    let result = contract.swap_chained_strict_receive(
        &user,
        &vec![&e, (tokens.clone(), pool_index, token_out.clone())],
        &rebasing_addr,
        &out_amount,
        &in_max,
        &100,
    );

    assert!(result > 0 && result < in_max);
    assert_eq!(
        SorobanTokenClient::new(&e, &token_out).balance(&user),
        out_amount as i128
    );
    assert_eq!(rebasing.balance(&contract.address), 0);
}
