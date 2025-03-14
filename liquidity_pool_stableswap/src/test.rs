#![cfg(test)]
extern crate std;

use crate::pool_constants::MIN_RAMP_TIME;
use core::cmp::min;
use rewards::utils::test_utils::assert_approx_eq_abs;
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{symbol_short, vec, Address, Env, Error, IntoVal, Symbol, Val, Vec};
use token_share::Client as ShareTokenClient;

use crate::testutils::{
    create_liqpool_contract, create_plane_contract, create_reward_boost_feed_contract,
    create_token_contract, get_token_admin_client, install_token_wasm,
    install_token_wasm_with_decimal, Setup, TestConfig,
};
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use utils::test_utils::{install_dummy_wasm, jump};

#[test]
#[should_panic(expected = "Error(Contract, #2010)")]
fn test_swap_empty_pool() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token_reward = create_token_contract(&e, &admin1);
    let plane = create_plane_contract(&e);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );
    assert_eq!(liqpool.estimate_swap(&0, &1, &10_0000000), 0);
    token1_admin_client.mint(&user1, &10_0000000);
    assert_eq!(liqpool.swap(&user1, &0, &1, &10_0000000, &0), 0);
}

#[test]
fn test_happy_flow() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
    assert_eq!(
        plane
            .get(&Vec::from_array(&e, [liqpool.address.clone()]))
            .get_unchecked(0)
            .2,
        Vec::from_array(&e, [100_0000000, 100_0000000,])
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
    assert_eq!(
        plane
            .get(&Vec::from_array(&e, [liqpool.address.clone()]))
            .get_unchecked(0)
            .2,
        Vec::from_array(&e, [200_0000000, 200_0000000,])
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
    let calculated_amount =
        liqpool.calc_token_amount(&Vec::from_array(&e, [10_0000000, 10_0000000]), &true);

    let total_share_token_amount = 400_0000000_u128; // share amount after two deposits

    assert_eq!(calculated_amount as u128, total_share_token_amount / 2 / 10);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount
    );
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token1.balance(&user1) as u128, 800_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token2.balance(&user1) as u128, 800_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 200_0000000);

    assert_eq!(liqpool.estimate_swap(&0, &1, &10_0000000), 79637266);
    liqpool.swap(&user1, &0, &1, &10_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 790_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 210_0000000);
    assert_eq!(token2.balance(&user1) as u128, 807_9637266);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362734);

    liqpool.withdraw(
        &user1,
        &(total_share_token_amount / 2),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 895_0000000);
    assert_eq!(token2.balance(&user1) as u128, 903_9818633);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount / 2
    );
    assert_eq!(token1.balance(&liqpool.address) as u128, 105_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 96_0181367);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);

    liqpool.withdraw(
        &user1,
        &(total_share_token_amount / 2),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 0);
    assert_eq!(token1.balance(&liqpool.address) as u128, 0);
    assert_eq!(token2.balance(&liqpool.address) as u128, 0);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[test]
fn test_strict_receive() {
    // mirror calculations from test_happy_flow to ensure that swap_strict_receive works as expected
    let setup = Setup::new_with_config(&TestConfig {
        a: 10,
        liq_pool_fee: 2000,
        ..TestConfig::default()
    });

    let token1_admin_client = get_token_admin_client(&setup.env, &setup.token1.address);
    let token2_admin_client = get_token_admin_client(&setup.env, &setup.token2.address);
    let user1 = Address::generate(&setup.env);

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);
    setup.liq_pool.deposit(
        &user1,
        &Vec::from_array(&setup.env, [200_0000000, 200_0000000]),
        &0,
    );

    // that's what we expect from test_happy_flow
    let swap_amount_in = 10_0000000;
    let swap_amount_out = 7_9637266;
    assert_eq!(
        setup.liq_pool.estimate_swap(&0, &1, &swap_amount_in),
        swap_amount_out
    );

    // reverse values
    assert_eq!(
        setup
            .liq_pool
            .estimate_swap_strict_receive(&0, &1, &swap_amount_out),
        swap_amount_in
    );
    assert_eq!(
        setup
            .liq_pool
            .swap_strict_receive(&user1, &0, &1, &swap_amount_out, &swap_amount_in),
        swap_amount_in
    );
    assert_eq!(
        setup.token1.balance(&setup.liq_pool.address) as u128,
        200_0000000 + swap_amount_in
    );
    assert_eq!(
        setup.token2.balance(&setup.liq_pool.address) as u128,
        200_0000000 - swap_amount_out
    );
    assert_eq!(
        setup.liq_pool.get_reserves(),
        Vec::from_array(
            &setup.env,
            [200_0000000 + swap_amount_in, 200_0000000 - swap_amount_out]
        )
    );
    assert_eq!(
        setup.token1.balance(&user1) as u128,
        800_0000000 - swap_amount_in
    );
    assert_eq!(
        setup.token2.balance(&user1) as u128,
        800_0000000 + swap_amount_out
    );
}

#[test]
fn test_strict_receive_over_max() {
    let setup = Setup::new_with_config(&TestConfig {
        a: 10,
        liq_pool_fee: 30,
        ..TestConfig::default()
    });
    let user1 = Address::generate(&setup.env);

    let token1_admin_client = get_token_admin_client(&setup.env, &setup.token1.address);
    let token2_admin_client = get_token_admin_client(&setup.env, &setup.token2.address);
    token1_admin_client.mint(&user1, &i128::MAX);
    token2_admin_client.mint(&user1, &i128::MAX);
    let desired_amounts = Vec::from_array(&setup.env, [100_0000000, 100_0000000]);
    setup.liq_pool.deposit(&user1, &desired_amounts, &0);

    assert!(setup
        .liq_pool
        .try_estimate_swap_strict_receive(&0, &1, &100_0000000)
        .is_err());
    assert!(setup
        .liq_pool
        .try_swap_strict_receive(&user1, &0, &1, &100_0000000, &100_0000000)
        .is_err());
    assert!(setup
        .liq_pool
        .try_estimate_swap_strict_receive(&0, &1, &99_7000000)
        .is_err());
    assert!(setup
        .liq_pool
        .try_swap_strict_receive(&user1, &0, &1, &99_7000000, &100_0000000)
        .is_err());
    // maximum we're able to buy is `reserve * (1 - fee) - delta`
    assert_eq!(
        setup
            .liq_pool
            .estimate_swap_strict_receive(&0, &1, &99_6999999),
        999995_0045125,
    );
    assert_eq!(
        setup
            .liq_pool
            .swap_strict_receive(&user1, &0, &1, &99_6999999, &999995_0045125),
        999995_0045125
    );
}

#[test]
fn test_happy_flow_different_decimals() {
    // values should not differ from test_happy_flow, only the decimals of the tokens
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token_7 = install_token_wasm_with_decimal(&e, &admin1, 7);
    let token18 = install_token_wasm_with_decimal(&e, &admin2, 18);
    let token7_admin_client = get_token_admin_client(&e, &token_7.address);
    let token18_admin_client = get_token_admin_client(&e, &token18.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token_7.address.clone(), token18.address.clone()]),
        10,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token7_admin_client.mint(&user1, &1000_0000000);
    token18_admin_client.mint(&user1, &1000_000000000000000000);
    assert_eq!(token_7.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token18.balance(&user1) as u128, 1000_000000000000000000);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_000000000000000000]),
        &0,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_000000000000000000);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_000000000000000000]),
        &0,
    );
    assert_eq!(
        plane
            .get(&Vec::from_array(&e, [liqpool.address.clone()]))
            .get_unchecked(0)
            .2,
        Vec::from_array(&e, [200_000000000000000000, 200_000000000000000000,])
    );
    assert_eq!(liqpool.get_virtual_price(), 1_000000000000000000);
    let calculated_amount = liqpool.calc_token_amount(
        &Vec::from_array(&e, [10_0000000, 10_000000000000000000]),
        &true,
    );

    let total_share_token_amount = 400000000000000000000_u128; // share amount after two deposits

    assert_eq!(calculated_amount, total_share_token_amount / 2 / 10);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount
    );
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token_7.balance(&user1) as u128, 800_0000000);
    assert_eq!(token_7.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token18.balance(&user1) as u128, 800_000000000000000000);
    assert_eq!(
        token18.balance(&liqpool.address) as u128,
        200_000000000000000000
    );

    assert_eq!(
        liqpool.estimate_swap(&0, &1, &10_0000000),
        7963726652740971897
    );
    liqpool.swap(&user1, &0, &1, &10_0000000, &1_000000000000000000);

    assert_eq!(token_7.balance(&user1) as u128, 790_0000000);
    assert_eq!(token_7.balance(&liqpool.address) as u128, 210_0000000);
    assert_eq!(token18.balance(&user1) as u128, 807_963726652740971897);
    assert_eq!(
        token18.balance(&liqpool.address) as u128,
        192_036273347259028103
    );

    liqpool.withdraw(
        &user1,
        &(total_share_token_amount / 2),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token_7.balance(&user1) as u128, 895_0000000);
    assert_eq!(token18.balance(&user1) as u128, 903_981863326370485948);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount / 2
    );
    assert_eq!(token_7.balance(&liqpool.address) as u128, 105_0000000);
    assert_eq!(
        token18.balance(&liqpool.address) as u128,
        96_018136673629514052
    );
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);

    liqpool.withdraw(
        &user1,
        &(total_share_token_amount / 2),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token_7.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token18.balance(&user1) as u128, 1000_000000000000000000);
    assert_eq!(token_share.balance(&user1) as u128, 0);
    assert_eq!(token_7.balance(&liqpool.address) as u128, 0);
    assert_eq!(token18.balance(&liqpool.address) as u128, 0);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[test]
fn test_strict_receive_different_decimals() {
    // mirror calculations from test_happy_flow_different_decimals to ensure that swap_strict_receive works as expected
    let setup = Setup::new_with_config(&TestConfig {
        a: 10,
        liq_pool_fee: 2000,
        token_2_decimals: 18,
        ..TestConfig::default()
    });
    let env = setup.env;
    let liqpool = setup.liq_pool;

    let token7_admin_client = get_token_admin_client(&env, &setup.token1.address);
    let token18_admin_client = get_token_admin_client(&env, &setup.token2.address);
    let user1 = Address::generate(&env);

    token7_admin_client.mint(&user1, &1000_0000000);
    token18_admin_client.mint(&user1, &1000_000000000000000000);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&env, [200_0000000, 200_000000000000000000]),
        &0,
    );

    // that's what we expect from test_happy_flow_different_decimals
    assert_eq!(
        liqpool.estimate_swap(&0, &1, &10_0000000),
        7_963726652740971897
    );

    // reverse values
    assert_eq!(
        liqpool.estimate_swap_strict_receive(&0, &1, &7_963726652740971897),
        10_0000000
    );
    assert_eq!(
        liqpool.swap_strict_receive(&user1, &0, &1, &7_963726652740971897, &10_0000000),
        10_0000000
    );
}

