#![cfg(test)]
extern crate std;

use crate::testutils::{deploy_rewards_gauge, get_token_admin_client, Setup};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Map, Symbol, Vec, U256};
use utils::test_utils::jump;

#[test]
fn test_swap_empty_pool() {
    let setup = Setup::default();
    setup.mint_user_tokens(10_0000000, 0);

    assert_eq!(setup.pool.estimate_swap(&0, &1, &10_0000000), 0);
    assert_eq!(setup.pool.swap(&setup.user, &0, &1, &10_0000000, &0), 0);

    assert_eq!(setup.token0.balance(&setup.user), 10_0000000);
    assert_eq!(setup.token1.balance(&setup.user), 0);
}

#[test]
fn test_router_happy_flow() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    let initial_user_0 = setup.token0.balance(&setup.user) as u128;
    let initial_user_1 = setup.token1.balance(&setup.user) as u128;

    let desired = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let estimated_shares = setup.pool.estimate_deposit(&desired);
    let (amounts, minted_shares) = setup.pool.deposit(&setup.user, &desired, &0);
    assert_eq!(minted_shares, estimated_shares);
    assert_eq!(setup.pool.get_total_shares(), minted_shares);
    assert_eq!(setup.pool.get_user_shares(&setup.user), minted_shares);

    let spent0 = amounts.get_unchecked(0);
    let spent1 = amounts.get_unchecked(1);
    assert_eq!(
        setup.token0.balance(&setup.user) as u128,
        initial_user_0 - spent0
    );
    assert_eq!(
        setup.token1.balance(&setup.user) as u128,
        initial_user_1 - spent1
    );

    let amount_in = 10_0000000u128;
    let estimated_out = setup.pool.estimate_swap(&0, &1, &amount_in);
    let out = setup.pool.swap(&setup.user, &0, &1, &amount_in, &0);
    assert_eq!(out, estimated_out);
    assert_eq!(
        setup.token0.balance(&setup.user) as u128,
        initial_user_0 - spent0 - amount_in
    );
    assert_eq!(
        setup.token1.balance(&setup.user) as u128,
        initial_user_1 - spent1 + out
    );

    let withdrawn = setup.pool.withdraw(
        &setup.user,
        &minted_shares,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    );
    assert_eq!(setup.pool.get_total_shares(), 0);
    assert_eq!(setup.pool.get_user_shares(&setup.user), 0);

    let destination = Address::generate(&setup.env);
    let claimed = setup.pool.claim_protocol_fees(&setup.admin, &destination);
    assert_eq!(
        setup.token0.balance(&destination) as u128,
        claimed.get_unchecked(0)
    );
    assert_eq!(
        setup.token1.balance(&destination) as u128,
        claimed.get_unchecked(1)
    );

    assert_eq!(
        setup.token0.balance(&setup.user) as u128,
        initial_user_0 - spent0 - amount_in + withdrawn.get_unchecked(0)
    );
    assert_eq!(
        setup.token1.balance(&setup.user) as u128,
        initial_user_1 - spent1 + out + withdrawn.get_unchecked(1)
    );
}

#[test]
fn test_strict_receive_matches_estimate() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    let desired = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    let (deposited, _) = setup.pool.deposit(&setup.user, &desired, &0);

    let initial_user_0 = setup.token0.balance(&setup.user) as u128;
    let initial_user_1 = setup.token1.balance(&setup.user) as u128;

    let out_amount = 1_0000000u128;
    let quoted_in = setup.pool.estimate_swap_strict_receive(&0, &1, &out_amount);
    let amount_in = setup
        .pool
        .swap_strict_receive(&setup.user, &0, &1, &out_amount, &quoted_in);
    assert_eq!(amount_in, quoted_in);

    assert_eq!(
        setup.token0.balance(&setup.user) as u128,
        initial_user_0 - amount_in
    );
    assert_eq!(
        setup.token1.balance(&setup.user) as u128,
        initial_user_1 + out_amount
    );
    assert_eq!(
        setup.token0.balance(&setup.pool.address) as u128,
        deposited.get_unchecked(0) + amount_in
    );
    assert_eq!(
        setup.token1.balance(&setup.pool.address) as u128,
        deposited.get_unchecked(1) - out_amount
    );
}

