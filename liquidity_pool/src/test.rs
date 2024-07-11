#![cfg(test)]
extern crate std;

use crate::testutils::{
    create_liqpool_contract, create_plane_contract, create_token_contract, install_token_wasm,
    jump, Setup, TestConfig,
};
use soroban_sdk::testutils::{AuthorizedFunction, AuthorizedInvocation, Events};
use soroban_sdk::{testutils::Address as _, vec, Address, Env, Error, IntoVal, Symbol, Vec};
use token_share::Client;
use utils::test_utils::assert_approx_eq_abs;

#[test]
fn test() {
    let Setup {
        env: e,
        router: _router,
        users,
        token1,
        token2,
        token_reward,
        token_share,
        liq_pool,
        plane: _plane,
    } = Setup::new_with_config(&TestConfig {
        mint_to_user: i128::MAX,
        ..TestConfig::default()
    });
    let user1 = users[0].clone();
    let reward_1_tps = 10_5000000_u128;
    let reward_2_tps = 20_0000000_u128;
    let reward_3_tps = 6_0000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    let amount_to_deposit = 100_0000000;
    let desired_amounts = Vec::from_array(&e, [amount_to_deposit, amount_to_deposit]);

    liq_pool.deposit(&user1, &desired_amounts, &0);
    assert_eq!(
        e.auths()[0],
        (
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    liq_pool.address.clone(),
                    Symbol::new(&e, "deposit"),
                    Vec::from_array(
                        &e,
                        [
                            user1.to_val(),
                            desired_amounts.to_val(),
                            0_u128.into_val(&e)
                        ]
                    ),
                )),
                sub_invocations: std::vec![
                    AuthorizedInvocation {
                        function: AuthorizedFunction::Contract((
                            token1.address.clone(),
                            Symbol::new(&e, "transfer"),
                            Vec::from_array(
                                &e,
                                [
                                    user1.to_val(),
                                    liq_pool.address.to_val(),
                                    (desired_amounts.get(0).unwrap() as i128).into_val(&e),
                                ]
                            ),
                        )),
                        sub_invocations: std::vec![],
                    },
                    AuthorizedInvocation {
                        function: AuthorizedFunction::Contract((
                            token2.address.clone(),
                            Symbol::new(&e, "transfer"),
                            Vec::from_array(
                                &e,
                                [
                                    user1.to_val(),
                                    liq_pool.address.to_val(),
                                    (desired_amounts.get(1).unwrap() as i128).into_val(&e),
                                ]
                            ),
                        )),
                        sub_invocations: std::vec![],
                    }
                ],
            }
        )
    );

    assert_eq!(token_reward.balance(&user1), 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);
    assert_eq!(liq_pool.claim(&user1), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1 / 2);
    // 60 seconds more passed. full reward was available though half already claimed
    jump(&e, 60);
    assert_eq!(liq_pool.claim(&user1), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1);

    // more rewards added with different configs
    let total_reward_2 = reward_2_tps * 100;
    liq_pool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(100),
        &reward_2_tps,
    );
    jump(&e, 105);
    let total_reward_3 = reward_3_tps * 50;
    liq_pool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(50),
        &reward_3_tps,
    );
    jump(&e, 500);
    // two rewards available for the user
    assert_eq!(liq_pool.claim(&user1), total_reward_2 + total_reward_3);
    assert_eq!(
        token_reward.balance(&user1) as u128,
        total_reward_1 + total_reward_2 + total_reward_3
    );

    // when we deposit equal amounts, we gotta have deposited amount of share tokens
    let expected_share_amount = amount_to_deposit as i128;
    assert_eq!(token_share.balance(&user1), expected_share_amount);
    assert_eq!(token_share.balance(&liq_pool.address), 0);
    assert_eq!(
        token1.balance(&user1),
        i128::MAX - amount_to_deposit as i128
    );
    assert_eq!(token1.balance(&liq_pool.address), amount_to_deposit as i128);
    assert_eq!(
        token2.balance(&user1),
        i128::MAX - amount_to_deposit as i128
    );
    assert_eq!(token2.balance(&liq_pool.address), amount_to_deposit as i128);

    let swap_in_amount = 1_0000000_u128;
    let expected_swap_result = 9871287_u128;

    assert_eq!(
        liq_pool.estimate_swap(&0, &1, &swap_in_amount),
        expected_swap_result
    );
    assert_eq!(
        liq_pool.swap(&user1, &0, &1, &swap_in_amount, &expected_swap_result),
        expected_swap_result
    );
    assert_eq!(
        e.auths()[0],
        (
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    liq_pool.address.clone(),
                    Symbol::new(&e, "swap"),
                    (&user1, 0_u32, 1_u32, swap_in_amount, expected_swap_result).into_val(&e)
                )),
                sub_invocations: std::vec![AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        token1.address.clone(),
                        Symbol::new(&e, "transfer"),
                        Vec::from_array(
                            &e,
                            [
                                user1.to_val(),
                                liq_pool.address.to_val(),
                                (swap_in_amount as i128).into_val(&e),
                            ]
                        ),
                    )),
                    sub_invocations: std::vec![],
                },],
            }
        )
    );

    assert_eq!(
        token1.balance(&user1),
        i128::MAX - amount_to_deposit as i128 - swap_in_amount as i128
    );
    assert_eq!(
        token1.balance(&liq_pool.address),
        amount_to_deposit as i128 + swap_in_amount as i128
    );
    assert_eq!(
        token2.balance(&user1),
        i128::MAX - amount_to_deposit as i128 + expected_swap_result as i128
    );
    assert_eq!(
        token2.balance(&liq_pool.address),
        amount_to_deposit as i128 - expected_swap_result as i128
    );

    let withdraw_amounts = [
        amount_to_deposit + swap_in_amount,
        amount_to_deposit - expected_swap_result,
    ];
    liq_pool.withdraw(
        &user1,
        &(expected_share_amount as u128),
        &Vec::from_array(&e, withdraw_amounts),
    );
    assert_eq!(
        e.auths()[0],
        (
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    liq_pool.address.clone(),
                    Symbol::new(&e, "withdraw"),
                    Vec::from_array(
                        &e,
                        [
                            user1.clone().into_val(&e),
                            (expected_share_amount as u128).into_val(&e),
                            Vec::from_array(&e, withdraw_amounts).into_val(&e)
                        ],
                    )
                )),
                sub_invocations: std::vec![AuthorizedInvocation {
                    function: AuthorizedFunction::Contract((
                        token_share.address.clone(),
                        Symbol::new(&e, "transfer"),
                        Vec::from_array(
                            &e,
                            [
                                user1.to_val(),
                                liq_pool.address.to_val(),
                                (amount_to_deposit as i128).into_val(&e),
                            ]
                        ),
                    )),
                    sub_invocations: std::vec![],
                }],
            }
        )
    );

    jump(&e, 600);
    assert_eq!(liq_pool.claim(&user1), 0);
    assert_eq!(
        token_reward.balance(&user1) as u128,
        total_reward_1 + total_reward_2 + total_reward_3
    );

    assert_eq!(token1.balance(&user1), i128::MAX);
    assert_eq!(token2.balance(&user1), i128::MAX);
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token1.balance(&liq_pool.address), 0);
    assert_eq!(token2.balance(&liq_pool.address), 0);
    assert_eq!(token_share.balance(&liq_pool.address), 0);
}