#[test]
fn test_events_2_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address
    ];
    tokens.sort();
    let token1 = SorobanTokenClient::new(&e, &tokens[0]);
    let token2 = SorobanTokenClient::new(&e, &tokens[1]);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin);
    let user1 = Address::generate(&e);
    let fee = 30_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [tokens[0].clone(), tokens[1].clone()]),
        10,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    let (amounts, share_amt) =
        liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
    assert_eq!(amounts.get(0).unwrap(), 1000000000);
    assert_eq!(amounts.get(1).unwrap(), 1000000000);
    assert_eq!(share_amt, 2000000000);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "deposit_liquidity"),
                    token1.address.clone(),
                    token2.address.clone(),
                )
                    .into_val(&e),
                (200_0000000_i128, 100_0000000_i128, 100_0000000_i128,).into_val(&e),
            ),
        ]
    );

    assert_eq!(liqpool.swap(&user1, &0, &1, &100, &95), 98);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "trade"),
                    token1.address.clone(),
                    token2.address.clone(),
                    user1.clone()
                )
                    .into_val(&e),
                (100_i128, 98_i128, 1_i128).into_val(&e),
            )
        ]
    );

    let amounts_out = liqpool.withdraw(&user1, &200_0000000, &Vec::from_array(&e, [0, 0]));
    assert_eq!(amounts_out.get(0).unwrap(), 1000000100);
    assert_eq!(amounts_out.get(1).unwrap(), 999999902);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "withdraw_liquidity"),
                    token1.address.clone(),
                    token2.address.clone()
                )
                    .into_val(&e),
                (
                    200_0000000_i128,
                    amounts_out.get(0).unwrap() as i128,
                    amounts_out.get(1).unwrap() as i128
                )
                    .into_val(&e),
            )
        ]
    );
}

#[test]
fn test_events_3_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
    ];
    tokens.sort();
    let token1 = SorobanTokenClient::new(&e, &tokens[0]);
    let token2 = SorobanTokenClient::new(&e, &tokens[1]);
    let token3 = SorobanTokenClient::new(&e, &tokens[2]);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token3_admin_client = get_token_admin_client(&e, &token3.address);
    let token_reward = create_token_contract(&e, &admin);
    let user1 = Address::generate(&e);
    let fee = 30_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [tokens[0].clone(), tokens[1].clone(), tokens[2].clone()],
        ),
        10,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);
    token3_admin_client.mint(&user1, &1000_0000000);

    let (amounts, share_amt) = liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000]),
        &0,
    );
    assert_eq!(amounts.get(0).unwrap(), 1000000000);
    assert_eq!(amounts.get(1).unwrap(), 1000000000);
    assert_eq!(amounts.get(2).unwrap(), 1000000000);
    assert_eq!(share_amt, 3000000000);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "deposit_liquidity"),
                    token1.address.clone(),
                    token2.address.clone(),
                    token3.address.clone(),
                )
                    .into_val(&e),
                (
                    300_0000000_i128,
                    100_0000000_i128,
                    100_0000000_i128,
                    100_0000000_i128,
                )
                    .into_val(&e),
            ),
        ]
    );

    assert_eq!(liqpool.swap(&user1, &0, &1, &100, &95), 98);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "trade"),
                    token1.address.clone(),
                    token2.address.clone(),
                    user1.clone()
                )
                    .into_val(&e),
                (100_i128, 98_i128, 1_i128).into_val(&e),
            )
        ]
    );

    let amounts_out = liqpool.withdraw(&user1, &300_0000000, &Vec::from_array(&e, [0, 0, 0]));
    assert_eq!(amounts_out.get(0).unwrap(), 1000000100);
    assert_eq!(amounts_out.get(1).unwrap(), 999999902);
    assert_eq!(amounts_out.get(2).unwrap(), 1000000000);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "withdraw_liquidity"),
                    token1.address.clone(),
                    token2.address.clone(),
                    token3.address.clone(),
                )
                    .into_val(&e),
                (
                    300_0000000_i128,
                    amounts_out.get(0).unwrap() as i128,
                    amounts_out.get(1).unwrap() as i128,
                    amounts_out.get(2).unwrap() as i128,
                )
                    .into_val(&e),
            )
        ]
    );
}

#[test]
fn test_events_4_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
    ];
    tokens.sort();
    let token1 = SorobanTokenClient::new(&e, &tokens[0]);
    let token2 = SorobanTokenClient::new(&e, &tokens[1]);
    let token3 = SorobanTokenClient::new(&e, &tokens[2]);
    let token4 = SorobanTokenClient::new(&e, &tokens[3]);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token3_admin_client = get_token_admin_client(&e, &token3.address);
    let token4_admin_client = get_token_admin_client(&e, &token4.address);
    let token_reward = create_token_contract(&e, &admin);
    let user1 = Address::generate(&e);
    let fee = 30_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                tokens[0].clone(),
                tokens[1].clone(),
                tokens[2].clone(),
                tokens[3].clone(),
            ],
        ),
        10,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);
    token3_admin_client.mint(&user1, &1000_0000000);
    token4_admin_client.mint(&user1, &1000_0000000);

    let (amounts, share_amt) = liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000, 100_0000000]),
        &0,
    );
    assert_eq!(amounts.get(0).unwrap(), 1000000000);
    assert_eq!(amounts.get(1).unwrap(), 1000000000);
    assert_eq!(amounts.get(2).unwrap(), 1000000000);
    assert_eq!(amounts.get(3).unwrap(), 1000000000);
    assert_eq!(share_amt, 4000000000);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "deposit_liquidity"),
                    token1.address.clone(),
                    token2.address.clone(),
                    token3.address.clone(),
                    token4.address.clone(),
                )
                    .into_val(&e),
                (
                    400_0000000_i128,
                    100_0000000_i128,
                    100_0000000_i128,
                    100_0000000_i128,
                    100_0000000_i128,
                )
                    .into_val(&e),
            ),
        ]
    );

    assert_eq!(liqpool.swap(&user1, &0, &1, &100, &95), 98);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "trade"),
                    token1.address.clone(),
                    token2.address.clone(),
                    user1.clone()
                )
                    .into_val(&e),
                (100_i128, 98_i128, 1_i128).into_val(&e),
            )
        ]
    );

    let amounts_out = liqpool.withdraw(&user1, &400_0000000, &Vec::from_array(&e, [0, 0, 0, 0]));
    assert_eq!(amounts_out.get(0).unwrap(), 1000000100);
    assert_eq!(amounts_out.get(1).unwrap(), 999999902);
    assert_eq!(amounts_out.get(2).unwrap(), 1000000000);
    assert_eq!(amounts_out.get(3).unwrap(), 1000000000);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "withdraw_liquidity"),
                    token1.address.clone(),
                    token2.address.clone(),
                    token3.address.clone(),
                    token4.address.clone(),
                )
                    .into_val(&e),
                (
                    400_0000000_i128,
                    amounts_out.get(0).unwrap() as i128,
                    amounts_out.get(1).unwrap() as i128,
                    amounts_out.get(2).unwrap() as i128,
                    amounts_out.get(3).unwrap() as i128,
                )
                    .into_val(&e),
            )
        ]
    );
}

#[test]
fn test_pool_imbalance_draw_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
    ];
    tokens.sort();
    let token1 = SorobanTokenClient::new(&e, &tokens[0]);
    let token2 = SorobanTokenClient::new(&e, &tokens[1]);
    let token3 = SorobanTokenClient::new(&e, &tokens[2]);
    let token4 = SorobanTokenClient::new(&e, &tokens[3]);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token3_admin_client = get_token_admin_client(&e, &token3.address);
    let token4_admin_client = get_token_admin_client(&e, &token4.address);
    let token_reward = create_token_contract(&e, &admin);
    let user1 = Address::generate(&e);
    let fee = 50_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                tokens[0].clone(),
                tokens[1].clone(),
                tokens[2].clone(),
                tokens[3].clone(),
            ],
        ),
        85,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &8734464);
    token2_admin_client.mint(&user1, &1000000000);
    token3_admin_client.mint(&user1, &789021);
    token4_admin_client.mint(&user1, &789020);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [8734464, 1000000000, 789020, 789020]),
        &0,
    );
    assert_eq!(liqpool.swap(&user1, &2, &1, &1, &0), 567);
}

#[test]
fn test_pool_imbalance_draw_tokens_different_decimals() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);

    let tokens = std::vec![
        install_token_wasm_with_decimal(&e, &admin, 18).address,
        install_token_wasm_with_decimal(&e, &admin, 12).address,
        create_token_contract(&e, &admin).address,
        install_token_wasm_with_decimal(&e, &admin, 4).address,
    ];
    let token1 = SorobanTokenClient::new(&e, &tokens[0]);
    let token2 = SorobanTokenClient::new(&e, &tokens[1]);
    let token3 = SorobanTokenClient::new(&e, &tokens[2]);
    let token4 = SorobanTokenClient::new(&e, &tokens[3]);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token3_admin_client = get_token_admin_client(&e, &token3.address);
    let token4_admin_client = get_token_admin_client(&e, &token4.address);
    let token_reward = create_token_contract(&e, &admin);
    let user1 = Address::generate(&e);
    let fee = 50_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                tokens[0].clone(),
                tokens[1].clone(),
                tokens[2].clone(),
                tokens[3].clone(),
            ],
        ),
        85,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &0_873446400000000000);
    token2_admin_client.mint(&user1, &100_000000000000);
    token3_admin_client.mint(&user1, &(789021 + 1));
    token4_admin_client.mint(&user1, &0_0789);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [0_873446400000000000, 100_000000000000, 789021, 0_0789]),
        &0,
    );
    assert_eq!(liqpool.swap(&user1, &2, &1, &1, &0), 56252658);
    assert_eq!(
        plane
            .get(&Vec::from_array(&e, [liqpool.address.clone()]))
            .get_unchecked(0)
            .1,
        Vec::from_array(&e, [50, 85, 0, 85, 0])
    );
    assert_eq!(
        plane
            .get(&Vec::from_array(&e, [liqpool.address.clone()]))
            .get_unchecked(0)
            .2,
        Vec::from_array(
            &e,
            [
                873446400000000000,
                99999943747342000000,
                78902200000000000,
                78900000000000000,
            ]
        )
    );
}

#[should_panic(expected = "Error(Contract, #2018)")]
#[test]
fn test_pool_zero_swap() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
    ];
    tokens.sort();
    let token1 = SorobanTokenClient::new(&e, &tokens[0]);
    let token2 = SorobanTokenClient::new(&e, &tokens[1]);
    let token3 = SorobanTokenClient::new(&e, &tokens[2]);
    let token4 = SorobanTokenClient::new(&e, &tokens[3]);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token3_admin_client = get_token_admin_client(&e, &token3.address);
    let token4_admin_client = get_token_admin_client(&e, &token4.address);
    let token_reward = create_token_contract(&e, &admin);
    let user1 = Address::generate(&e);
    let fee = 50_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                tokens[0].clone(),
                tokens[1].clone(),
                tokens[2].clone(),
                tokens[3].clone(),
            ],
        ),
        85,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &8734464);
    token2_admin_client.mint(&user1, &1000000000);
    token3_admin_client.mint(&user1, &789020);
    token4_admin_client.mint(&user1, &789020);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [8734464, 1000000000, 789020, 789020]),
        &0,
    );
    liqpool.swap(&user1, &2, &1, &0, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2003)")]