#[test]
fn test_public_deposit_position_updates_position_tick_and_bitmap() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);

    setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &1_0000000);

    let position = setup.pool.get_position(&setup.user, &-10, &10);
    assert_eq!(position.liquidity, 1_0000000);
    assert_eq!(position.tokens_owed_0, 0);
    assert_eq!(position.tokens_owed_1, 0);

    let lower = setup.pool.ticks(&-10);
    assert!(lower.initialized);
    assert_eq!(lower.liquidity_gross, 1_0000000);
    assert_eq!(lower.liquidity_net, 1_0000000);

    let upper = setup.pool.ticks(&10);
    assert!(upper.initialized);
    assert_eq!(upper.liquidity_gross, 1_0000000);
    assert_eq!(upper.liquidity_net, -1_0000000);

    let zero = U256::from_u32(&setup.env, 0);
    assert_ne!(setup.pool.tick_bitmap(&-1), zero);
    assert_ne!(setup.pool.tick_bitmap(&0), zero);
}

#[test]
#[should_panic(expected = "Error(Contract, #205)")]
fn test_deposit_killed() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);

    setup.pool.kill_deposit(&setup.admin);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]),
        &0,
    );
}

#[test]
fn test_boosted_native_rewards_and_gauge() {
    let setup = Setup::default();
    let user1 = setup.user.clone();
    let user2 = Address::generate(&setup.env);

    let deposit_amount = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let user_funds = 1_000_0000000i128;
    setup.mint_user_tokens(user_funds, user_funds);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&user2, &user_funds);
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&user2, &user_funds);

    setup.pool.initialize_boost_config(
        &setup.reward_boost_token.address,
        &setup.reward_boost_feed.address,
    );
    setup
        .pool
        .initialize_rewards_config(&setup.reward_token.address);

    setup.pool.deposit(&user1, &deposit_amount, &0);

    let reward_tps = 2_100u128;
    let reward_duration = 60u64;
    let total_reward = reward_tps * reward_duration as u128;
    let reward_expired_at = setup.env.ledger().timestamp() + reward_duration;

    get_token_admin_client(&setup.env, &setup.reward_token.address)
        .mint(&setup.pool.address, &(total_reward as i128));
    setup
        .pool
        .set_rewards_config(&setup.admin, &reward_expired_at, &reward_tps);

    let gauge = deploy_rewards_gauge(&setup.env, &setup.pool.address, &setup.reward_token.address);
    setup.pool.gauges_add(&setup.admin, &gauge.address);

    let gauge_distributor = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.reward_token.address)
        .mint(&gauge_distributor, &(total_reward as i128));
    let working_supply = setup
        .pool
        .get_rewards_info(&user1)
        .get(Symbol::new(&setup.env, "working_supply"))
        .unwrap() as u128;
    gauge.schedule_rewards_config(
        &setup.pool.address,
        &gauge_distributor,
        &None,
        &reward_duration,
        &reward_tps,
        &working_supply,
    );

    jump(&setup.env, 30);
    assert_eq!(setup.pool.claim(&user1), total_reward / 2);
    assert_eq!(
        setup.pool.gauges_claim(&user1),
        Map::from_array(
            &setup.env,
            [(setup.reward_token.address.clone(), total_reward / 2)]
        )
    );

    let user2_boost_balance = 10_000_0000000i128;
    let total_locked_supply = 20_000_0000000u128;
    get_token_admin_client(&setup.env, &setup.reward_boost_token.address)
        .mint(&user2, &user2_boost_balance);
    setup
        .reward_boost_feed
        .set_total_supply(&setup.admin, &total_locked_supply);
    setup.pool.deposit(&user2, &deposit_amount, &0);

    jump(&setup.env, 10);
    let expected_user1 = total_reward / 6 * 100 / 350;
    let expected_user2 = total_reward / 6 * 250 / 350;

    assert_eq!(setup.pool.claim(&user1), expected_user1);
    assert_eq!(setup.pool.claim(&user2), expected_user2);
    assert_eq!(
        setup.pool.gauges_claim(&user1),
        Map::from_array(
            &setup.env,
            [(setup.reward_token.address.clone(), expected_user1)]
        )
    );
    assert_eq!(
        setup.pool.gauges_claim(&user2),
        Map::from_array(
            &setup.env,
            [(setup.reward_token.address.clone(), expected_user2)]
        )
    );
}