#[test]
fn test_events() {
    let Setup {
        env: e,
        router: _router,
        users,
        token1,
        token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::new_with_config(&TestConfig {
        mint_to_user: i128::MAX,
        ..TestConfig::default()
    });
    let user1 = users[0].clone();
    let amount_to_deposit = 100_0000000;
    let desired_amounts = Vec::from_array(&e, [amount_to_deposit, amount_to_deposit]);

    liq_pool.deposit(&user1, &desired_amounts, &0);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liq_pool.address.clone(),
                (
                    Symbol::new(&e, "deposit_liquidity"),
                    token1.address.clone(),
                    token2.address.clone()
                )
                    .into_val(&e),
                (
                    amount_to_deposit as i128,
                    amount_to_deposit as i128,
                    amount_to_deposit as i128
                )
                    .into_val(&e),
            ),
        ]
    );

    assert_eq!(liq_pool.swap(&user1, &0, &1, &100, &97), 98);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liq_pool.address.clone(),
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

    let amounts_out = liq_pool.withdraw(&user1, &amount_to_deposit, &Vec::from_array(&e, [0, 0]));
    assert_eq!(amounts_out.get(0).unwrap(), 1000000100);
    assert_eq!(amounts_out.get(1).unwrap(), 999999902);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liq_pool.address.clone(),
                (
                    Symbol::new(&e, "withdraw_liquidity"),
                    token1.address.clone(),
                    token2.address.clone()
                )
                    .into_val(&e),
                (
                    amount_to_deposit as i128,
                    amounts_out.get(0).unwrap() as i128,
                    amounts_out.get(1).unwrap() as i128
                )
                    .into_val(&e),
            )
        ]
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2006)")]
fn test_deposit_min_mint() {
    let Setup {
        env: e,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::new_with_config(&TestConfig {
        mint_to_user: i128::MAX,
        ..TestConfig::default()
    });
    let user1 = users[0].clone();

    liq_pool.deposit(
        &user1,
        &Vec::from_array(&e, [1_000_000_000_000_0000000, 1_000_000_000_000_0000000]),
        &0,
    );
    liq_pool.deposit(&user1, &Vec::from_array(&e, [1, 1]), &10);
}

#[test]
#[should_panic(expected = "Error(Contract, #2004)")]
fn test_zero_initial_deposit() {
    let Setup {
        env: e,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();
    let user1 = users[0].clone();
    liq_pool.deposit(&user1, &Vec::from_array(&e, [100, 0]), &0);
}

#[test]
fn test_zero_deposit_ok() {
    let Setup {
        env: e,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();
    let user1 = users[0].clone();
    liq_pool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);
    liq_pool.deposit(&user1, &Vec::from_array(&e, [100, 0]), &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #201)")]
fn initialize_already_initialized() {
    let setup = Setup::default();

    let users = Setup::generate_random_users(&setup.env, 3);
    let token1 = create_token_contract(&setup.env, &users[1]);
    let token2 = create_token_contract(&setup.env, &users[2]);

    setup.liq_pool.initialize(
        &users[0],
        &users[0],
        &install_token_wasm(&setup.env),
        &Vec::from_array(&setup.env, [token1.address.clone(), token2.address.clone()]),
        &10_u32,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #202)")]
fn initialize_already_initialized_plane() {
    let setup = Setup::default();

    let users = Setup::generate_random_users(&setup.env, 3);
    let token1 = create_token_contract(&setup.env, &users[1]);
    let token2 = create_token_contract(&setup.env, &users[2]);

    setup.liq_pool.initialize_all(
        &users[0],
        &users[0],
        &install_token_wasm(&setup.env),
        &Vec::from_array(&setup.env, [token1.address.clone(), token2.address.clone()]),
        &10_u32,
        &token1.address,
        &setup.plane.address,
    );
}

#[test]
fn test_custom_fee() {
    let config = TestConfig {
        mint_to_user: 1000000_0000000,
        ..TestConfig::default()
    };
    let setup = Setup::new_with_config(&config);

    // we're checking fraction against output for 1 token
    for fee_config in [
        (0, 9900990_u128),    // 0%
        (10, 9891089_u128),   // 0.1%
        (30, 9871287_u128),   // 0.3%
        (100, 9801980_u128),  // 1%
        (1000, 8910891_u128), // 10%
        (3000, 6930693_u128), // 30%
        (9900, 99009_u128),   // 99%
        (9999, 990_u128),     // 99.99% - maximum fee
    ] {
        let liqpool = create_liqpool_contract(
            &setup.env,
            &Address::generate(&setup.env),
            &setup.users[0],
            &install_token_wasm(&setup.env),
            &Vec::from_array(
                &setup.env,
                [setup.token1.address.clone(), setup.token2.address.clone()],
            ),
            &setup.token_reward.address,
            fee_config.0, // ten percent
            &setup.plane.address,
        );
        liqpool.deposit(
            &setup.users[0],
            &Vec::from_array(&setup.env, [100_0000000, 100_0000000]),
            &0,
        );
        assert_eq!(liqpool.estimate_swap(&1, &0, &1_0000000), fee_config.1);
        assert_eq!(
            liqpool.swap(&setup.users[0], &1, &0, &1_0000000, &0),
            fee_config.1
        );
    }
}

#[test]
fn test_simple_ongoing_reward() {
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();
    let total_reward_1 = TestConfig::default().reward_tps * 60;
    assert_eq!(liq_pool.get_total_configured_reward(), total_reward_1);
    assert_eq!(liq_pool.get_total_accumulated_reward(), 0);
    assert_eq!(liq_pool.get_total_claimed_reward(), 0);

    // 10 seconds passed since config, user depositing
    jump(&env, 10);

    assert_eq!(liq_pool.get_total_configured_reward(), total_reward_1);
    assert_eq!(
        liq_pool.get_total_accumulated_reward(),
        TestConfig::default().reward_tps * 10
    );
    assert_eq!(liq_pool.get_total_claimed_reward(), 0);

    liq_pool.deposit(&users[0], &Vec::from_array(&env, [100, 100]), &0);

    assert_eq!(token_reward.balance(&users[0]), 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&env, 30);

    assert_eq!(liq_pool.get_total_configured_reward(), total_reward_1);
    assert_eq!(
        liq_pool.get_total_accumulated_reward(),
        TestConfig::default().reward_tps * 40
    );
    assert_eq!(liq_pool.get_total_claimed_reward(), 0);

    assert_eq!(liq_pool.claim(&users[0]), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&users[0]) as u128, total_reward_1 / 2);

    assert_eq!(liq_pool.get_total_configured_reward(), total_reward_1);
    assert_eq!(
        liq_pool.get_total_accumulated_reward(),
        TestConfig::default().reward_tps * 40
    );
    assert_eq!(
        liq_pool.get_total_claimed_reward(),
        TestConfig::default().reward_tps * 30
    );

    // 40 seconds passed, reward config ended
    //  5/6 of the reward is available for the user since he has missed first 10 seconds
    jump(&env, 40);

    assert_eq!(liq_pool.get_total_configured_reward(), total_reward_1);
    assert_eq!(liq_pool.get_total_accumulated_reward(), total_reward_1);
    assert_eq!(
        liq_pool.get_total_claimed_reward(),
        TestConfig::default().reward_tps * 30
    );

    assert_eq!(liq_pool.claim(&users[0]), total_reward_1 * 2 / 6);
    assert_eq!(
        token_reward.balance(&users[0]) as u128,
        total_reward_1 * 5 / 6
    );

    assert_eq!(liq_pool.get_total_configured_reward(), total_reward_1);
    assert_eq!(liq_pool.get_total_accumulated_reward(), total_reward_1);
    assert_eq!(
        liq_pool.get_total_claimed_reward(),
        TestConfig::default().reward_tps * 50
    );
}

#[test]
fn test_estimate_ongoing_reward() {
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();

    // 10 seconds passed since config, user depositing
    jump(&env, 10);
    liq_pool.deposit(&users[0], &Vec::from_array(&env, [100, 100]), &0);

    assert_eq!(token_reward.balance(&users[0]), 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&env, 30);
    let total_reward_1 = TestConfig::default().reward_tps * 60;
    assert_eq!(liq_pool.get_user_reward(&users[0]), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&users[0]) as u128, 0);
}

#[test]
fn test_simple_reward() {
    let setup = Setup::setup(&TestConfig::default());
    setup.mint_tokens_for_users(TestConfig::default().mint_to_user);
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = setup;

    // 10 seconds. user depositing
    jump(&env, 10);
    liq_pool.deposit(&users[0], &Vec::from_array(&env, [100, 100]), &0);

    // 20 seconds. rewards set up for 60 seconds
    jump(&env, 10);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liq_pool.set_rewards_config(
        &users[0],
        &env.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    // 90 seconds. rewards ended.
    jump(&env, 70);
    // calling set rewards config to checkpoint. should be removed
    liq_pool.set_rewards_config(
        &users[0],
        &env.ledger().timestamp().saturating_add(60),
        &0_u128,
    );

    // 100 seconds. user claim reward
    jump(&env, 10);
    assert_eq!(token_reward.balance(&users[0]), 0);
    // full reward should be available to the user
    assert_eq!(liq_pool.claim(&users[0]), total_reward_1);
    assert_eq!(token_reward.balance(&users[0]) as u128, total_reward_1);
}

#[test]
fn test_two_users_rewards() {
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();

    let total_reward_1 = &TestConfig::default().reward_tps * 60;

    // two users make deposit for equal value. second after 30 seconds after rewards start,
    //  so it gets only 1/4 of total reward
    liq_pool.deposit(&users[0], &Vec::from_array(&env, [100, 100]), &0);
    jump(&env, 30);
    assert_eq!(liq_pool.claim(&users[0]), total_reward_1 / 2);
    liq_pool.deposit(&users[1], &Vec::from_array(&env, [100, 100]), &0);
    jump(&env, 100);
    assert_eq!(liq_pool.claim(&users[0]), total_reward_1 / 4);
    assert_eq!(liq_pool.claim(&users[1]), total_reward_1 / 4);
    assert_eq!(
        token_reward.balance(&users[0]) as u128,
        total_reward_1 / 4 * 3
    );
    assert_eq!(token_reward.balance(&users[1]) as u128, total_reward_1 / 4);
}

#[test]
fn test_lazy_user_rewards() {
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();

    let total_reward_1 = &TestConfig::default().reward_tps * 60;

    liq_pool.deposit(&users[0], &Vec::from_array(&env, [100, 100]), &0);
    jump(&env, 59);
    liq_pool.deposit(&users[1], &Vec::from_array(&env, [1000, 1000]), &0);
    jump(&env, 100);
    let user1_claim = liq_pool.claim(&users[0]);
    let user2_claim = liq_pool.claim(&users[1]);
    assert_approx_eq_abs(
        user1_claim,
        total_reward_1 * 59 / 60 + total_reward_1 / 1100 * 100 / 60,
        1000,
    );
    assert_approx_eq_abs(user2_claim, total_reward_1 / 1100 * 1000 / 60, 1000);
    assert_approx_eq_abs(token_reward.balance(&users[0]) as u128, user1_claim, 1000);
    assert_approx_eq_abs(token_reward.balance(&users[1]) as u128, user2_claim, 1000);
    assert_approx_eq_abs(user1_claim + user2_claim, total_reward_1, 1000);
}

#[test]
fn test_rewards_disable_before_expiration() {
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::new_with_config(&TestConfig {
        users_count: 3,
        reward_tps: 0,
        ..TestConfig::default()
    });

    // user 1 has 10% of total reward
    liq_pool.deposit(&users[0], &Vec::from_array(&env, [900, 900]), &0);
    liq_pool.deposit(&users[1], &Vec::from_array(&env, [100, 100]), &0);

    jump(&env, 10);
    let admin = users[0].clone();
    let tps = 1_0000000;
    // admin sets rewards distribution a bit in the future from the expected point
    liq_pool.set_rewards_config(&admin, &env.ledger().timestamp().saturating_add(100), &tps);

    // user 2 enters. now user 1 gets 5% of total reward, user 2 receives 50%
    jump(&env, 20);
    liq_pool.deposit(&users[2], &Vec::from_array(&env, [1000, 1000]), &0);

    jump(&env, 10);
    liq_pool.withdraw(&users[2], &1000, &Vec::from_array(&env, [1000, 1000]));

    // before config expiration, admin decides to stop as it's time to reward other pools
    jump(&env, 50);
    liq_pool.set_rewards_config(&admin, &env.ledger().timestamp().saturating_add(10), &0);

    // user decides to claim in far future
    jump(&env, 1000);
    assert_eq!(
        liq_pool.claim(&users[1]),
        tps * 20 / 10 + tps * 10 / 20 + tps * 50 / 10
    );
    assert_eq!(liq_pool.claim(&users[2]), tps * 10 / 2);
}

#[test]
fn test_rewards_disable_after_expiration() {
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::new_with_config(&TestConfig {
        reward_tps: 0,
        ..TestConfig::default()
    });

    // user 1 has 10% of total reward
    liq_pool.deposit(&users[0], &Vec::from_array(&env, [900, 900]), &0);
    liq_pool.deposit(&users[1], &Vec::from_array(&env, [100, 100]), &0);

    jump(&env, 10);
    let admin = users[0].clone();
    let tps = 1_0000000;
    // admin sets rewards distribution, then decides to stop rewards after expiration
    liq_pool.set_rewards_config(&admin, &env.ledger().timestamp().saturating_add(100), &tps);
    jump(&env, 150);
    liq_pool.set_rewards_config(&admin, &env.ledger().timestamp().saturating_add(100), &0);

    // user decides to claim in far future
    jump(&env, 1000);
    assert_eq!(liq_pool.claim(&users[1]), tps * 100 / 10);
}

#[test]
fn test_rewards_set_new_after_expiration() {
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::new_with_config(&TestConfig {
        reward_tps: 0,
        ..TestConfig::default()
    });

    // user 1 has 10% of total reward
    liq_pool.deposit(&users[0], &Vec::from_array(&env, [900, 900]), &0);
    liq_pool.deposit(&users[1], &Vec::from_array(&env, [100, 100]), &0);

    jump(&env, 10);
    let admin = users[0].clone();
    let tps_1 = 1_0000000;
    let tps_2 = 10000;
    // admin configures first rewards distribution, then it ends and admin sets new one which also expires
    liq_pool.set_rewards_config(
        &admin,
        &env.ledger().timestamp().saturating_add(100),
        &tps_1,
    );
    jump(&env, 150);
    liq_pool.set_rewards_config(
        &admin,
        &env.ledger().timestamp().saturating_add(100),
        &tps_2,
    );

    // user decides to claim in far future
    jump(&env, 1000);
    assert_eq!(
        liq_pool.claim(&users[1]),
        tps_1 * 100 / 10 + tps_2 * 100 / 10
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #702)")]
fn test_rewards_same_expiration_time() {
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();

    jump(&env, 10);
    liq_pool.set_rewards_config(&users[0], &env.ledger().timestamp().saturating_add(100), &1);
    jump(&env, 10);
    liq_pool.set_rewards_config(&users[0], &env.ledger().timestamp().saturating_add(90), &2);
}

#[test]
#[should_panic(expected = "Error(Contract, #701)")]
fn test_rewards_past() {
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();

    jump(&env, 10);
    let original_expiration_time = env.ledger().timestamp().saturating_add(100);
    liq_pool.set_rewards_config(&users[0], &original_expiration_time, &1);
    jump(&env, 1000);
    liq_pool.set_rewards_config(&users[0], &original_expiration_time.saturating_add(90), &2);
}

fn test_rewards_many_users(iterations_to_simulate: u32) {
    // first user comes as initial liquidity provider
    //  many users come
    //  user does withdraw

    let Setup {
        env,
        router: _router,
        users,
        token1,
        token2,
        token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::new_with_config(&TestConfig {
        users_count: 100,
        ..TestConfig::default()
    });

    let admin = users[0].clone();
    let first_user = Address::generate(&env);

    for i in 0..101 {
        let user = match i {
            0 => &first_user,
            val => &users[val - 1],
        };
        token1.mint(user, &1_000_000_000_000_000_000_000);
        token2.mint(user, &1_000_000_000_000_000_000_000);
    }

    token_reward.mint(&liq_pool.address, &1_000_000_000_000_0000000);

    let reward_1_tps = 10_5000000_u128;
    liq_pool.set_rewards_config(
        &admin,
        &env.ledger()
            .timestamp()
            .saturating_add((iterations_to_simulate * 2 + 110).into()),
        &reward_1_tps,
    );
    jump(&env, 10);

    // we have this because of last jump(100)
    let mut expected_reward = 100 * reward_1_tps / iterations_to_simulate as u128;
    for i in 0..iterations_to_simulate as u128 {
        expected_reward += reward_1_tps / (i + 1);
    }

    liq_pool.deposit(
        &first_user,
        &Vec::from_array(&env, [1_000_000_000_000_0000000, 1_000_000_000_000_0000000]),
        &0,
    );
    jump(&env, 1);

    for i in 1..iterations_to_simulate as usize {
        let user = &users[i % 10];
        liq_pool.deposit(
            user,
            &Vec::from_array(&env, [1_000_000_000_000_0000000, 1_000_000_000_000_0000000]),
            &0,
        );
        jump(&env, 1);
    }

    jump(&env, 100);
    env.budget().reset_default();
    let user1_claim = liq_pool.claim(&first_user);
    env.budget().print();
    assert_approx_eq_abs(user1_claim, expected_reward, 10000); // small loss because of rounding is fine
}

#[test]
fn test_deposit_inequal_return_change() {
    let Setup {
        env: e,
        router: _router,
        users,
        token1,
        token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();
    let user1 = users[0].clone();
    liq_pool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);
    assert_eq!(token1.balance(&liq_pool.address), 100);
    assert_eq!(token2.balance(&liq_pool.address), 100);
    liq_pool.deposit(&user1, &Vec::from_array(&e, [200, 100]), &0);
    assert_eq!(token1.balance(&liq_pool.address), 200);
    assert_eq!(token2.balance(&liq_pool.address), 200);
}

#[test]
fn test_rewards_1k() {
    test_rewards_many_users(1_000);
}

#[cfg(feature = "slow_tests")]
#[test]
fn test_rewards_50k() {
    test_rewards_many_users(50_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #102)")]
fn test_config_rewards_not_admin() {
    let setup = Setup::setup(&TestConfig::default());
    setup.mint_tokens_for_users(TestConfig::default().mint_to_user);
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = setup;

    liq_pool.set_rewards_config(
        &users[1],
        &env.ledger().timestamp().saturating_add(60),
        &10_5000000_u128,
    );
}

#[test]
fn test_config_rewards_router() {
    let setup = Setup::setup(&TestConfig::default());
    setup.mint_tokens_for_users(TestConfig::default().mint_to_user);
    let Setup {
        env,
        router,
        users: _users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = setup;

    liq_pool.set_rewards_config(
        &router,
        &env.ledger().timestamp().saturating_add(60),
        &10_5000000_u128,
    );
}

#[should_panic(expected = "Error(Contract, #2018)")]
#[test]
fn test_zero_swap() {
    let Setup {
        env: e,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::new_with_config(&TestConfig {
        mint_to_user: i128::MAX,
        ..TestConfig::default()
    });
    let user1 = users[0].clone();
    let amount_to_deposit = 1_0000000;
    let desired_amounts = Vec::from_array(&e, [amount_to_deposit, amount_to_deposit]);

    liq_pool.deposit(&user1, &desired_amounts, &0);
    liq_pool.swap(&user1, &0, &1, &0, &0);
}

#[test]
fn test_large_numbers() {
    let Setup {
        env: e,
        router: _router,
        users,
        token1,
        token2,
        token_reward: _token_reward,
        token_share,
        liq_pool,
        plane: _plane,
    } = Setup::new_with_config(&TestConfig {
        mint_to_user: i128::MAX,
        ..TestConfig::default()
    });
    let user1 = users[0].clone();
    let amount_to_deposit = u128::MAX / 1_000_000;
    let desired_amounts = Vec::from_array(&e, [amount_to_deposit, amount_to_deposit]);

    liq_pool.deposit(&user1, &desired_amounts, &0);

    // when we deposit equal amounts, we gotta have deposited amount of share tokens
    let expected_share_amount = amount_to_deposit as i128;
    assert_eq!(token_share.balance(&user1), expected_share_amount);
    assert_eq!(token_share.balance(&liq_pool.address), 0);
    assert_eq!(
        token1.balance(&user1),
        i128::MAX - amount_to_deposit as i128
    );
    assert_eq!(token1.balance(&liq_pool.address), amount_to_deposit as i128);
    assert_eq!(
        token2.balance(&user1),
        i128::MAX - amount_to_deposit as i128
    );
    assert_eq!(token2.balance(&liq_pool.address), amount_to_deposit as i128);

    let swap_in = amount_to_deposit / 1_000;
    // swap out shouldn't differ for more than 0.4% since fee is 0.3%
    let expected_swap_result_delta = swap_in / 250;
    let estimate_swap_result = liq_pool.estimate_swap(&0, &1, &swap_in);
    assert_approx_eq_abs(estimate_swap_result, swap_in, expected_swap_result_delta);
    assert_eq!(
        liq_pool.swap(&user1, &0, &1, &swap_in, &estimate_swap_result),
        estimate_swap_result
    );

    assert_eq!(
        token1.balance(&user1),
        i128::MAX - amount_to_deposit as i128 - swap_in as i128
    );
    assert_eq!(
        token1.balance(&liq_pool.address),
        amount_to_deposit as i128 + swap_in as i128
    );
    assert_eq!(
        token2.balance(&user1),
        i128::MAX - amount_to_deposit as i128 + estimate_swap_result as i128
    );
    assert_eq!(
        token2.balance(&liq_pool.address),
        amount_to_deposit as i128 - estimate_swap_result as i128
    );

    let withdraw_amounts = [
        amount_to_deposit + swap_in,
        amount_to_deposit - estimate_swap_result,
    ];
    liq_pool.withdraw(
        &user1,
        &(expected_share_amount as u128),
        &Vec::from_array(&e, withdraw_amounts),
    );

    assert_eq!(token1.balance(&user1), i128::MAX);
    assert_eq!(token2.balance(&user1), i128::MAX);
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token1.balance(&liq_pool.address), 0);
    assert_eq!(token2.balance(&liq_pool.address), 0);
    assert_eq!(token_share.balance(&liq_pool.address), 0);
}

#[test]
fn test_swap_killed() {
    let Setup {
        env: e,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::new_with_config(&TestConfig {
        mint_to_user: i128::MAX,
        ..TestConfig::default()
    });

    assert_eq!(liq_pool.get_is_killed_deposit(), false);
    assert_eq!(liq_pool.get_is_killed_swap(), false);
    assert_eq!(liq_pool.get_is_killed_claim(), false);

    let admin = users[0].clone();

    liq_pool.kill_swap(&admin);
    assert_eq!(liq_pool.get_is_killed_deposit(), false);
    assert_eq!(liq_pool.get_is_killed_swap(), true);
    assert_eq!(liq_pool.get_is_killed_claim(), false);

    let user1 = users[1].clone();
    let amount_to_deposit = 1_0000000;
    let desired_amounts = Vec::from_array(&e, [amount_to_deposit, amount_to_deposit]);

    liq_pool.deposit(&user1, &desired_amounts, &0);

    assert_eq!(
        liq_pool.try_swap(&user1, &0, &1, &100, &0).unwrap_err(),
        Ok(Error::from_contract_error(206))
    );

    liq_pool.unkill_swap(&admin);
    assert_eq!(liq_pool.get_is_killed_deposit(), false);
    assert_eq!(liq_pool.get_is_killed_swap(), false);
    assert_eq!(liq_pool.get_is_killed_claim(), false);

    liq_pool.swap(&user1, &0, &1, &100, &0);
}

#[test]
fn test_deposit_killed() {
    let Setup {
        env: e,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::new_with_config(&TestConfig {
        mint_to_user: i128::MAX,
        ..TestConfig::default()
    });

    assert_eq!(liq_pool.get_is_killed_deposit(), false);
    assert_eq!(liq_pool.get_is_killed_swap(), false);
    assert_eq!(liq_pool.get_is_killed_claim(), false);

    let admin = users[0].clone();

    liq_pool.kill_deposit(&admin);
    assert_eq!(liq_pool.get_is_killed_deposit(), true);
    assert_eq!(liq_pool.get_is_killed_swap(), false);
    assert_eq!(liq_pool.get_is_killed_claim(), false);

    let user1 = users[1].clone();
    let amount_to_deposit = 1_0000000;
    let desired_amounts = Vec::from_array(&e, [amount_to_deposit, amount_to_deposit]);

    assert_eq!(
        liq_pool
            .try_deposit(&user1, &desired_amounts, &0)
            .unwrap_err(),
        Ok(Error::from_contract_error(205))
    );

    liq_pool.unkill_deposit(&admin);
    assert_eq!(liq_pool.get_is_killed_deposit(), false);
    assert_eq!(liq_pool.get_is_killed_swap(), false);
    assert_eq!(liq_pool.get_is_killed_claim(), false);

    liq_pool.deposit(&user1, &desired_amounts, &0);
}

#[test]
fn test_claim_killed() {
    let setup = Setup::setup(&TestConfig::default());
    setup.mint_tokens_for_users(TestConfig::default().mint_to_user);
    let Setup {
        env,
        router: _router,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = setup;
    assert_eq!(liq_pool.get_is_killed_deposit(), false);
    assert_eq!(liq_pool.get_is_killed_swap(), false);
    assert_eq!(liq_pool.get_is_killed_claim(), false);

    liq_pool.kill_claim(&users[0]);
    assert_eq!(liq_pool.get_is_killed_deposit(), false);
    assert_eq!(liq_pool.get_is_killed_swap(), false);
    assert_eq!(liq_pool.get_is_killed_claim(), true);

    // 10 seconds. user depositing
    jump(&env, 10);
    liq_pool.deposit(&users[1], &Vec::from_array(&env, [100, 100]), &0);

    // 20 seconds. rewards set up for 60 seconds
    jump(&env, 10);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liq_pool.set_rewards_config(
        &users[0],
        &env.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    // 90 seconds. rewards ended.
    jump(&env, 70);

    // 100 seconds. user claim reward
    jump(&env, 10);

    assert_eq!(
        liq_pool.try_claim(&users[1]).unwrap_err(),
        Ok(Error::from_contract_error(207))
    );
    liq_pool.unkill_claim(&users[0]);
    assert_eq!(liq_pool.get_is_killed_deposit(), false);
    assert_eq!(liq_pool.get_is_killed_swap(), false);
    assert_eq!(liq_pool.get_is_killed_claim(), false);
    assert_eq!(liq_pool.claim(&users[1]), total_reward_1);
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
    let token_reward = Client::new(&e, &token1.address.clone());

    let router = Address::generate(&e);

    let liq_pool = create_liqpool_contract(
        &e,
        &admin,
        &router,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        &token_reward.address,
        30,
        &plane.address,
    );
    let token_share = Client::new(&e, &liq_pool.share_id());

    token1.mint(&user1, &100_0000000);
    token2.mint(&user1, &100_0000000);
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
    token_reward.mint(&liq_pool.address, &(1_000_0000000 * 100));
    jump(&e, 100);

    token1.mint(&user2, &1_000_0000000);
    token2.mint(&user2, &1_000_0000000);
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

    liq_pool.withdraw(
        &user2,
        &(token_share.balance(&user2) as u128),
        &Vec::from_array(&e, [0, 0]),
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