fn test_bad_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        10000,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2004)")]
fn test_zero_initial_deposit() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );
    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [1000_0000000, 0]), &0);
}

#[test]
fn test_zero_deposit_ok() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );
    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [500_0000000, 500_0000000]), &0);
    liqpool.deposit(&user1, &Vec::from_array(&e, [500_0000000, 0]), &0);
}

#[test]
fn test_happy_flow_3_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);
    let admin3 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token3 = create_token_contract(&e, &admin3);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token3_admin_client = get_token_admin_client(&e, &token3.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                token1.address.clone(),
                token2.address.clone(),
                token3.address.clone(),
            ],
        ),
        10,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);
    token3_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token3.balance(&user1) as u128, 1000_0000000);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000]),
        &0,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000]),
        &0,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000); // ???
    let calculated_amount = liqpool.calc_token_amount(
        &Vec::from_array(&e, [10_0000000, 10_0000000, 10_0000000]),
        &true,
    );

    let total_share_token_amount = 600_0000000_u128; // share amount after two deposits

    assert_eq!(calculated_amount, total_share_token_amount / 2 / 10);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount
    );
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token1.balance(&user1) as u128, 800_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token2.balance(&user1) as u128, 800_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);

    liqpool.swap(&user1, &0, &1, &10_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 790_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 210_0000000);
    assert_eq!(token2.balance(&user1) as u128, 807_9637266);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362734);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);

    liqpool.swap(&user1, &2, &0, &20_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 805_9304412);
    assert_eq!(token1.balance(&liqpool.address) as u128, 194_0695588);
    assert_eq!(token2.balance(&user1) as u128, 807_9637266);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362734);
    assert_eq!(token3.balance(&user1) as u128, 780_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 220_0000000);

    liqpool.withdraw(
        &user1,
        &((total_share_token_amount as u128) / 2),
        &Vec::from_array(&e, [0, 0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 902_9652206);
    assert_eq!(token2.balance(&user1) as u128, 903_9818633);
    assert_eq!(token3.balance(&user1) as u128, 890_0000000);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount / 2
    );
    assert_eq!(token1.balance(&liqpool.address) as u128, 97_0347794);
    assert_eq!(token2.balance(&liqpool.address) as u128, 96_0181367);
    assert_eq!(token3.balance(&liqpool.address) as u128, 110_0000000);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);

    liqpool.withdraw(
        &user1,
        &((total_share_token_amount as u128) / 2),
        &Vec::from_array(&e, [0, 0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token3.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 0);
    assert_eq!(token1.balance(&liqpool.address) as u128, 0);
    assert_eq!(token2.balance(&liqpool.address) as u128, 0);
    assert_eq!(token3.balance(&liqpool.address) as u128, 0);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[test]
fn test_happy_flow_4_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);
    let admin3 = Address::generate(&e);
    let admin4 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token3 = create_token_contract(&e, &admin3);
    let token4 = create_token_contract(&e, &admin4);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token3_admin_client = get_token_admin_client(&e, &token3.address);
    let token4_admin_client = get_token_admin_client(&e, &token4.address);

    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                token1.address.clone(),
                token2.address.clone(),
                token3.address.clone(),
                token4.address.clone(),
            ],
        ),
        10,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);
    token3_admin_client.mint(&user1, &1000_0000000);
    token4_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token3.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token4.balance(&user1) as u128, 1000_0000000);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000, 100_0000000]),
        &0,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000, 100_0000000]),
        &0,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000); // ???
    let calculated_amount = liqpool.calc_token_amount(
        &Vec::from_array(&e, [10_0000000, 10_0000000, 10_0000000, 10_0000000]),
        &true,
    );

    let total_share_token_amount = 800_0000000_u128; // share amount after two deposits

    assert_eq!(calculated_amount, total_share_token_amount / 2 / 10);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount
    );
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token1.balance(&user1) as u128, 800_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token2.balance(&user1) as u128, 800_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token4.balance(&user1) as u128, 800_0000000);
    assert_eq!(token4.balance(&liqpool.address) as u128, 200_0000000);

    liqpool.swap(&user1, &0, &1, &10_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 790_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 210_0000000);
    assert_eq!(token2.balance(&user1) as u128, 807_9637266);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362734);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token4.balance(&user1) as u128, 800_0000000);
    assert_eq!(token4.balance(&liqpool.address) as u128, 200_0000000);

    liqpool.swap(&user1, &3, &0, &20_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 805_9304931);
    assert_eq!(token1.balance(&liqpool.address) as u128, 194_0695069);
    assert_eq!(token2.balance(&user1) as u128, 807_9637266);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362734);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token4.balance(&user1) as u128, 780_0000000);
    assert_eq!(token4.balance(&liqpool.address) as u128, 220_0000000);

    liqpool.withdraw(
        &user1,
        &(total_share_token_amount),
        &Vec::from_array(&e, [0, 0, 0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token3.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token4.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 0);
    assert_eq!(token1.balance(&liqpool.address) as u128, 0);
    assert_eq!(token2.balance(&liqpool.address) as u128, 0);
    assert_eq!(token3.balance(&liqpool.address) as u128, 0);
    assert_eq!(token4.balance(&liqpool.address) as u128, 0);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[test]
fn test_withdraw_partial() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let user1 = Address::generate(&e);
    let fee = 0_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        fee as u32,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);

    token2_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);

    let share_token_amount = 200_0000000;
    assert_eq!(token_share.balance(&user1) as u128, share_token_amount);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token1.balance(&user1) as u128, 900_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 100_0000000);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 100_0000000);

    liqpool.swap(&user1, &0, &1, &10_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 890_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 110_0000000);
    assert_eq!(token2.balance(&user1) as u128, 909_9091734 - fee);
    assert_eq!(token2.balance(&liqpool.address) as u128, 90_0908266 + fee);

    liqpool.withdraw(
        &user1,
        &(share_token_amount * 30 / 100),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 923_0000000);
    assert_eq!(token2.balance(&user1) as u128, 936_9364213);
    assert_eq!(
        token_share.balance(&user1) as u128,
        share_token_amount - (share_token_amount * 30 / 100)
    );
    assert_eq!(token1.balance(&liqpool.address) as u128, 77_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 63_0635787);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[test]
fn test_withdraw_one_token() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);

    token2_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);

    let share_token_amount = 200_0000000_u128;
    assert_eq!(token_share.balance(&user1) as u128, share_token_amount);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token1.balance(&user1) as u128, 900_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 100_0000000);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 100_0000000);

    assert_eq!(
        liqpool.withdraw_one_coin(&user1, &100_0000000, &0, &10_0000000),
        Vec::from_array(&e, [91_0435607_u128, 0_u128]),
    );
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "withdraw_liquidity"),
                    token1.address.clone(),
                    token2.address.clone()
                )
                    .into_val(&e),
                (100_0000000_i128, 91_0435607_i128, 0_i128).into_val(&e),
            )
        ]
    );

    assert_eq!(token1.balance(&user1) as u128, 991_0435607);
    assert_eq!(token1.balance(&liqpool.address) as u128, 8_9564393);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 100_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 100_0000000);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[test]
fn test_withdraw_one_token_different_decimals() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token18 = install_token_wasm_with_decimal(&e, &admin1, 18);
    let token7 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    let token18_admin_client = get_token_admin_client(&e, &token18.address);
    let token7_admin_client = get_token_admin_client(&e, &token7.address);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token18.address.clone(), token7.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token18_admin_client.mint(&user1, &1000_000000000000000000);
    assert_eq!(token18.balance(&user1) as u128, 1000_000000000000000000);

    token7_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token7.balance(&user1) as u128, 1000_0000000);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_000000000000000000, 100_0000000]),
        &0,
    );

    let share_token_amount = 200_000000000000000000_u128;
    assert_eq!(token_share.balance(&user1) as u128, share_token_amount);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token18.balance(&user1) as u128, 900_000000000000000000);
    assert_eq!(
        token18.balance(&liqpool.address) as u128,
        100_000000000000000000
    );
    assert_eq!(token7.balance(&user1) as u128, 900_0000000);
    assert_eq!(token7.balance(&liqpool.address) as u128, 100_0000000);

    assert_eq!(
        liqpool.withdraw_one_coin(&user1, &100_000000000000000000, &0, &10_000000000000000000),
        Vec::from_array(&e, [91_043560762610399983_u128, 0_u128]),
    );

    assert_eq!(token18.balance(&user1) as u128, 991_043560762610399983);
    assert_eq!(
        token18.balance(&liqpool.address) as u128,
        8_956439237389600017
    );
    assert_eq!(token7.balance(&user1) as u128, 900_0000000);
    assert_eq!(token7.balance(&liqpool.address) as u128, 100_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 100_000000000000000000);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[test]
fn test_custom_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let user1 = Address::generate(&e);

    token1_admin_client.mint(&user1, &1000000_0000000);
    token2_admin_client.mint(&user1, &1000000_0000000);

    // we're checking fraction against value required to swap 1 token
    for fee_config in [
        (0, 9990916),    // fee = 0%
        (10, 9980925),   // fee = 0.1%
        (30, 9960943),   // fee = 0.3%
        (100, 9891006),  // fee = 1%
        (1000, 8991824), // fee = 10%
        (3000, 6993641), // fee = 30%
    ] {
        let plane = create_plane_contract(&e);
        let liqpool = create_liqpool_contract(
            &e,
            &user1,
            &Address::generate(&e),
            &install_token_wasm(&e),
            &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
            10,
            fee_config.0,
            &token_reward.address,
            &create_token_contract(&e, &Address::generate(&e)).address,
            &create_reward_boost_feed_contract(
                &e,
                &Address::generate(&e),
                &Address::generate(&e),
                &Address::generate(&e),
            )
            .address,
            &plane.address,
        );
        liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
        assert_eq!(liqpool.estimate_swap(&0, &1, &1_0000000), fee_config.1);
        assert_eq!(liqpool.swap(&user1, &0, &1, &1_0000000, &0), fee_config.1);

        // full withdraw & deposit to reset pool reserves
        liqpool.withdraw(
            &user1,
            &(SorobanTokenClient::new(&e, &liqpool.share_id()).balance(&user1) as u128),
            &Vec::from_array(&e, [0, 0]),
        );
        liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
        assert_eq!(liqpool.estimate_swap(&0, &1, &1_0000000), fee_config.1); // re-check swap result didn't change
        assert_eq!(
            liqpool.estimate_swap_strict_receive(&0, &1, &fee_config.1),
            1_0000000
        );
        assert_eq!(
            liqpool.swap_strict_receive(&user1, &0, &1, &fee_config.1, &1_0000000),
            1_0000000
        );
    }
}

#[test]
fn test_deposit_inequal() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [10_0000000, 100_0000000]), &0);
    assert_eq!(token_share.balance(&user1) as u128, 101_8767615);
    assert_eq!(token1.balance(&user1) as u128, 990_0000000);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 10_0000000]), &0);
    assert_eq!(token1.balance(&user1) as u128, 890_0000000);
    assert_eq!(token2.balance(&user1) as u128, 890_0000000);
}

