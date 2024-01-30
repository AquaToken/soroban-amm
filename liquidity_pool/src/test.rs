#![cfg(test)]
extern crate std;

use crate::testutils::{
    create_liqpool_contract, create_token_contract, install_token_wasm, jump, Setup, TestConfig,
};
use soroban_sdk::testutils::{AuthorizedFunction, AuthorizedInvocation};
use soroban_sdk::{testutils::Address as _, Address, IntoVal, Symbol, Vec};
use utils::test_utils::assert_approx_eq_abs;

#[test]
fn test() {
    let Setup {
        env: e,
        users,
        token1,
        token2,
        token_reward,
        token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();
    let user1 = users[0].clone();
    let reward_1_tps = 10_5000000_u128;
    let reward_2_tps = 20_0000000_u128;
    let reward_3_tps = 6_0000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    let desired_amounts = Vec::from_array(&e, [100, 100]);

    liq_pool.deposit(&user1, &desired_amounts);
    assert_eq!(
        e.auths()[0],
        (
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    liq_pool.address.clone(),
                    Symbol::new(&e, "deposit"),
                    Vec::from_array(&e, [user1.to_val(), desired_amounts.to_val()]),
                )),
                sub_invocations: std::vec![],
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

    assert_eq!(token_share.balance(&user1), 100);
    assert_eq!(token_share.balance(&liq_pool.address), 0);
    assert_eq!(token1.balance(&user1), 900);
    assert_eq!(token1.balance(&liq_pool.address), 100);
    assert_eq!(token2.balance(&user1), 900);
    assert_eq!(token2.balance(&liq_pool.address), 100);

    assert_eq!(liq_pool.estimate_swap(&0, &1, &97), 49);
    assert_eq!(liq_pool.swap(&user1, &0, &1, &97_u128, &49_u128), 49);
    assert_eq!(
        e.auths()[0],
        (
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    liq_pool.address.clone(),
                    Symbol::new(&e, "swap"),
                    (&user1, 0_u32, 1_u32, 97_u128, 49_u128).into_val(&e)
                )),
                sub_invocations: std::vec![],
            }
        )
    );

    assert_eq!(token1.balance(&user1), 803);
    assert_eq!(token1.balance(&liq_pool.address), 197);
    assert_eq!(token2.balance(&user1), 949);
    assert_eq!(token2.balance(&liq_pool.address), 51);

    token_share.approve(&user1, &liq_pool.address, &100, &99999);

    liq_pool.withdraw(&user1, &100_u128, &Vec::from_array(&e, [197_u128, 51_u128]));
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
                            100_u128.into_val(&e),
                            Vec::from_array(&e, [197_u128, 51_u128]).into_val(&e)
                        ],
                    )
                )),
                sub_invocations: std::vec![],
            }
        )
    );

    jump(&e, 600);
    assert_eq!(liq_pool.claim(&user1), 0);
    assert_eq!(
        token_reward.balance(&user1) as u128,
        total_reward_1 + total_reward_2 + total_reward_3
    );

    assert_eq!(token1.balance(&user1), 1000);
    assert_eq!(token2.balance(&user1), 1000);
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token1.balance(&liq_pool.address), 0);
    assert_eq!(token2.balance(&liq_pool.address), 0);
    assert_eq!(token_share.balance(&liq_pool.address), 0);
}