#[test]
fn test_deposit_inequal_different_decimals() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token7 = create_token_contract(&e, &admin2);
    let token18 = install_token_wasm_with_decimal(&e, &admin1, 18);
    let token7_admin_client = get_token_admin_client(&e, &token7.address);
    let token18_admin_client = get_token_admin_client(&e, &token18.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token7.address.clone(), token18.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token7_admin_client.mint(&user1, &1000_0000000);
    token18_admin_client.mint(&user1, &1000_000000000000000000);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [10_0000000, 100_000000000000000000]),
        &0,
    );
    assert_eq!(token_share.balance(&user1) as u128, 101_876761504564086655);
    assert_eq!(token7.balance(&user1) as u128, 990_0000000);
    assert_eq!(token18.balance(&user1) as u128, 900_000000000000000000);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 10_000000000000000000]),
        &0,
    );
    assert_eq!(token7.balance(&user1) as u128, 890_0000000);
    assert_eq!(token18.balance(&user1) as u128, 890_000000000000000000);
}

#[test]
fn test_remove_liquidity_imbalance() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [10_0000000, 100_0000000]), &0);
    assert_eq!(token1.balance(&user1) as u128, 990_0000000);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    let token_share_amount = token_share.balance(&user1) as u128;
    assert_eq!(token_share_amount, 101_8767615);
    liqpool.remove_liquidity_imbalance(
        &user1,
        &Vec::from_array(&e, [0_5000000, 99_0000000]),
        &token_share_amount,
    );
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "withdraw_liquidity"),
                    token1.address.clone(),
                    token2.address.clone()
                )
                    .into_val(&e),
                (
                    (token_share_amount - 9_7635378) as i128,
                    0_5000000_i128,
                    99_0000000_i128
                )
                    .into_val(&e),
            )
        ]
    );
    assert_eq!(token1.balance(&user1) as u128, 990_5000000);
    assert_eq!(token2.balance(&user1) as u128, 999_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 9_5000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 1_0000000);
    assert!((token_share.balance(&user1) as u128) < token_share_amount / 10); // more than 90% of the share were burned
    assert_eq!(token_share.balance(&user1) as u128, 9_7635378); // control exact value
}

#[test]
fn test_remove_liquidity_imbalance_different_decimals() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = install_token_wasm_with_decimal(&e, &admin1, 18);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_000000000000000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [10_000000000000000000, 100_0000000]),
        &0,
    );
    assert_eq!(token1.balance(&user1) as u128, 990_000000000000000000);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    let token_share_amount = token_share.balance(&user1) as u128;
    assert_eq!(token_share_amount, 101_876761504564086655);
    liqpool.remove_liquidity_imbalance(
        &user1,
        &Vec::from_array(&e, [0_500000000000000000, 99_0000000]),
        &token_share_amount,
    );
    assert_eq!(token1.balance(&user1) as u128, 990_500000000000000000);
    assert_eq!(token2.balance(&user1) as u128, 999_0000000);
    assert_eq!(
        token1.balance(&liqpool.address) as u128,
        9_500000000000000000
    );
    assert_eq!(token2.balance(&liqpool.address) as u128, 1_0000000);
    assert!((token_share.balance(&user1) as u128) < token_share_amount / 10); // more than 90% of the share were burned
    assert_eq!(token_share.balance(&user1) as u128, 9_763537957616772036); // control exact value
}

#[test]
fn test_simple_ongoing_reward() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let token_reward_admin_client = get_token_admin_client(&e, &token_reward.address);

    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token_reward_admin_client.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    token1_admin_client.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1) as u128, 1000);

    token2_admin_client.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1) as u128, 1000);

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);

    assert_eq!(token_reward.balance(&user1) as u128, 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1 / 2);
}

#[test]
fn test_simple_ongoing_reward_different_decimals() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token18 = install_token_wasm_with_decimal(&e, &admin1, 18);
    let token7 = create_token_contract(&e, &admin2);
    let token18_admin_client = get_token_admin_client(&e, &token18.address);
    let token7_admin_client = get_token_admin_client(&e, &token7.address);
    let token_reward = create_token_contract(&e, &admin1);
    let token_reward_admin_client = get_token_admin_client(&e, &token_reward.address);

    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token18.address.clone(), token7.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token_reward_admin_client.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    token18_admin_client.mint(&user1, &100000000000000);
    assert_eq!(token18.balance(&user1) as u128, 100000000000000);

    token7_admin_client.mint(&user1, &1000);
    assert_eq!(token7.balance(&user1) as u128, 1000);

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    liqpool.deposit(&user1, &Vec::from_array(&e, [10000000000000, 100]), &0);

    assert_eq!(token_reward.balance(&user1) as u128, 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);
    assert_approx_eq_abs(liqpool.claim(&user1), total_reward_1 / 2, 2);
    assert_approx_eq_abs(token_reward.balance(&user1) as u128, total_reward_1 / 2, 2);
}

#[test]
fn test_simple_reward() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let token_reward_admin_client = get_token_admin_client(&e, &token_reward.address);

    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1) as u128, 1000);

    token2_admin_client.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1) as u128, 1000);

    // 10 seconds. user depositing
    jump(&e, 10);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);

    // 20 seconds. rewards set up for 60 seconds
    jump(&e, 10);
    token_reward_admin_client.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    // 90 seconds. rewards ended.
    jump(&e, 70);
    // calling set rewards config to checkpoint. should be removed
    liqpool.set_rewards_config(&user1, &e.ledger().timestamp().saturating_add(60), &0_u128);

    // 100 seconds. user claim reward
    jump(&e, 10);
    assert_eq!(token_reward.balance(&user1) as u128, 0);
    // full reward should be available to the user
    assert_eq!(liqpool.claim(&user1), total_reward_1);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1);
}

#[test]
fn test_two_users_rewards() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let token_reward_admin_client = get_token_admin_client(&e, &token_reward.address);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token_reward_admin_client.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    for user in [&user1, &user2] {
        token1_admin_client.mint(user, &1000);
        assert_eq!(token1.balance(user) as u128, 1000);

        token2_admin_client.mint(user, &1000);
        assert_eq!(token2.balance(user) as u128, 1000);
    }

    // two users make deposit for equal value. second after 30 seconds after rewards start,
    //  so it gets only 1/4 of total reward
    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);
    jump(&e, 30);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 2);
    liqpool.deposit(&user2, &Vec::from_array(&e, [100, 100]), &0);
    jump(&e, 100);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 4);
    assert_eq!(liqpool.claim(&user2), total_reward_1 / 4);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1 / 4 * 3);
    assert_eq!(token_reward.balance(&user2) as u128, total_reward_1 / 4);
}

#[test]
fn test_boosted_rewards() {
    let setup = Setup::default();
    let env = setup.env;
    let liq_pool = setup.liq_pool;
    let token_reward = setup.token_reward;

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);
    let user3 = Address::generate(&env);

    let token1_admin_client = get_token_admin_client(&env, &setup.token1.address);
    let token2_admin_client = get_token_admin_client(&env, &setup.token2.address);
    let token_reward_admin_client = get_token_admin_client(&env, &token_reward.address);

    for user in [&user1, &user2, &user3] {
        token1_admin_client.mint(user, &1000);
        assert_eq!(setup.token1.balance(user) as u128, 1000);

        token2_admin_client.mint(user, &1000);
        assert_eq!(setup.token2.balance(user) as u128, 1000);
    }

    let reward_boost_token = setup.reward_boost_token.address;
    let locked_token_admin_client = get_token_admin_client(&env, &reward_boost_token);

    token_reward_admin_client.mint(&liq_pool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liq_pool.set_rewards_config(
        &setup.rewards_admin,
        &env.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    // two users make deposit for equal value. second after 30 seconds after rewards start,
    //  so it gets only 1/4 of total reward
    liq_pool.deposit(&user1, &Vec::from_array(&env, [100, 100]), &0);
    jump(&env, 30);
    assert_eq!(liq_pool.claim(&user1), total_reward_1 / 2);

    // instead of simple deposit, second user locks tokens to boost rewards, then deposits
    // second user lock percentage is 50%. this is equilibrium point for 50% shareholder
    locked_token_admin_client.mint(&user2, &10_000_0000000);
    setup
        .reward_boost_feed
        .set_total_supply(&setup.operations_admin, &20_000_0000000);
    liq_pool.deposit(&user2, &Vec::from_array(&env, [100, 100]), &0);

    jump(&env, 10);
    // total effective share now 200 + 200 * 2.5 = 700
    // first user gets ~28% of total reward, second ~72%
    assert_eq!(liq_pool.claim(&user1), total_reward_1 / 6 * 200 / 700);
    assert_eq!(liq_pool.claim(&user2), total_reward_1 / 6 * 500 / 700);

    // third user joins, depositing 50 tokens. no boost yet
    liq_pool.deposit(&user3, &Vec::from_array(&env, [50, 50]), &0);
    let rewards_info = liq_pool.get_rewards_info(&user3);
    assert_eq!(
        rewards_info
            .get(Symbol::new(&env, "working_balance"))
            .unwrap(),
        100
    );
    assert_eq!(
        rewards_info
            .get(Symbol::new(&env, "working_supply"))
            .unwrap(),
        800
    );

    jump(&env, 10);
    // total effective share now 200 + 200 * 2.5 + 100 = 800
    assert_eq!(liq_pool.claim(&user1), total_reward_1 / 6 * 200 / 800);
    assert_eq!(liq_pool.claim(&user2), total_reward_1 / 6 * 500 / 800);
    assert_eq!(liq_pool.claim(&user3), total_reward_1 / 6 * 100 / 800);

    let user3_tokens_to_lock = 1_000_0000000;
    let new_locked_supply = 25_000_0000000;

    // pre-calculate expected boosted rewards for the third user
    let supply = rewards_info.get(symbol_short!("supply")).unwrap() as u128;
    let old_w_balance = rewards_info
        .get(Symbol::new(&env, "working_balance"))
        .unwrap() as u128;
    let old_w_supply = rewards_info
        .get(Symbol::new(&env, "working_supply"))
        .unwrap() as u128;
    let new_w_balance = min(
        old_w_balance + 3 * user3_tokens_to_lock * supply / new_locked_supply / 2,
        old_w_balance * 5 / 2,
    );
    let new_w_supply = old_w_supply + new_w_balance - old_w_balance;
    let total_reward_step3 = total_reward_1 / 6; // total reward for 10 seconds
    let user2_expected_boosted_reward = new_w_balance * total_reward_step3 / new_w_supply;

    // third user locks tokens to boost rewards
    // effective boost is 1.3
    // effective share balance is 100 * 1.3 = 130
    locked_token_admin_client.mint(&user3, &(user3_tokens_to_lock as i128));
    setup
        .reward_boost_feed
        .set_total_supply(&setup.operations_admin, &new_locked_supply);

    // user checkpoints itself to receive boosted rewards by calling get_rewards_info
    // rewards info should be updated
    let new_rewards_info = liq_pool.get_rewards_info(&user3);
    assert_eq!(
        new_rewards_info
            .get(Symbol::new(&env, "working_balance"))
            .unwrap() as u128,
        old_w_balance
    );
    assert_eq!(
        new_rewards_info
            .get(Symbol::new(&env, "working_supply"))
            .unwrap() as u128,
        old_w_supply
    );
    assert_eq!(
        new_rewards_info
            .get(Symbol::new(&env, "new_working_balance"))
            .unwrap() as u128,
        new_w_balance
    );
    assert_eq!(
        new_rewards_info
            .get(Symbol::new(&env, "new_working_supply"))
            .unwrap() as u128,
        new_w_supply
    );
    assert_eq!(
        new_rewards_info
            .get(Symbol::new(&env, "boost_balance"))
            .unwrap() as u128,
        user3_tokens_to_lock
    );
    assert_eq!(
        new_rewards_info
            .get(Symbol::new(&env, "boost_supply"))
            .unwrap() as u128,
        new_locked_supply
    );
    assert_eq!(
        new_rewards_info.get(symbol_short!("supply")).unwrap() as u128,
        supply
    );

    jump(&env, 10);
    // total effective share now 200 + 200 * 2.5 + 130 = 830
    assert_eq!(liq_pool.claim(&user1), total_reward_1 / 6 * 200 / 830);
    assert_eq!(liq_pool.claim(&user2), total_reward_1 / 6 * 500 / 830);
    let user3_claim = liq_pool.claim(&user3);
    assert_eq!(user3_claim, total_reward_1 / 6 * 130 / 830);
    assert_eq!(user3_claim, user2_expected_boosted_reward);

    // total reward is distributed should be distributed to all three users. rounding occurs, so we check with delta
    assert_approx_eq_abs(
        token_reward.balance(&user1) as u128
            + token_reward.balance(&user2) as u128
            + token_reward.balance(&user3) as u128,
        total_reward_1,
        2,
    );
}

#[test]
fn test_lazy_user_rewards() {
    // first user comes as initial liquidity provider and expects to get maximum reward
    //  second user comes at the end makes huge deposit
    //  first should receive almost full reward

    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let token_reward_admin_client = get_token_admin_client(&e, &token_reward.address);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token_reward_admin_client.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    for user in [&user1, &user2] {
        token1_admin_client.mint(user, &1000);
        assert_eq!(token1.balance(user) as u128, 1000);

        token2_admin_client.mint(user, &1000);
        assert_eq!(token2.balance(user) as u128, 1000);
    }

    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);
    jump(&e, 59);
    liqpool.deposit(&user2, &Vec::from_array(&e, [1000, 1000]), &0);
    jump(&e, 100);
    let user1_claim = liqpool.claim(&user1);
    let user2_claim = liqpool.claim(&user2);
    assert_approx_eq_abs(
        user1_claim,
        total_reward_1 * 59 / 60 + total_reward_1 / 1100 * 100 / 60,
        1000,
    );
    assert_approx_eq_abs(user2_claim, total_reward_1 / 1100 * 1000 / 60, 1000);
    assert_approx_eq_abs(token_reward.balance(&user1) as u128, user1_claim, 1000);
    assert_approx_eq_abs(token_reward.balance(&user2) as u128, user2_claim, 1000);
    assert_approx_eq_abs(user1_claim + user2_claim, total_reward_1, 1000);
}

#[test]
#[should_panic(expected = "Error(Contract, #102)")]
fn test_config_rewards_not_admin() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &admin,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                create_token_contract(&e, &admin).address,
                create_token_contract(&e, &admin).address,
            ],
        ),
        10,
        0,
        &(create_token_contract(&e, &admin).address),
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &(create_plane_contract(&e).address),
    );

    liqpool.set_rewards_config(
        &Address::generate(&e),
        &e.ledger().timestamp().saturating_add(60),
        &1,
    );
}

#[test]
fn test_config_rewards_router() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);
    let router = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &admin,
        &router,
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                create_token_contract(&e, &admin).address,
                create_token_contract(&e, &admin).address,
            ],
        ),
        10,
        0,
        &(create_token_contract(&e, &admin).address),
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &(create_plane_contract(&e).address),
    );

    liqpool.set_rewards_config(&router, &e.ledger().timestamp().saturating_add(60), &1);
}

#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_update_fee_too_early() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    liqpool.commit_new_fee(&pool_admin_original, &30);
    assert_eq!(liqpool.get_fee_fraction(), 0);
    liqpool.apply_new_fee(&pool_admin_original);
    jump(&e, 2 * 30 * 86400 - 1);
}

#[test]
fn test_update_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    liqpool.commit_new_fee(&pool_admin_original, &30);
    assert_eq!(liqpool.get_fee_fraction(), 0);

    jump(&e, 2 * 30 * 86400 + 1);
    liqpool.apply_new_fee(&pool_admin_original);
    assert_eq!(liqpool.get_fee_fraction(), 30);
}

#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_transfer_ownership_too_early() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);
    let pool_admin_new = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    liqpool.commit_transfer_ownership(
        &pool_admin_original,
        &symbol_short!("Admin"),
        &pool_admin_new,
    );
    // check admin by calling protected method
    liqpool.stop_ramp_a(&pool_admin_original);
    jump(&e, ADMIN_ACTIONS_DELAY - 1);
    liqpool.apply_transfer_ownership(&pool_admin_original, &symbol_short!("Admin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2906)")]
fn test_transfer_ownership_twice() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);
    let pool_admin_new = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    liqpool.commit_transfer_ownership(
        &pool_admin_original,
        &symbol_short!("Admin"),
        &pool_admin_new,
    );
    liqpool.commit_transfer_ownership(
        &pool_admin_original,
        &symbol_short!("Admin"),
        &pool_admin_new,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_transfer_ownership_not_committed() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    jump(&e, ADMIN_ACTIONS_DELAY + 1);
    liqpool.apply_transfer_ownership(&pool_admin_original, &symbol_short!("Admin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_transfer_ownership_reverted() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);
    let pool_admin_new = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    liqpool.commit_transfer_ownership(
        &pool_admin_original,
        &symbol_short!("Admin"),
        &pool_admin_new,
    );
    // check admin by calling protected method
    liqpool.stop_ramp_a(&pool_admin_original);
    jump(&e, ADMIN_ACTIONS_DELAY + 1);
    liqpool.revert_transfer_ownership(&pool_admin_original, &symbol_short!("Admin"));
    liqpool.apply_transfer_ownership(&pool_admin_original, &symbol_short!("Admin"));
}

#[test]
fn test_transfer_ownership() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);
    let pool_admin_new = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    liqpool.commit_transfer_ownership(
        &pool_admin_original,
        &symbol_short!("Admin"),
        &pool_admin_new,
    );
    // check admin by calling protected method
    liqpool.stop_ramp_a(&pool_admin_original);
    jump(&e, ADMIN_ACTIONS_DELAY + 1);
    liqpool.apply_transfer_ownership(&pool_admin_original, &symbol_short!("Admin"));
    liqpool.stop_ramp_a(&pool_admin_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #2902)")]
fn test_ramp_a_too_early() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    jump(&e, MIN_RAMP_TIME - 1);
    assert_eq!(liqpool.a(), 10);
    liqpool.ramp_a(
        &pool_admin_original,
        &30,
        &e.ledger().timestamp().saturating_add(MIN_RAMP_TIME + 1),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2903)")]
fn test_ramp_a_too_short() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    jump(&e, MIN_RAMP_TIME + 1);
    assert_eq!(liqpool.a(), 10);
    liqpool.ramp_a(
        &pool_admin_original,
        &30,
        &e.ledger().timestamp().saturating_add(MIN_RAMP_TIME - 1),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2905)")]
fn test_ramp_a_too_fast() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    jump(&e, MIN_RAMP_TIME + 1);
    assert_eq!(liqpool.a(), 10);
    liqpool.ramp_a(
        &pool_admin_original,
        &101,
        &e.ledger().timestamp().saturating_add(MIN_RAMP_TIME + 1),
    );
}

#[test]
fn test_ramp_a() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    jump(&e, MIN_RAMP_TIME + 1);
    assert_eq!(liqpool.a(), 10);
    liqpool.ramp_a(
        &pool_admin_original,
        &99,
        &e.ledger().timestamp().saturating_add(MIN_RAMP_TIME + 1),
    );
    jump(&e, MIN_RAMP_TIME / 2 + 1);
    assert_eq!(liqpool.a(), 54);
    jump(&e, MIN_RAMP_TIME);
    assert_eq!(liqpool.a(), 99);
}

#[test]
#[should_panic(expected = "Error(Contract, #2006)")]
fn test_deposit_min_mint() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin = Address::generate(&e);
    let plane = create_plane_contract(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let user1 = Address::generate(&e);
    token1_admin_client.mint(&user1, &i128::MAX);
    token2_admin_client.mint(&user1, &i128::MAX);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [1_000_000_000_0000000, 1_000_000_000_0000000]),
        &0,
    );
    liqpool.deposit(&user1, &Vec::from_array(&e, [1, 1]), &10);
}

#[test]
fn test_deposit_inequal_ok() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin = Address::generate(&e);
    let plane = create_plane_contract(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let user1 = Address::generate(&e);
    token1_admin_client.mint(&user1, &i128::MAX);
    token2_admin_client.mint(&user1, &i128::MAX);

    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());
    assert_eq!(token1.balance(&liqpool.address), 100);
    assert_eq!(token2.balance(&liqpool.address), 100);
    assert_eq!(token_share.balance(&user1), 200);
    liqpool.deposit(&user1, &Vec::from_array(&e, [200, 100]), &0);
    assert_eq!(token1.balance(&liqpool.address), 300);
    assert_eq!(token2.balance(&liqpool.address), 200);
    assert_eq!(token_share.balance(&user1), 499);
}

#[test]
fn test_large_numbers() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin = Address::generate(&e);
    let plane = create_plane_contract(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        85,
        6,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    let user1 = Address::generate(&e);
    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &i128::MAX);
    token2_admin_client.mint(&user1, &i128::MAX);

    let amount_to_deposit = u128::MAX / 1_000_000;
    let desired_amounts = Vec::from_array(&e, [amount_to_deposit, amount_to_deposit]);

    liqpool.deposit(&user1, &desired_amounts, &0);

    // when we deposit equal amounts, we gotta have deposited amount of share tokens
    assert_eq!(token_share.balance(&liqpool.address), 0);
    assert_eq!(
        token1.balance(&user1),
        i128::MAX - amount_to_deposit as i128
    );
    assert_eq!(token1.balance(&liqpool.address), amount_to_deposit as i128);
    assert_eq!(
        token2.balance(&user1),
        i128::MAX - amount_to_deposit as i128
    );
    assert_eq!(token2.balance(&liqpool.address), amount_to_deposit as i128);

    let swap_in = amount_to_deposit / 1_000;
    // swap out shouldn't differ for more than 0.1% since fee is 0.06%
    let expected_swap_result_delta = swap_in / 1000;
    let estimate_swap_result = liqpool.estimate_swap(&0, &1, &swap_in);
    assert_approx_eq_abs(estimate_swap_result, swap_in, expected_swap_result_delta);
    assert_eq!(
        liqpool.swap(&user1, &0, &1, &swap_in, &estimate_swap_result),
        estimate_swap_result
    );

    assert_eq!(
        token1.balance(&user1),
        i128::MAX - amount_to_deposit as i128 - swap_in as i128
    );
    assert_eq!(
        token1.balance(&liqpool.address),
        amount_to_deposit as i128 + swap_in as i128
    );
    assert_eq!(
        token2.balance(&user1),
        i128::MAX - amount_to_deposit as i128 + estimate_swap_result as i128
    );
    assert_eq!(
        token2.balance(&liqpool.address),
        amount_to_deposit as i128 - estimate_swap_result as i128
    );

    let share_amount = token_share.balance(&user1);

    let withdraw_amounts = [
        amount_to_deposit + swap_in,
        amount_to_deposit - estimate_swap_result,
    ];
    liqpool.withdraw(
        &user1,
        &(share_amount as u128),
        &Vec::from_array(&e, withdraw_amounts),
    );

    assert_eq!(token1.balance(&user1), i128::MAX);
    assert_eq!(token2.balance(&user1), i128::MAX);
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token1.balance(&liqpool.address), 0);
    assert_eq!(token2.balance(&liqpool.address), 0);
    assert_eq!(token_share.balance(&liqpool.address), 0);
}