#[test]
#[should_panic(expected = "initial deposit requires all coins")]
fn test_zero_initial_deposit() {
    let Setup {
        env: e,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();
    let user1 = users[0].clone();
    liq_pool.deposit(&user1, &Vec::from_array(&e, [100, 0]));
}

#[test]
fn test_zero_deposit_ok() {
    let Setup {
        env: e,
        users,
        token1: _token1,
        token2: _token2,
        token_reward: _token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();
    let user1 = users[0].clone();
    liq_pool.deposit(&user1, &Vec::from_array(&e, [100, 100]));
    liq_pool.deposit(&user1, &Vec::from_array(&e, [100, 0]));
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
        &install_token_wasm(&setup.env),
        &Vec::from_array(&setup.env, [token1.address.clone(), token2.address.clone()]),
        &10_u32,
        &token1.address,
        &setup.liq_pool.address,
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

    // we're checking fraction against value required to swap 1 token
    for fee_config in [
        (0, 1_0101011_u128),        // 0%
        (10, 1_0111122_u128),       // 0.1%
        (30, 1_0131405_u128),       // 0.3%
        (100, 1_0203041_u128),      // 1%
        (1000, 1_1223345_u128),     // 10%
        (3000, 1_4430015_u128),     // 30%
        (9900, 101_0101011_u128),   // 99%
        (9999, 10101_0101011_u128), // 99.99% - maximum fee
    ] {
        let liqpool = create_liqpool_contract(
            &setup.env,
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
        setup
            .token1
            .approve(&setup.users[0], &liqpool.address, &100000_0000000, &99999);
        setup
            .token2
            .approve(&setup.users[0], &liqpool.address, &100000_0000000, &99999);
        liqpool.deposit(
            &setup.users[0],
            &Vec::from_array(&setup.env, [100_0000000, 100_0000000]),
        );
        assert_eq!(liqpool.estimate_swap(&1, &0, &fee_config.1), 1_0000000);
        assert_eq!(
            liqpool.swap(&setup.users[0], &1, &0, &fee_config.1, &0),
            1_0000000
        );
    }
}

#[test]
fn test_simple_ongoing_reward() {
    let Setup {
        env,
        users,
        token1: _token1,
        token2: _token2,
        token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();
    let total_reward_1 = TestConfig::default().reward_tps * 60;

    // 10 seconds passed since config, user depositing
    jump(&env, 10);
    liq_pool.deposit(&users[0], &Vec::from_array(&env, [100, 100]));

    assert_eq!(token_reward.balance(&users[0]), 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&env, 30);
    assert_eq!(liq_pool.claim(&users[0]), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&users[0]) as u128, total_reward_1 / 2);
}

#[test]
fn test_estimate_ongoing_reward() {
    let Setup {
        env,
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
    liq_pool.deposit(&users[0], &Vec::from_array(&env, [100, 100]));

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
    setup.mint_tokens_for_users(&TestConfig::default().mint_to_user);
    let Setup {
        env,
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
    liq_pool.deposit(&users[0], &Vec::from_array(&env, [100, 100]));

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
    // assert_eq!(token_reward.balance(&users[0]) as u128, total_reward_1);
}

#[test]
fn test_two_users_rewards() {
    let Setup {
        env,
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
    liq_pool.deposit(&users[0], &Vec::from_array(&env, [100, 100]));
    jump(&env, 30);
    assert_eq!(liq_pool.claim(&users[0]), total_reward_1 / 2);
    liq_pool.deposit(&users[1], &Vec::from_array(&env, [100, 100]));
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
        users,
        token1: _token1,
        token2: _token2,
        token_reward,
        token_share: _token_share,
        liq_pool,
        plane: _plane,
    } = Setup::default();

    let total_reward_1 = &TestConfig::default().reward_tps * 60;

    liq_pool.deposit(&users[0], &Vec::from_array(&env, [100, 100]));
    jump(&env, 59);
    liq_pool.deposit(&users[1], &Vec::from_array(&env, [1000, 1000]));
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

fn test_rewards_many_users(iterations_to_simulate: u32) {
    // first user comes as initial liquidity provider
    //  many users come
    //  user does withdraw

    let Setup {
        env,
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
        token1.mint(user, &1_000_000_000);
        token2.mint(user, &1_000_000_000);
        token1.approve(user, &liq_pool.address, &1_000_000_000, &99999);
        token2.approve(user, &liq_pool.address, &1_000_000_000, &99999);
    }

    token_reward.mint(&liq_pool.address, &1_000_000_000_000_0000000);
    token_reward.approve(
        &liq_pool.address,
        &liq_pool.address,
        &1_000_000_000_000_0000000,
        &99999,
    );

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

    liq_pool.deposit(&first_user, &Vec::from_array(&env, [1000, 1000]));
    jump(&env, 1);

    for i in 1..iterations_to_simulate as usize {
        let user = &users[i % 10];
        liq_pool.deposit(user, &Vec::from_array(&env, [1000, 1000]));
        jump(&env, 1);
    }

    jump(&env, 100);
    env.budget().reset_default();
    env.budget().reset_tracker();
    let user1_claim = liq_pool.claim(&first_user);
    env.budget().print();
    assert_eq!(user1_claim, expected_reward);
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