#[test]
fn test_kill_deposit() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &admin,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );
    assert_eq!(liqpool.get_is_killed_deposit(), false);
    assert_eq!(liqpool.get_is_killed_swap(), false);
    assert_eq!(liqpool.get_is_killed_claim(), false);
    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    liqpool.kill_deposit(&admin);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (Symbol::new(&e, "kill_deposit"),).into_val(&e),
                Val::VOID.into_val(&e),
            )
        ]
    );
    assert_eq!(liqpool.get_is_killed_deposit(), true);
    assert_eq!(liqpool.get_is_killed_swap(), false);
    assert_eq!(liqpool.get_is_killed_claim(), false);
    assert_eq!(
        liqpool
            .try_deposit(
                &user1,
                &Vec::from_array(&e, [1000_0000000, 1000_0000000]),
                &0,
            )
            .unwrap_err(),
        Ok(Error::from_contract_error(205))
    );
    liqpool.unkill_deposit(&admin);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (Symbol::new(&e, "unkill_deposit"),).into_val(&e),
                Val::VOID.into_val(&e),
            )
        ]
    );
    assert_eq!(liqpool.get_is_killed_deposit(), false);
    assert_eq!(liqpool.get_is_killed_swap(), false);
    assert_eq!(liqpool.get_is_killed_claim(), false);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [1000_0000000, 1000_0000000]),
        &0,
    );
}

#[test]
fn test_kill_swap() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &admin,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );
    assert_eq!(liqpool.get_is_killed_deposit(), false);
    assert_eq!(liqpool.get_is_killed_swap(), false);
    assert_eq!(liqpool.get_is_killed_claim(), false);
    token1_admin_client.mint(&user1, &10000_0000000);
    token2_admin_client.mint(&user1, &10000_0000000);

    liqpool.kill_swap(&admin);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (Symbol::new(&e, "kill_swap"),).into_val(&e),
                Val::VOID.into_val(&e),
            )
        ]
    );
    assert_eq!(liqpool.get_is_killed_deposit(), false);
    assert_eq!(liqpool.get_is_killed_swap(), true);
    assert_eq!(liqpool.get_is_killed_claim(), false);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [1000_0000000, 1000_0000000]),
        &0,
    );
    assert_eq!(
        liqpool
            .try_swap(&user1, &0, &1, &10_0000000, &0)
            .unwrap_err(),
        Ok(Error::from_contract_error(206))
    );
    liqpool.unkill_swap(&admin);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (Symbol::new(&e, "unkill_swap"),).into_val(&e),
                Val::VOID.into_val(&e),
            )
        ]
    );
    assert_eq!(liqpool.get_is_killed_deposit(), false);
    assert_eq!(liqpool.get_is_killed_swap(), false);
    assert_eq!(liqpool.get_is_killed_claim(), false);
    liqpool.swap(&user1, &0, &1, &10_0000000, &0);
}

#[test]
fn test_kill_claim() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let token_reward_admin_client = get_token_admin_client(&e, &token_reward.address);

    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &admin,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        &token_reward.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    assert_eq!(liqpool.get_is_killed_deposit(), false);
    assert_eq!(liqpool.get_is_killed_swap(), false);
    assert_eq!(liqpool.get_is_killed_claim(), false);

    liqpool.kill_claim(&admin);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (Symbol::new(&e, "kill_claim"),).into_val(&e),
                Val::VOID.into_val(&e),
            )
        ]
    );
    assert_eq!(liqpool.get_is_killed_deposit(), false);
    assert_eq!(liqpool.get_is_killed_swap(), false);
    assert_eq!(liqpool.get_is_killed_claim(), true);

    token1_admin_client.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1) as u128, 1000);

    token2_admin_client.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1) as u128, 1000);

    // 10 seconds. user depositing
    jump(&e, 10);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);

    // 20 seconds. rewards set up for 60 seconds
    jump(&e, 10);
    token_reward_admin_client.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    // 90 seconds. rewards ended.
    jump(&e, 70);

    // 100 seconds. user claim reward
    jump(&e, 10);

    assert_eq!(
        liqpool.try_claim(&user1).unwrap_err(),
        Ok(Error::from_contract_error(207))
    );

    liqpool.unkill_claim(&admin);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (Symbol::new(&e, "unkill_claim"),).into_val(&e),
                Val::VOID.into_val(&e),
            )
        ]
    );
    assert_eq!(liqpool.get_is_killed_deposit(), false);
    assert_eq!(liqpool.get_is_killed_swap(), false);
    assert_eq!(liqpool.get_is_killed_claim(), false);

    assert_eq!(liqpool.claim(&user1), total_reward_1);
}

#[test]
fn test_withdraw_rewards() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin);
    let mut token2 = create_token_contract(&e, &admin);

    let plane = create_plane_contract(&e);

    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward_admin_client = get_token_admin_client(&e, &token1.address);

    let router = Address::generate(&e);

    let liq_pool = create_liqpool_contract(
        &e,
        &admin,
        &router,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        85,
        30,
        &token_reward_admin_client.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );
    let token_share = ShareTokenClient::new(&e, &liq_pool.share_id());

    token1_admin_client.mint(&user1, &100_0000000);
    token2_admin_client.mint(&user1, &100_0000000);
    liq_pool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
    assert_eq!(
        liq_pool.get_reserves(),
        Vec::from_array(&e, [100_0000000, 100_0000000])
    );

    liq_pool.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(100),
        &1_000_0000000,
    );
    token_reward_admin_client.mint(&liq_pool.address, &(1_000_0000000 * 100));
    jump(&e, 100);

    token1_admin_client.mint(&user2, &1_000_0000000);
    token2_admin_client.mint(&user2, &1_000_0000000);
    liq_pool.deposit(
        &user2,
        &Vec::from_array(&e, [1_000_0000000, 1_000_0000000]),
        &0,
    );
    assert_eq!(
        liq_pool.get_reserves(),
        Vec::from_array(&e, [1_100_0000000, 1_100_0000000])
    );

    assert_eq!(
        liq_pool.get_reserves(),
        Vec::from_array(&e, [1_100_0000000, 1_100_0000000])
    );
    assert_eq!(
        token1.balance(&liq_pool.address),
        1_100_0000000 + 1_000_0000000 * 100
    );
    assert_eq!(token2.balance(&liq_pool.address), 1_100_0000000);

    assert_eq!(
        liq_pool.withdraw(
            &user2,
            &(token_share.balance(&user2) as u128),
            &Vec::from_array(&e, [0, 0]),
        ),
        Vec::from_array(&e, [1_000_0000000, 1_000_0000000])
    );
    assert_eq!(
        liq_pool.get_reserves(),
        Vec::from_array(&e, [100_0000000, 100_0000000])
    );
    assert_eq!(
        token1.balance(&liq_pool.address),
        100_0000000 + 1_000_0000000 * 100
    );
    assert_eq!(token2.balance(&liq_pool.address), 100_0000000);
    assert_eq!(token1.balance(&user2), 1_000_0000000);
    assert_eq!(token2.balance(&user2), 1_000_0000000);

    assert_eq!(liq_pool.claim(&user1), 1_000_0000000 * 100);
    assert_eq!(liq_pool.claim(&user2), 0);
}

#[test]
fn test_deposit_rewards() {
    // test pool reserves are not affected by rewards if reward token is one of pool tokens and presented in pool balance
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin);
    let mut token2 = create_token_contract(&e, &admin);

    let plane = create_plane_contract(&e);

    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward_admin_client = SorobanTokenAdminClient::new(&e, &token1.address.clone());

    let router = Address::generate(&e);

    let liq_pool = create_liqpool_contract(
        &e,
        &admin,
        &router,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        85,
        30,
        &token_reward_admin_client.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    liq_pool.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(100),
        &1_000_0000000,
    );
    token_reward_admin_client.mint(&liq_pool.address, &(1_000_0000000 * 100));
    assert_eq!(liq_pool.get_reserves(), Vec::from_array(&e, [0, 0]));

    token1_admin_client.mint(&user1, &100_0000000);
    token2_admin_client.mint(&user1, &100_0000000);
    liq_pool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
    assert_eq!(
        liq_pool.get_reserves(),
        Vec::from_array(&e, [100_0000000, 100_0000000])
    );
}

#[test]
fn test_swap_rewards() {
    // check that swap rewards are calculated correctly if reward token is one of pool tokens
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin);
    let mut token2 = create_token_contract(&e, &admin);

    let plane = create_plane_contract(&e);

    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward_admin_client = SorobanTokenAdminClient::new(&e, &token1.address.clone());

    let router = Address::generate(&e);

    // we compare two pools to check swap in both directions
    let liq_pool1 = create_liqpool_contract(
        &e,
        &admin,
        &router,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        85,
        30,
        &token_reward_admin_client.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );
    let liq_pool2 = create_liqpool_contract(
        &e,
        &admin,
        &router,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        85,
        30,
        &token_reward_admin_client.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );
    token1_admin_client.mint(&user1, &200_0000000);
    token2_admin_client.mint(&user1, &200_0000000);
    liq_pool1.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
    liq_pool2.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
    assert_eq!(
        liq_pool1.get_reserves(),
        Vec::from_array(&e, [100_0000000, 100_0000000])
    );
    assert_eq!(
        liq_pool2.get_reserves(),
        Vec::from_array(&e, [100_0000000, 100_0000000])
    );

    let estimate1_before_rewards = liq_pool1.estimate_swap(&0, &1, &10_0000000);
    let estimate2_before_rewards = liq_pool1.estimate_swap(&1, &0, &10_0000000);
    // swap is balanced, so values should be the same
    assert_eq!(estimate1_before_rewards, estimate2_before_rewards);

    liq_pool1.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(100),
        &1_000_0000000,
    );
    liq_pool2.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(100),
        &1_000_0000000,
    );
    token_reward_admin_client.mint(&liq_pool1.address, &(1_000_0000000 * 100));
    token_reward_admin_client.mint(&liq_pool2.address, &(1_000_0000000 * 100));
    jump(&e, 100);

    let estimate1_after_rewards = liq_pool1.estimate_swap(&0, &1, &10_0000000);
    let estimate2_after_rewards = liq_pool1.estimate_swap(&1, &0, &10_0000000);
    // balances are out of balance, but reserves are balanced.
    assert_eq!(estimate1_after_rewards, estimate2_after_rewards);
    assert_eq!(estimate1_before_rewards, estimate1_after_rewards);

    token1_admin_client.mint(&user2, &10_0000000);
    token2_admin_client.mint(&user2, &10_0000000);
    // in case of disbalance, user may receive much more tokens than he sent as reward is included
    let swap_result1 = liq_pool1.swap(&user2, &0, &1, &10_0000000, &estimate1_after_rewards);
    let swap_result2 = liq_pool2.swap(&user2, &1, &0, &10_0000000, &estimate1_after_rewards);
    assert_eq!(swap_result1, estimate1_after_rewards);
    assert_eq!(swap_result2, estimate1_after_rewards);

    let reserves1 = liq_pool1.get_reserves();

    // check that balance minus rewards is equal to reserves as they should also have fee and it's same for both pools but in different order
    assert_eq!(
        liq_pool1.get_reserves(),
        Vec::from_array(
            &e,
            [
                token1.balance(&liq_pool1.address) as u128 - 1_000_0000000 * 100,
                token2.balance(&liq_pool1.address) as u128
            ]
        )
    );
    // reverse pool1 reserves to check swap in other direction gave same results
    assert_eq!(
        liq_pool2.get_reserves(),
        Vec::from_array(&e, [reserves1.get(1).unwrap(), reserves1.get(0).unwrap()])
    );

    // receive tokens decimals
    assert_eq!(liq_pool1.get_decimals(), vec![&e, 7u32, 7u32]);
    assert_eq!(liq_pool2.get_decimals(), vec![&e, 7u32, 7u32]);
}

#[test]
fn test_decimals_in_swap_pool() {
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let mut token1 = install_token_wasm_with_decimal(&e, &admin, 18u32);
    let mut token2 = install_token_wasm_with_decimal(&e, &admin, 12u32);
    let plane = create_plane_contract(&e);

    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }

    let token_reward_admin_client = SorobanTokenAdminClient::new(&e, &token1.address);
    let router = Address::generate(&e);

    // we compare two pools to check swap in both directions
    let liq_pool1 = create_liqpool_contract(
        &e,
        &admin,
        &router,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        85,
        30,
        &token_reward_admin_client.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    assert_eq!(
        liq_pool1.get_decimals(),
        vec![&e, token1.decimals(), token2.decimals()]
    );
    assert_eq!(
        liq_pool1.get_tokens(),
        vec![&e, token1.address, token2.address]
    );
}

#[test]
fn test_claim_rewards() {
    // test user cannot claim from pool if rewards configured but not distributed
    let e = Env::default();
    e.mock_all_auths();

    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin);
    let mut token2 = create_token_contract(&e, &admin);

    let plane = create_plane_contract(&e);

    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward_admin_client = SorobanTokenAdminClient::new(&e, &token1.address.clone());

    let router = Address::generate(&e);

    let liq_pool = create_liqpool_contract(
        &e,
        &admin,
        &router,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        30,
        &token_reward_admin_client.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &100_0000000);
    token2_admin_client.mint(&user1, &100_0000000);
    liq_pool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
    assert_eq!(
        liq_pool.get_reserves(),
        Vec::from_array(&e, [100_0000000, 100_0000000])
    );

    liq_pool.set_rewards_config(&admin, &e.ledger().timestamp().saturating_add(100), &1000);
    jump(&e, 100);

    assert!(liq_pool.try_claim(&user1).is_err());
    token_reward_admin_client.mint(&liq_pool.address, &(1000 * 100));
    assert_eq!(liq_pool.claim(&user1), 1000 * 100);
}

#[test]
fn test_drain_reward() {
    // test pool reserves are not affected by rewards if reward token is one of pool tokens and presented in pool balance
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);
    let users = [
        Address::generate(&e),
        Address::generate(&e),
        Address::generate(&e),
        Address::generate(&e),
        Address::generate(&e),
    ];

    let mut token1 = create_token_contract(&e, &admin);
    let mut token2 = create_token_contract(&e, &admin);

    let plane = create_plane_contract(&e);

    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward_admin_client = SorobanTokenAdminClient::new(&e, &token1.address.clone());
    for user in &users {
        token1_admin_client.mint(user, &1_000_000_0000000);
        token2_admin_client.mint(user, &1_000_000_0000000);
    }

    let router = Address::generate(&e);

    let liq_pool = create_liqpool_contract(
        &e,
        &admin,
        &router,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        30,
        &token_reward_admin_client.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );
    let token_share = SorobanTokenClient::new(&e, &liq_pool.share_id());

    liq_pool.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(60),
        &10_5000000,
    );
    token_reward_admin_client.mint(&liq_pool.address, &(1_000_0000000 * 100));
    assert_eq!(liq_pool.get_reserves(), Vec::from_array(&e, [0, 0]));

    // 10 seconds passed since config, user depositing
    jump(&e, 10);

    liq_pool.deposit(
        &users[0],
        &Vec::from_array(&e, [1000_0000000, 1000_0000000]),
        &0,
    );
    let (_, lp_amount) = liq_pool.deposit(
        &users[1],
        &Vec::from_array(&e, [100_0000000, 100_0000000]),
        &0,
    );

    jump(&e, 10);

    for i in 2..5 {
        token_share.transfer(&users[i - 1], &users[i], &(lp_amount as i128));
        // liq_pool.get_user_reward(&users[i]);
        // liq_pool.claim(&users[i]);
        liq_pool.deposit(&users[i], &Vec::from_array(&e, [1, 1]), &0);
    }

    jump(&e, 50);
    assert_eq!(liq_pool.claim(&users[4]), 381818182);
    token_share.transfer(&users[4], &users[3], &(lp_amount as i128));
    assert_eq!(liq_pool.claim(&users[3]), 0);
    token_share.transfer(&users[3], &users[2], &(lp_amount as i128));
    assert_eq!(liq_pool.claim(&users[2]), 0);
    token_share.transfer(&users[2], &users[1], &(lp_amount as i128));
    assert_eq!(liq_pool.claim(&users[1]), 95454545);
    assert_eq!(liq_pool.claim(&users[0]), 4772727271);
}

#[test]
fn test_drain_reserves() {
    // test pool reserves are not affected by rewards if reward token is one of pool tokens and presented in pool balance
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&e);
    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);
    let user3 = Address::generate(&e);
    let user4 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin);
    let mut token2 = create_token_contract(&e, &admin);

    let plane = create_plane_contract(&e);

    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
    }
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward_admin_client = SorobanTokenAdminClient::new(&e, &token1.address.clone());

    let router = Address::generate(&e);

    let liq_pool = create_liqpool_contract(
        &e,
        &admin,
        &router,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        30,
        &token_reward_admin_client.address,
        &create_token_contract(&e, &Address::generate(&e)).address,
        &create_reward_boost_feed_contract(
            &e,
            &Address::generate(&e),
            &Address::generate(&e),
            &Address::generate(&e),
        )
        .address,
        &plane.address,
    );

    liq_pool.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(100),
        &1_000_0000000,
    );
    token_reward_admin_client.mint(&liq_pool.address, &(1_000_0000000 * 100));
    assert_eq!(liq_pool.get_reserves(), Vec::from_array(&e, [0, 0]));

    // first user deposits
    token1_admin_client.mint(&user1, &1_000_000_0000000);
    token2_admin_client.mint(&user1, &1_000_000_0000000);
    liq_pool.deposit(
        &user1,
        &Vec::from_array(&e, [1_000_000_0000000, 1_000_000_0000000]),
        &0,
    );

    // first exploiter deposits
    token1_admin_client.mint(&user2, &1_000_000_0000000);
    token2_admin_client.mint(&user2, &1_000_000_0000000);
    let (_, lp_amount) = liq_pool.deposit(
        &user2,
        &Vec::from_array(&e, [300_000_0000000, 300_000_0000000]),
        &0,
    );

    let token_share = SorobanTokenClient::new(&e, &liq_pool.share_id());

    token_share.transfer(&user2, &user3, &(lp_amount as i128));
    liq_pool.claim(&user3);
    token_share.transfer(&user3, &user4, &(lp_amount as i128));
    liq_pool.claim(&user4);

    jump(&e, 100);

    // exploit starts
    assert_eq!(liq_pool.claim(&user4), 230769230769);
    token_share.transfer(&user4, &user3, &(lp_amount as i128));
    assert_eq!(liq_pool.claim(&user3), 0);
    token_share.transfer(&user3, &user2, &(lp_amount as i128));
    assert_eq!(liq_pool.claim(&user2), 0);

    // first user claims
    assert_eq!(liq_pool.claim(&user1), 769230769230);

    // check reserves
    assert_eq!(
        liq_pool.get_reserves(),
        Vec::from_array(&e, [1_300_000_0000000, 1_300_000_0000000])
    );
    assert_eq!(token1.balance(&liq_pool.address), 1_300_000_0000001); // 1 token left on balance because of rounding
    assert_eq!(token2.balance(&liq_pool.address), 1_300_000_0000000);
}

#[test]
fn test_return_unused_reward() {
    let setup = Setup::new_with_config(&TestConfig {
        reward_token_in_pool: false,
        ..TestConfig::default()
    });
    assert_ne!(setup.token1.address, setup.token_reward.address);
    let e = setup.env;
    let admin = setup.admin;
    let liq_pool = setup.liq_pool;
    let router = setup.router;
    let token_1_admin_client = SorobanTokenAdminClient::new(&e, &setup.token1.address.clone());
    let token_2_admin_client = SorobanTokenAdminClient::new(&e, &setup.token2.address.clone());
    let token_reward_admin_client =
        SorobanTokenAdminClient::new(&e, &setup.token_reward.address.clone());
    let user = Address::generate(&e);

    token_1_admin_client.mint(&user, &1000_0000000);
    token_2_admin_client.mint(&user, &1000_0000000);
    liq_pool.deposit(
        &user,
        &Vec::from_array(&e, [1000_0000000, 1000_0000000]),
        &0,
    );

    liq_pool.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(60),
        &1_0000000,
    );
    // pool has configured rewards, but not minted
    assert_eq!(liq_pool.get_unused_reward(), 0);

    token_reward_admin_client.mint(&liq_pool.address, &(1_0000000 * 100));

    // we've configured rewards for 60 seconds, but minted for 100. 40 surplus
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 40);

    // 10 seconds passed
    jump(&e, 10);
    liq_pool.claim(&user);

    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 40);
    assert_eq!(setup.token_reward.balance(&router), 0);
    jump(&e, 10);

    // pool stops rewards on new iteration
    liq_pool.set_rewards_config(&admin, &e.ledger().timestamp().saturating_add(0), &0);
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 80);

    jump(&e, 10);
    // new config iteration. pool got 50 seconds of rewards. 100 - 20 - 50 = 30 unused
    liq_pool.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(50),
        &1_0000000,
    );

    // neither time nor claim should affect unused rewards
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 30);
    jump(&e, 10);
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 30);
    liq_pool.claim(&user);
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 30);
    jump(&e, 10);
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 30);
    assert_eq!(setup.token_reward.balance(&router), 0);
    assert_eq!(liq_pool.return_unused_reward(&admin), 1_0000000 * 30);
    assert_eq!(setup.token_reward.balance(&router), 1_0000000 * 30);
}

#[test]
fn test_return_unused_reward_reward_token_in_pool() {
    let setup = Setup::new_with_config(&TestConfig {
        reward_token_in_pool: true,
        ..TestConfig::default()
    });
    assert_eq!(setup.token1.address, setup.token_reward.address);
    let e = setup.env;
    let admin = setup.admin;
    let liq_pool = setup.liq_pool;
    let router = setup.router;
    let token_1_admin_client = SorobanTokenAdminClient::new(&e, &setup.token1.address.clone());
    let token_2_admin_client = SorobanTokenAdminClient::new(&e, &setup.token2.address.clone());
    let token_reward_admin_client =
        SorobanTokenAdminClient::new(&e, &setup.token_reward.address.clone());
    let user = Address::generate(&e);

    token_1_admin_client.mint(&user, &1000_0000000);
    token_2_admin_client.mint(&user, &1000_0000000);
    liq_pool.deposit(
        &user,
        &Vec::from_array(&e, [1000_0000000, 1000_0000000]),
        &0,
    );

    liq_pool.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(60),
        &1_0000000,
    );
    // pool has configured rewards, but not minted
    assert_eq!(liq_pool.get_unused_reward(), 0);

    token_reward_admin_client.mint(&liq_pool.address, &(1_0000000 * 100));

    // we've configured rewards for 60 seconds, but minted for 100. 40 surplus
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 40);

    // 10 seconds passed
    jump(&e, 10);
    liq_pool.claim(&user);

    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 40);
    assert_eq!(setup.token_reward.balance(&router), 0);
    jump(&e, 10);

    // pool stops rewards on new iteration
    liq_pool.set_rewards_config(&admin, &e.ledger().timestamp().saturating_add(0), &0);
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 80);

    jump(&e, 10);
    // new config iteration. pool got 50 seconds of rewards. 100 - 20 - 50 = 30 unused
    liq_pool.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(50),
        &1_0000000,
    );

    // neither time nor claim should affect unused rewards
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 30);
    jump(&e, 10);
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 30);
    liq_pool.claim(&user);
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 30);
    jump(&e, 10);
    assert_eq!(liq_pool.get_unused_reward(), 1_0000000 * 30);
    assert_eq!(setup.token_reward.balance(&router), 0);
    assert_eq!(liq_pool.return_unused_reward(&admin), 1_0000000 * 30);
    assert_eq!(setup.token_reward.balance(&router), 1_0000000 * 30);
}

#[test]
fn test_kill_deposit_event() {
    let setup = Setup::default();
    let pool = setup.liq_pool;

    pool.kill_deposit(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "kill_deposit"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_kill_swap_event() {
    let setup = Setup::default();
    let pool = setup.liq_pool;

    pool.kill_swap(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "kill_swap"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_kill_claim_event() {
    let setup = Setup::default();
    let pool = setup.liq_pool;

    pool.kill_claim(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "kill_claim"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_unkill_deposit_event() {
    let setup = Setup::default();
    let pool = setup.liq_pool;

    pool.unkill_deposit(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "unkill_deposit"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_unkill_swap_event() {
    let setup = Setup::default();
    let pool = setup.liq_pool;

    pool.unkill_swap(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "unkill_swap"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_unkill_claim_event() {
    let setup = Setup::default();
    let pool = setup.liq_pool;

    pool.unkill_claim(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "unkill_claim"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_set_privileged_addresses_event() {
    let setup = Setup::default();
    let pool = setup.liq_pool;

    pool.set_privileged_addrs(
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
                pool.address.clone(),
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
fn test_set_rewards_config() {
    let setup = Setup::default();
    let pool = setup.liq_pool;

    pool.set_rewards_config(
        &setup.admin.clone(),
        &setup.env.ledger().timestamp().saturating_add(100),
        &1_0000000,
    );

    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "set_rewards_config"),).into_val(&setup.env),
                (
                    setup.env.ledger().timestamp().saturating_add(100),
                    1_0000000_u128,
                )
                    .into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_transfer_ownership_events() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let new_admin = Address::generate(&setup.env);

    pool.commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &new_admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (
                    Symbol::new(&setup.env, "commit_transfer_ownership"),
                    symbol_short!("Admin")
                )
                    .into_val(&setup.env),
                (new_admin.clone(),).into_val(&setup.env),
            ),
        ]
    );

    pool.revert_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (
                    Symbol::new(&setup.env, "revert_transfer_ownership"),
                    symbol_short!("Admin")
                )
                    .into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );

    pool.commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &new_admin);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    pool.apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
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
fn test_ramp_a_events() {
    let setup = Setup::default();
    let pool = setup.liq_pool;

    jump(&setup.env, MIN_RAMP_TIME);
    pool.ramp_a(
        &setup.admin,
        &185,
        &setup.env.ledger().timestamp().saturating_add(MIN_RAMP_TIME),
    );
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "ramp_a"),).into_val(&setup.env),
                (
                    185_u128,
                    setup.env.ledger().timestamp().saturating_add(MIN_RAMP_TIME)
                )
                    .into_val(&setup.env),
            ),
        ]
    );

    jump(&setup.env, MIN_RAMP_TIME / 2);

    pool.stop_ramp_a(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "stop_ramp_a"),).into_val(&setup.env),
                (135_u128,).into_val(&setup.env),
            ),
        ]
    );
    assert_eq!(pool.a(), 135);
}

#[test]
fn test_set_fee_events() {
    let setup = Setup::default();
    let pool = setup.liq_pool;

    pool.commit_new_fee(&setup.admin, &8);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "commit_new_fee"),).into_val(&setup.env),
                (8_u32,).into_val(&setup.env),
            ),
        ]
    );

    pool.revert_new_parameters(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "revert_new_parameters"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );

    pool.commit_new_fee(&setup.admin, &8);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    pool.apply_new_fee(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                pool.address.clone(),
                (Symbol::new(&setup.env, "apply_new_fee"),).into_val(&setup.env),
                (8_u32,).into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_upgrade_events() {
    let setup = Setup::default();
    let contract = setup.liq_pool;
    let new_wasm_hash = install_dummy_wasm(&setup.env);
    let token_new_wasm_hash = install_dummy_wasm(&setup.env);

    contract.commit_upgrade(&setup.admin, &new_wasm_hash, &token_new_wasm_hash);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "commit_upgrade"),).into_val(&setup.env),
                (new_wasm_hash.clone(), token_new_wasm_hash.clone()).into_val(&setup.env),
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

    contract.commit_upgrade(&setup.admin, &new_wasm_hash, &token_new_wasm_hash);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    contract.apply_upgrade(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "apply_upgrade"),).into_val(&setup.env),
                (new_wasm_hash.clone(), token_new_wasm_hash.clone()).into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_emergency_mode_events() {
    let setup = Setup::default();
    let contract = setup.liq_pool;

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
    let contract = setup.liq_pool;
    let token = ShareTokenClient::new(&setup.env, &contract.share_id());

    let new_wasm = install_dummy_wasm(&setup.env);
    let new_token_wasm = install_dummy_wasm(&setup.env);

    assert_eq!(contract.get_emergency_mode(), false);
    assert_ne!(contract.version(), 130);
    assert_ne!(token.version(), 130);
    contract.set_emergency_mode(&setup.emergency_admin, &true);

    contract.commit_upgrade(&setup.admin, &new_wasm, &new_token_wasm);
    contract.apply_upgrade(&setup.admin);

    assert_eq!(contract.version(), 130);
    assert_eq!(token.version(), 130);
}

#[test]
fn test_regular_upgrade_token() {
    let setup = Setup::default();
    let contract = setup.liq_pool;
    let token = ShareTokenClient::new(&setup.env, &contract.share_id());

    let token_wasm = setup
        .env
        .deployer()
        .upload_contract_wasm(token_share::token::WASM);
    let new_wasm = install_dummy_wasm(&setup.env);

    // dummy wasm has version 130, everything else has greater version
    assert_eq!(contract.get_emergency_mode(), false);
    assert_ne!(contract.version(), 130);
    assert_ne!(token.version(), 130);

    contract.commit_upgrade(&setup.admin, &new_wasm, &token_wasm);
    assert!(contract.try_apply_upgrade(&setup.admin).is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert_eq!(
        contract.apply_upgrade(&setup.admin),
        (new_wasm.clone(), token_wasm.clone())
    );

    assert_eq!(contract.version(), 130);
    assert_ne!(token.version(), 130);
}

#[test]
fn test_regular_upgrade_pool() {
    let setup = Setup::default();
    let contract = setup.liq_pool;
    let token = ShareTokenClient::new(&setup.env, &contract.share_id());

    let new_wasm = install_dummy_wasm(&setup.env);
    let new_token_wasm = install_dummy_wasm(&setup.env);

    // dummy wasm has version 130, everything else has greater version
    assert_eq!(contract.get_emergency_mode(), false);
    assert_ne!(contract.version(), 130);
    assert_ne!(token.version(), 130);

    contract.commit_upgrade(&setup.admin, &new_wasm, &new_token_wasm);
    assert!(contract.try_apply_upgrade(&setup.admin).is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert_eq!(
        contract.apply_upgrade(&setup.admin),
        (new_wasm.clone(), new_token_wasm.clone())
    );

    assert_eq!(contract.version(), 130);
    assert_eq!(token.version(), 130);
}

#[test]
fn test_claim_event() {
    let setup = Setup::default();
    let liq_pool = setup.liq_pool;
    let token_1_admin_client =
        SorobanTokenAdminClient::new(&setup.env, &setup.token1.address.clone());
    let token_2_admin_client =
        SorobanTokenAdminClient::new(&setup.env, &setup.token2.address.clone());
    let token_reward_admin_client =
        SorobanTokenAdminClient::new(&setup.env, &setup.token_reward.address.clone());

    let user = Address::generate(&setup.env);

    token_1_admin_client.mint(&user, &1000);
    token_2_admin_client.mint(&user, &1000);
    liq_pool.deposit(&user, &Vec::from_array(&setup.env, [1000, 1000]), &0);
    token_reward_admin_client.mint(&liq_pool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 70;
    liq_pool.set_rewards_config(
        &setup.admin,
        &setup.env.ledger().timestamp().saturating_add(70),
        &reward_1_tps,
    );
    jump(&setup.env, 70);
    liq_pool.claim(&user);

    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                liq_pool.address.clone(),
                (
                    Symbol::new(&setup.env, "claim_reward"),
                    setup.token_reward.address.clone(),
                    user.clone(),
                )
                    .into_val(&setup.env),
                (total_reward_1 as i128,).into_val(&setup.env),
            ),
        ]
    );
}
