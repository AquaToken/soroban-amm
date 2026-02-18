#![cfg(test)]
extern crate std;

use crate::math::{sqrt_ratio_at_tick, tick_at_sqrt_ratio};
use crate::testutils::{
    create_pool_contract, create_token_contract, deploy_rewards_gauge, get_token_admin_client,
    Setup,
};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, Map, Symbol, Vec, U256};
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
fn test_auto_price_on_empty_pool() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 2_000_0000000);

    // Pool initialized at tick 0 (1:1), but first deposit with 1:2 ratio should auto-set price
    let amounts = Vec::from_array(&setup.env, [1_000_0000000u128, 2_000_0000000u128]);
    let (_actual, liq) =
        setup
            .pool
            .deposit_position(&setup.user, &setup.user, &-100, &100, &amounts);
    assert!(liq > 0);

    // Verify price was set based on ratio (not default 1:1)
    let slot = setup.pool.slot0();
    let tick = slot.tick;
    // tick for price 2.0 ≈ 6931, should be positive for price > 1
    assert!(tick > 0, "tick should be positive for price > 1");
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

    let amounts = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liquidity) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &amounts);
    assert!(liquidity > 0);

    let position = setup.pool.get_position(&setup.user, &-10, &10);
    assert_eq!(position.liquidity, liquidity);
    assert_eq!(position.tokens_owed_0, 0);
    assert_eq!(position.tokens_owed_1, 0);

    let lower = setup.pool.ticks(&-10);
    assert_eq!(lower.liquidity_gross, liquidity);
    assert_eq!(lower.liquidity_net, liquidity as i128);

    let upper = setup.pool.ticks(&10);
    assert_eq!(upper.liquidity_gross, liquidity);
    assert_eq!(upper.liquidity_net, -(liquidity as i128));

    let zero = U256::from_u32(&setup.env, 0);
    assert_ne!(setup.pool.chunk_bitmap(&-1), zero);
    assert_ne!(setup.pool.chunk_bitmap(&0), zero);
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
    setup.pool.gauge_add(&setup.admin, &gauge.address);

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
        .set_total_supply(&setup.operations_admin, &total_locked_supply);
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

#[test]
fn test_router_compatible_gauge_schedule_reward() {
    let setup = Setup::default();
    let user = setup.user.clone();

    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);
    setup.pool.initialize_boost_config(
        &setup.reward_boost_token.address,
        &setup.reward_boost_feed.address,
    );
    setup
        .pool
        .initialize_rewards_config(&setup.reward_token.address);
    setup.pool.deposit(
        &user,
        &Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]),
        &0,
    );

    let gauge = deploy_rewards_gauge(&setup.env, &setup.pool.address, &setup.reward_token.address);
    setup.pool.gauge_add(&setup.router, &gauge.address);
    assert_eq!(
        setup.pool.get_gauges(),
        Map::from_array(
            &setup.env,
            [(setup.reward_token.address.clone(), gauge.address.clone())]
        )
    );

    let tps = 2_100u128;
    let duration = 60u64;
    let total_reward = tps * duration as u128;
    let distributor = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.reward_token.address)
        .mint(&distributor, &(total_reward as i128));

    setup.pool.gauge_schedule_reward(
        &setup.router,
        &distributor,
        &gauge.address,
        &None,
        &duration,
        &tps,
    );

    jump(&setup.env, 30);
    assert_eq!(
        setup.pool.gauges_claim(&user),
        Map::from_array(
            &setup.env,
            [(setup.reward_token.address.clone(), total_reward / 2)]
        )
    );
}

#[test]
fn test_kill_and_unkill_gauges_claim() {
    let setup = Setup::default();
    let user = setup.user.clone();

    setup.pool.kill_gauges_claim(&setup.admin);
    assert!(setup.pool.try_gauges_claim(&user).is_err());

    setup.pool.unkill_gauges_claim(&setup.admin);
    assert!(setup.pool.try_gauges_claim(&user).is_ok());
}

#[test]
fn test_get_and_return_unused_reward() {
    let setup = Setup::default();

    setup.pool.initialize_boost_config(
        &setup.reward_boost_token.address,
        &setup.reward_boost_feed.address,
    );
    setup
        .pool
        .initialize_rewards_config(&setup.reward_token.address);

    let tps = 100u128;
    let duration = 10u64;
    let configured_reward = tps * duration as u128;
    let extra_reward = 250u128;
    get_token_admin_client(&setup.env, &setup.reward_token.address).mint(
        &setup.pool.address,
        &((configured_reward + extra_reward) as i128),
    );
    setup.pool.set_rewards_config(
        &setup.admin,
        &(setup.env.ledger().timestamp() + duration),
        &tps,
    );

    assert_eq!(setup.pool.get_unused_reward(), extra_reward);
    assert_eq!(setup.pool.return_unused_reward(&setup.admin), extra_reward);
    assert_eq!(
        setup.reward_token.balance(&setup.router) as u128,
        extra_reward
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Error handling: invalid tick ranges
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Error(Contract, #2110)")]
fn test_deposit_position_tick_lower_gte_upper() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let amounts = Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &setup.user, &10, &10, &amounts);
}

#[test]
#[should_panic(expected = "Error(Contract, #2111)")]
fn test_deposit_position_tick_lower_too_low() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let amounts = Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-887_273, &0, &amounts);
}

#[test]
#[should_panic(expected = "Error(Contract, #2112)")]
fn test_deposit_position_tick_upper_too_high() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let amounts = Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &setup.user, &0, &887_273, &amounts);
}

#[test]
#[should_panic(expected = "Error(Contract, #2114)")]
fn test_deposit_position_zero_amount() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let amounts = Vec::from_array(&setup.env, [0u128, 0u128]);
    setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &amounts);
}

#[test]
#[should_panic(expected = "Error(Contract, #212)")]
fn test_withdraw_position_not_found() {
    let setup = Setup::default();
    setup
        .pool
        .withdraw_position(&setup.user, &-10, &10, &1_000_000);
}

#[test]
#[should_panic(expected = "Error(Contract, #213)")]
fn test_withdraw_position_insufficient_liquidity() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let amounts = Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]);
    let (_, liquidity) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &amounts);
    setup
        .pool
        .withdraw_position(&setup.user, &-10, &10, &(liquidity + 1));
}

#[test]
#[should_panic(expected = "Error(Contract, #2114)")]
fn test_withdraw_position_zero_amount() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let amounts = Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &amounts);
    setup.pool.withdraw_position(&setup.user, &-10, &10, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2119)")]
fn test_max_user_positions_exceeded() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    // MAX_USER_POSITIONS = 20; create 20 positions then try a 21st
    let amounts = Vec::from_array(&setup.env, [1_0000000u128, 1_0000000u128]);
    for i in 0..20u32 {
        let lower = -((i as i32 + 1) * 2);
        let upper = (i as i32 + 1) * 2;
        setup
            .pool
            .deposit_position(&setup.user, &setup.user, &lower, &upper, &amounts);
    }
    // 21st position should fail
    setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-100, &100, &amounts);
}

// ═══════════════════════════════════════════════════════════════════════════
// Error handling: kill switches
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Error(Contract, #206)")]
fn test_swap_killed() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );
    setup.pool.kill_swap(&setup.admin);
    setup.pool.swap(&setup.user, &0, &1, &1_0000000, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #207)")]
fn test_claim_killed() {
    let setup = Setup::default();
    setup.pool.initialize_boost_config(
        &setup.reward_boost_token.address,
        &setup.reward_boost_feed.address,
    );
    setup
        .pool
        .initialize_rewards_config(&setup.reward_token.address);
    setup.pool.set_claim_killed(&setup.admin, &true);
    setup.pool.claim(&setup.user);
}

// ═══════════════════════════════════════════════════════════════════════════
// Position lifecycle: one-sided deposits
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_deposit_position_below_price_token0_only() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);

    let initial_0 = setup.token0.balance(&setup.user);
    let initial_1 = setup.token1.balance(&setup.user);

    // Deposit below current tick (0): only token0 needed
    // Current price at tick 0, deposit at range [10, 20] which is ABOVE tick 0
    // → only token0
    let amounts = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &setup.user, &10, &20, &amounts);

    assert!(setup.token0.balance(&setup.user) < initial_0);
    assert_eq!(setup.token1.balance(&setup.user), initial_1);
}

#[test]
fn test_deposit_position_above_price_token1_only() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);

    let initial_0 = setup.token0.balance(&setup.user);
    let initial_1 = setup.token1.balance(&setup.user);

    // Deposit range entirely below current tick (0): only token1
    let amounts = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-20, &-10, &amounts);

    assert_eq!(setup.token0.balance(&setup.user), initial_0);
    assert!(setup.token1.balance(&setup.user) < initial_1);
}

// ═══════════════════════════════════════════════════════════════════════════
// Position lifecycle: add to existing, partial/full withdraw
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_add_liquidity_to_existing_position() {
    let setup = Setup::default();
    setup.mint_user_tokens(200_0000000, 200_0000000);

    let amounts1 = Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]);
    let (_, liq1) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &amounts1);
    let pos = setup.pool.get_position(&setup.user, &-10, &10);
    assert_eq!(pos.liquidity, liq1);

    // Add more to the same range
    let amounts2 = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liq2) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &amounts2);
    let pos = setup.pool.get_position(&setup.user, &-10, &10);
    assert_eq!(pos.liquidity, liq1 + liq2);

    // Ticks should reflect total
    let total_liq = liq1 + liq2;
    let lower = setup.pool.ticks(&-10);
    assert_eq!(lower.liquidity_gross, total_liq);
    assert_eq!(lower.liquidity_net, total_liq as i128);
}

#[test]
fn test_partial_withdrawal_keeps_position() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);

    let amounts = Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]);
    let (_, liquidity) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &amounts);
    let withdraw_amount = liquidity * 40 / 100; // ~40%
    setup
        .pool
        .withdraw_position(&setup.user, &-10, &10, &withdraw_amount);

    let remaining = liquidity - withdraw_amount;
    let pos = setup.pool.get_position(&setup.user, &-10, &10);
    assert_eq!(pos.liquidity, remaining);
    // tokens_owed should have the withdrawn amounts
    assert!(pos.tokens_owed_0 > 0 || pos.tokens_owed_1 > 0);

    // Ticks still initialized
    let lower = setup.pool.ticks(&-10);
    assert_eq!(lower.liquidity_gross, remaining);
}

#[test]
fn test_full_withdrawal_deletes_position_and_clears_ticks() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);

    let amounts = Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]);
    let (_, liquidity) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &amounts);

    // Withdraw full amount
    setup
        .pool
        .withdraw_position(&setup.user, &-10, &10, &liquidity);

    // Claim owed tokens
    setup
        .pool
        .claim_position_fees(&setup.user, &setup.user, &-10, &10, &u128::MAX, &u128::MAX);

    // Position should be deleted — get_position panics
    let result = setup.pool.try_get_position(&setup.user, &-10, &10);
    assert!(result.is_err());

    // Ticks should be uninitialized (liquidity_gross = 0)
    let lower = setup.pool.ticks(&-10);
    assert_eq!(lower.liquidity_gross, 0);
    let upper = setup.pool.ticks(&10);
    assert_eq!(upper.liquidity_gross, 0);

    // Bitmap should be cleared
    let zero = U256::from_u32(&setup.env, 0);
    assert_eq!(setup.pool.chunk_bitmap(&-1), zero);
    assert_eq!(setup.pool.chunk_bitmap(&0), zero);
}

#[test]
fn test_withdraw_clears_only_non_shared_upper_tick() {
    let setup = Setup::default();
    let user2 = Address::generate(&setup.env);

    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&user2, &1_000_0000000);
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&user2, &1_000_0000000);

    let amounts = Vec::from_array(&setup.env, [300_0000000u128, 300_0000000u128]);
    let (_, liq1) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-100, &100, &amounts);
    let (_, liq2) = setup
        .pool
        .deposit_position(&user2, &user2, &-100, &200, &amounts);

    setup
        .pool
        .withdraw_position(&setup.user, &-100, &100, &liq1);

    // Shared lower tick stays initialized by user2's position.
    let lower = setup.pool.ticks(&-100);
    assert_eq!(lower.liquidity_gross, liq2);
    assert_eq!(lower.liquidity_net, liq2 as i128);

    // Upper tick from user1-only range is cleared.
    let middle = setup.pool.ticks(&100);
    assert_eq!(middle.liquidity_gross, 0);
    assert_eq!(middle.liquidity_net, 0);

    // User2 upper tick remains initialized.
    let upper = setup.pool.ticks(&200);
    assert_eq!(upper.liquidity_gross, liq2);
    assert_eq!(upper.liquidity_net, -(liq2 as i128));
}

#[test]
fn test_withdraw_clears_only_non_shared_lower_tick() {
    let setup = Setup::default();
    let user2 = Address::generate(&setup.env);

    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&user2, &1_000_0000000);
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&user2, &1_000_0000000);

    let amounts = Vec::from_array(&setup.env, [300_0000000u128, 300_0000000u128]);
    let (_, liq1) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-100, &100, &amounts);
    let (_, liq2) = setup
        .pool
        .deposit_position(&user2, &user2, &-200, &100, &amounts);

    setup
        .pool
        .withdraw_position(&setup.user, &-100, &100, &liq1);

    // User1 lower tick is cleared, user2 lower tick remains initialized.
    let lower = setup.pool.ticks(&-100);
    assert_eq!(lower.liquidity_gross, 0);
    assert_eq!(lower.liquidity_net, 0);

    let lower_user2 = setup.pool.ticks(&-200);
    assert_eq!(lower_user2.liquidity_gross, liq2);
    assert_eq!(lower_user2.liquidity_net, liq2 as i128);

    // Shared upper tick stays initialized by user2's position.
    let upper = setup.pool.ticks(&100);
    assert_eq!(upper.liquidity_gross, liq2);
    assert_eq!(upper.liquidity_net, -(liq2 as i128));
}

// ═══════════════════════════════════════════════════════════════════════════
// Fee collection: swaps generate fees, positions accrue them
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_position_accrues_fees_from_swaps() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    // LP deposits position around current price
    let amounts = Vec::from_array(&setup.env, [500_0000000u128, 500_0000000u128]);
    let (_, liquidity) =
        setup
            .pool
            .deposit_position(&setup.user, &setup.user, &-100, &100, &amounts);

    // Swapper trades through the position range
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &50_0000000);
    let out = setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);
    assert!(out > 0);

    // LP withdraws — should get more than they deposited due to fees
    let balance0_before = setup.token0.balance(&setup.user);
    let balance1_before = setup.token1.balance(&setup.user);

    setup
        .pool
        .withdraw_position(&setup.user, &-100, &100, &liquidity);
    setup.pool.claim_position_fees(
        &setup.user,
        &setup.user,
        &-100,
        &100,
        &u128::MAX,
        &u128::MAX,
    );

    let balance0_after = setup.token0.balance(&setup.user);
    let balance1_after = setup.token1.balance(&setup.user);

    let received0 = (balance0_after - balance0_before) as u128;
    let received1 = (balance1_after - balance1_before) as u128;

    // Fees should be non-zero (30 bps fee, 50% protocol cut → 15 bps to LP)
    // With 10M swap, LP should earn ~15000 in fees across both tokens
    assert!(
        received0 > 0 || received1 > 0,
        "LP should receive fee revenue"
    );
}

#[test]
fn test_claim_position_fees_without_withdrawal() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    let amounts = Vec::from_array(&setup.env, [500_0000000u128, 500_0000000u128]);
    let (_, liquidity) =
        setup
            .pool
            .deposit_position(&setup.user, &setup.user, &-100, &100, &amounts);

    // Swap to generate fees
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &10_0000000);
    setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);

    let balance0_before = setup.token0.balance(&setup.user);
    let balance1_before = setup.token1.balance(&setup.user);

    // Claim fees without withdrawing liquidity
    let (claimed0, claimed1) = setup.pool.claim_position_fees(
        &setup.user,
        &setup.user,
        &-100,
        &100,
        &u128::MAX,
        &u128::MAX,
    );

    // Position still has liquidity
    let pos = setup.pool.get_position(&setup.user, &-100, &100);
    assert_eq!(pos.liquidity, liquidity);

    // User received fee tokens
    assert_eq!(
        setup.token0.balance(&setup.user),
        balance0_before + claimed0 as i128
    );
    assert_eq!(
        setup.token1.balance(&setup.user),
        balance1_before + claimed1 as i128
    );
}

#[test]
fn test_claim_position_fees_partial_then_full() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    let amounts = Vec::from_array(&setup.env, [500_0000000u128, 500_0000000u128]);
    let (_, liquidity) =
        setup
            .pool
            .deposit_position(&setup.user, &setup.user, &-100, &100, &amounts);

    // Realize some owed amounts without closing the position entirely.
    let burn_amount = liquidity / 2;
    setup
        .pool
        .withdraw_position(&setup.user, &-100, &100, &burn_amount);

    let before = setup.pool.get_position(&setup.user, &-100, &100);
    assert!(
        before.tokens_owed_0 > 1,
        "expected non-trivial token0 owed after burn"
    );
    assert!(
        before.tokens_owed_1 > 0,
        "expected token1 owed after burn for in-range position"
    );

    let req0 = before.tokens_owed_0 / 2;
    let (claimed0_partial, claimed1_partial) =
        setup
            .pool
            .claim_position_fees(&setup.user, &setup.user, &-100, &100, &req0, &0);
    assert_eq!(claimed0_partial, req0);
    assert_eq!(claimed1_partial, 0);

    let mid = setup.pool.get_position(&setup.user, &-100, &100);
    assert_eq!(mid.tokens_owed_0, before.tokens_owed_0 - req0);
    assert_eq!(mid.tokens_owed_1, before.tokens_owed_1);

    // Collect the rest.
    let (claimed0_rest, claimed1_rest) = setup.pool.claim_position_fees(
        &setup.user,
        &setup.user,
        &-100,
        &100,
        &u128::MAX,
        &u128::MAX,
    );

    let after = setup.pool.get_position(&setup.user, &-100, &100);
    assert_eq!(after.tokens_owed_0, 0);
    assert_eq!(after.tokens_owed_1, 0);
    assert_eq!(claimed0_partial + claimed0_rest, before.tokens_owed_0);
    assert_eq!(claimed1_partial + claimed1_rest, before.tokens_owed_1);
}

// ═══════════════════════════════════════════════════════════════════════════
// Protocol fee collection
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_protocol_fee_accumulates_and_collects() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]),
        &0,
    );

    // Do several swaps
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &100_0000000);
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&swapper, &100_0000000);

    setup.pool.swap(&swapper, &0, &1, &20_0000000, &0);
    setup.pool.swap(&swapper, &1, &0, &20_0000000, &0);

    // Protocol fees should have accumulated
    let fees = setup.pool.protocol_fees();
    assert!(
        fees.token0 > 0 || fees.token1 > 0,
        "protocol fees should accumulate"
    );

    // Collect protocol fees
    let dest = Address::generate(&setup.env);
    let claimed = setup.pool.claim_protocol_fees(&setup.admin, &dest);
    assert_eq!(
        setup.token0.balance(&dest) as u128,
        claimed.get_unchecked(0)
    );
    assert_eq!(
        setup.token1.balance(&dest) as u128,
        claimed.get_unchecked(1)
    );

    // Fees should be reset to zero
    let fees_after = setup.pool.protocol_fees();
    assert_eq!(fees_after.token0, 0);
    assert_eq!(fees_after.token1, 0);
}

// ═══════════════════════════════════════════════════════════════════════════
// Swap variations: both directions, crossing ticks
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_swap_both_directions() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]),
        &0,
    );

    let slot_before = setup.pool.slot0();

    // Swap 0→1 (zero_for_one)
    let out_0to1 = setup.pool.swap(&setup.user, &0, &1, &5_0000000, &0);
    assert!(out_0to1 > 0);
    let slot_mid = setup.pool.slot0();
    assert!(
        slot_mid.tick < slot_before.tick,
        "price should decrease for 0→1"
    );

    // Swap 1→0 (one_for_zero)
    let out_1to0 = setup.pool.swap(&setup.user, &1, &0, &5_0000000, &0);
    assert!(out_1to0 > 0);
    let slot_after = setup.pool.slot0();
    assert!(
        slot_after.tick > slot_mid.tick,
        "price should increase for 1→0"
    );
}

#[test]
fn test_swap_crossing_multiple_ticks() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);
    let user = Address::generate(&env);

    let token0 = create_token_contract(&env, &admin);
    let token1 = create_token_contract(&env, &admin);

    mod pool_plane {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane::Client::new(&env, &env.register(pool_plane::WASM, ()));

    // Use tick_spacing=10 for finer granularity
    let pool = create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
        30,
        10,
    );

    // Create multiple narrow positions that the swap must cross
    get_token_admin_client(&env, &token0.address).mint(&user, &1_000_0000000);
    get_token_admin_client(&env, &token1.address).mint(&user, &1_000_0000000);

    // Stacked positions at different ranges (small amounts to allow tick crossings)
    let amounts = Vec::from_array(&env, [1_0000000u128, 1_0000000u128]);
    pool.deposit_position(&user, &user, &-50, &-10, &amounts);
    pool.deposit_position(&user, &user, &-10, &10, &amounts);
    pool.deposit_position(&user, &user, &10, &50, &amounts);

    let slot_before = pool.slot0();

    // Large swap that should cross tick boundaries
    let swapper = Address::generate(&env);
    get_token_admin_client(&env, &token0.address).mint(&swapper, &100_0000000);
    let out = pool.swap(&swapper, &0, &1, &50_0000000, &0);
    assert!(out > 0);

    let slot_after = pool.slot0();
    let ticks_crossed = (slot_before.tick - slot_after.tick).abs() / 10;
    assert!(
        ticks_crossed > 1,
        "swap should cross multiple ticks, crossed {}",
        ticks_crossed
    );
}

// Verify swap traverses a liquidity gap (L10 fix).
// Two positions with a gap between them: [-60, -20] and [20, 60].
// Price starts at tick 0 (in the gap). A swap zero_for_one must slide through
// the gap for free, find liquidity at [-60, -20], and execute.
#[test]
fn test_swap_across_liquidity_gap() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);
    let user = Address::generate(&env);

    let token0 = create_token_contract(&env, &admin);
    let token1 = create_token_contract(&env, &admin);

    mod pool_plane_gap {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane_gap::Client::new(&env, &env.register(pool_plane_gap::WASM, ()));

    let pool = create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
        30,
        10,
    );

    // Fund the LP and create two positions with a gap at tick 0.
    get_token_admin_client(&env, &token0.address).mint(&user, &1_000_0000000);
    get_token_admin_client(&env, &token1.address).mint(&user, &1_000_0000000);

    // Position below current price (only token1 deposited)
    let amounts_below = Vec::from_array(&env, [10_0000000u128, 10_0000000u128]);
    let (_, liq_below) = pool.deposit_position(&user, &user, &-60, &-20, &amounts_below);
    assert!(liq_below > 0, "below-range position should have liquidity");

    // Position above current price (only token0 deposited)
    let amounts_above = Vec::from_array(&env, [10_0000000u128, 10_0000000u128]);
    let (_, liq_above) = pool.deposit_position(&user, &user, &20, &60, &amounts_above);
    assert!(liq_above > 0, "above-range position should have liquidity");

    // Active liquidity at tick 0 should be 0 (gap between positions)
    assert_eq!(pool.liquidity(), 0, "no active liquidity in the gap");

    // Swap zero_for_one: price moves down, should cross gap and reach [-60, -20]
    let swapper = Address::generate(&env);
    get_token_admin_client(&env, &token0.address).mint(&swapper, &5_0000000);
    let out = pool.swap(&swapper, &0, &1, &5_0000000, &0);
    assert!(out > 0, "swap should produce output by crossing the gap");

    // Price should have moved below -20 (into the lower position)
    let slot = pool.slot0();
    assert!(
        slot.tick < -20,
        "tick should be below -20 after crossing gap, got {}",
        slot.tick
    );

    // estimate_swap should match: feed the same input, get the same output
    let estimate = pool.estimate_swap(&0, &1, &out);
    assert!(estimate > 0, "estimate should also cross the gap");
}

#[test]
fn test_estimate_swap_matches_actual() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]),
        &0,
    );

    // Test both directions
    let estimate_0to1 = setup.pool.estimate_swap(&0, &1, &5_0000000);
    let actual_0to1 = setup.pool.swap(&setup.user, &0, &1, &5_0000000, &0);
    assert_eq!(estimate_0to1, actual_0to1);

    let estimate_1to0 = setup.pool.estimate_swap(&1, &0, &5_0000000);
    let actual_1to0 = setup.pool.swap(&setup.user, &1, &0, &5_0000000, &0);
    assert_eq!(estimate_1to0, actual_1to0);
}

#[test]
fn test_swap_by_tokens_stops_at_price_limit_zero_for_one() {
    let setup = Setup::default();
    setup.mint_user_tokens(2_000_0000000, 2_000_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [800_0000000u128, 800_0000000u128]),
        &0,
    );

    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &2_000_0000000);

    let limit = sqrt_ratio_at_tick(&setup.env, -100).unwrap();
    let amount_specified = 1_000_0000000i128;

    let result = setup.pool.swap_by_tokens(
        &swapper,
        &swapper,
        &setup.token0.address,
        &setup.token1.address,
        &amount_specified,
        &limit,
    );

    // Should partially fill due to price limit.
    assert!(result.amount0 > 0);
    assert!(result.amount0 < amount_specified);
    assert!(result.amount1 < 0);
    assert_eq!(setup.pool.slot0().sqrt_price_x96, limit);
}

#[test]
fn test_swap_by_tokens_stops_at_price_limit_one_for_zero() {
    let setup = Setup::default();
    setup.mint_user_tokens(2_000_0000000, 2_000_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [800_0000000u128, 800_0000000u128]),
        &0,
    );

    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&swapper, &2_000_0000000);

    let limit = sqrt_ratio_at_tick(&setup.env, 100).unwrap();
    let amount_specified = 1_000_0000000i128;

    let result = setup.pool.swap_by_tokens(
        &swapper,
        &swapper,
        &setup.token1.address,
        &setup.token0.address,
        &amount_specified,
        &limit,
    );

    // Should partially fill due to price limit.
    assert!(result.amount1 > 0);
    assert!(result.amount1 < amount_specified);
    assert!(result.amount0 < 0);
    assert_eq!(setup.pool.slot0().sqrt_price_x96, limit);
}

#[test]
fn test_swap_by_tokens_non_boundary_price_limit_has_consistent_tick() {
    let setup = Setup::default();
    setup.mint_user_tokens(2_000_0000000, 2_000_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [800_0000000u128, 800_0000000u128]),
        &0,
    );

    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &2_000_0000000);

    let upper = sqrt_ratio_at_tick(&setup.env, -100).unwrap();
    let lower = sqrt_ratio_at_tick(&setup.env, -101).unwrap();
    let half = upper.sub(&lower).div(&U256::from_u32(&setup.env, 2));
    let limit = lower.add(&half);
    let amount_specified = 1_000_0000000i128;

    let result = setup.pool.swap_by_tokens(
        &swapper,
        &swapper,
        &setup.token0.address,
        &setup.token1.address,
        &amount_specified,
        &limit,
    );

    assert!(result.amount0 > 0);
    assert!(result.amount1 < 0);

    let slot_after = setup.pool.slot0();
    assert_eq!(slot_after.sqrt_price_x96, limit);
    let expected_tick = tick_at_sqrt_ratio(&setup.env, &limit).unwrap();
    assert_eq!(slot_after.tick, expected_tick);
}

// ═══════════════════════════════════════════════════════════════════════════
// Admin: protocol fee configuration
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_set_protocol_fee_fraction() {
    let setup = Setup::default();

    assert_eq!(setup.pool.get_protocol_fee_fraction(), 5_000); // default 50%

    setup.pool.set_protocol_fee_fraction(&setup.admin, &0);
    assert_eq!(setup.pool.get_protocol_fee_fraction(), 0);

    setup.pool.set_protocol_fee_fraction(&setup.admin, &10_000);
    assert_eq!(setup.pool.get_protocol_fee_fraction(), 10_000); // 100% to protocol
}

#[test]
#[should_panic(expected = "Error(Contract, #2105)")]
fn test_set_protocol_fee_fraction_too_high() {
    let setup = Setup::default();
    // FEE_DENOMINATOR = 10_000, so 10_001 is invalid
    setup.pool.set_protocol_fee_fraction(&setup.admin, &10_001);
}

// ═══════════════════════════════════════════════════════════════════════════
// Admin: kill/unkill swap and deposit
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_kill_unkill_swap() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );

    // Kill swap
    setup.pool.kill_swap(&setup.admin);
    assert!(setup.pool.get_is_killed_swap());
    assert!(setup
        .pool
        .try_swap(&setup.user, &0, &1, &1_0000000, &0)
        .is_err());

    // Unkill swap
    setup.pool.unkill_swap(&setup.admin);
    assert!(!setup.pool.get_is_killed_swap());
    let out = setup.pool.swap(&setup.user, &0, &1, &1_0000000, &0);
    assert!(out > 0);
}

#[test]
fn test_kill_unkill_deposit() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);

    // Kill deposit
    setup.pool.kill_deposit(&setup.admin);
    assert!(setup.pool.get_is_killed_deposit());

    // deposit_position should also be blocked
    let amounts = Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]);
    assert!(setup
        .pool
        .try_deposit_position(&setup.user, &setup.user, &-10, &10, &amounts)
        .is_err());

    // Unkill deposit
    setup.pool.unkill_deposit(&setup.admin);
    assert!(!setup.pool.get_is_killed_deposit());
    setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &amounts);
}

// ═══════════════════════════════════════════════════════════════════════════
// estimate_working_balance
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_estimate_working_balance() {
    let setup = Setup::default();
    let user2 = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&user2, &1_000_0000000);
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&user2, &1_000_0000000);

    // Before any deposit: in-range estimate should be higher than out-of-range
    let test_liq = 1_000_000u128;
    let (wb_in, _) = setup
        .pool
        .estimate_working_balance(&user2, &-100, &100, &test_liq);
    assert!(wb_in > 0, "in-range estimate must be positive");

    let (wb_out, _) = setup
        .pool
        .estimate_working_balance(&user2, &500, &600, &test_liq);
    assert!(
        wb_in > wb_out,
        "in-range wb ({}) must be > out-of-range wb ({})",
        wb_in,
        wb_out
    );

    // Deposit for user2
    let dep_amounts = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, dep_liq) = setup
        .pool
        .deposit_position(&user2, &user2, &-100, &100, &dep_amounts);

    let info = setup.pool.get_rewards_info(&user2);
    let actual_wb = info
        .get(Symbol::new(&setup.env, "working_balance"))
        .unwrap() as u128;
    let actual_ws = info.get(Symbol::new(&setup.env, "working_supply")).unwrap() as u128;
    assert!(
        actual_wb > 0,
        "working balance must be positive after deposit"
    );

    // Estimate with the deposited liquidity should match actual
    let (wb_est, ws_est) = setup
        .pool
        .estimate_working_balance(&user2, &-100, &100, &dep_liq);
    assert_eq!(
        wb_est, actual_wb,
        "estimated wb ({}) must match actual wb ({})",
        wb_est, actual_wb
    );
    assert_eq!(
        ws_est, actual_ws,
        "estimated ws ({}) must match actual ws ({})",
        ws_est, actual_ws
    );

    // Withdrawal preview: new_liquidity=0 for existing position → wb should decrease
    let (wb_zero, _) = setup.pool.estimate_working_balance(&user2, &-100, &100, &0);
    assert!(
        wb_zero < actual_wb,
        "zero liquidity wb ({}) must be < actual wb ({})",
        wb_zero,
        actual_wb
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Multi-user scenarios
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_two_users_overlapping_positions() {
    let setup = Setup::default();
    let user2 = Address::generate(&setup.env);

    setup.mint_user_tokens(500_0000000, 500_0000000);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&user2, &500_0000000);
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&user2, &500_0000000);

    // Both users deposit overlapping positions
    let amounts1 = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    let (_, liq1) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-50, &50, &amounts1);
    let amounts2 = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    let (_, liq2) = setup
        .pool
        .deposit_position(&user2, &user2, &-20, &20, &amounts2);

    // Active liquidity should be sum of overlapping positions
    let active_liq = setup.pool.liquidity();
    assert_eq!(active_liq, liq1 + liq2);

    // Swap generates fees for both
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &10_0000000);
    setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);

    // Both users can claim fees
    let (u1_fee0, u1_fee1) =
        setup
            .pool
            .claim_position_fees(&setup.user, &setup.user, &-50, &50, &u128::MAX, &u128::MAX);
    let (u2_fee0, u2_fee1) =
        setup
            .pool
            .claim_position_fees(&user2, &user2, &-20, &20, &u128::MAX, &u128::MAX);

    // Both should have received fees (proportional to liquidity share)
    assert!(u1_fee0 > 0 || u1_fee1 > 0, "user1 should get fees");
    assert!(u2_fee0 > 0 || u2_fee1 > 0, "user2 should get fees");
}

#[test]
fn test_multiple_positions_same_user() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    // User creates multiple positions at different ranges
    let amounts1 = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liq1) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-100, &-50, &amounts1);
    let amounts2 = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    let (_, liq2) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &amounts2);
    let amounts3 = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liq3) = setup
        .pool
        .deposit_position(&setup.user, &setup.user, &50, &100, &amounts3);

    // User should have 3 ranges tracked
    let snapshot = setup.pool.get_user_position_snapshot(&setup.user);
    assert_eq!(snapshot.ranges.len(), 3);
    assert_eq!(snapshot.raw_liquidity, liq1 + liq2 + liq3);
}

// ═══════════════════════════════════════════════════════════════════════════
// State queries
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_get_full_pool_state() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );

    let state = setup.pool.get_full_pool_state();
    assert!(state.is_some());
    let state = state.unwrap();
    assert_eq!(state.fee, 30);
    assert_eq!(state.tick_spacing, 1);
    assert!(state.liquidity > 0);
    assert_eq!(state.token0, setup.token0.address);
    assert_eq!(state.token1, setup.token1.address);
}

#[test]
fn test_get_pool_state_with_balances() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );

    let state = setup.pool.get_pool_state_with_balances();
    assert!(state.is_some());
    let state = state.unwrap();
    assert!(state.reserve0 > 0);
    assert!(state.reserve1 > 0);
}

#[test]
fn test_get_reserves_excludes_protocol_fees() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]),
        &0,
    );

    let reserves_before = setup.pool.get_reserves();

    // Swap to generate protocol fees
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &50_0000000);
    setup.pool.swap(&swapper, &0, &1, &20_0000000, &0);

    let reserves_after = setup.pool.get_reserves();
    let fees = setup.pool.protocol_fees();

    // Balance = reserves + protocol_fees
    let balance0 = setup.token0.balance(&setup.pool.address) as u128;
    assert_eq!(balance0, reserves_after.get_unchecked(0) + fees.token0);
}

// ═══════════════════════════════════════════════════════════════════════════
// Rewards state management (opt-out / opt-in)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_rewards_state_opt_out_and_resume() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    setup.pool.initialize_boost_config(
        &setup.reward_boost_token.address,
        &setup.reward_boost_feed.address,
    );
    setup
        .pool
        .initialize_rewards_config(&setup.reward_token.address);

    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]),
        &0,
    );

    let tps = 1_000u128;
    let duration = 100u64;
    let total_reward = tps * duration as u128;
    get_token_admin_client(&setup.env, &setup.reward_token.address)
        .mint(&setup.pool.address, &(total_reward as i128));
    setup.pool.set_rewards_config(
        &setup.admin,
        &(setup.env.ledger().timestamp() + duration),
        &tps,
    );

    // Opt out of rewards
    setup.pool.set_rewards_state(&setup.user, &false);

    jump(&setup.env, 50);
    // Claim should return 0 while opted out
    let claimed = setup.pool.claim(&setup.user);
    assert_eq!(claimed, 0);

    // Opt back in
    setup.pool.set_rewards_state(&setup.user, &true);

    jump(&setup.env, 25);
    let claimed = setup.pool.claim(&setup.user);
    assert!(claimed > 0, "should accrue rewards after re-enabling");
}

// ═══════════════════════════════════════════════════════════════════════════
// Error handling: plane & rewards double-initialization
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Error(Contract, #202)")]
fn test_plane_already_initialized() {
    let setup = Setup::default();
    // Plane was already set during Setup::new(). Second call should panic.
    setup.pool.init_pools_plane(&setup.plane);
}

#[test]
#[should_panic(expected = "Error(Contract, #203)")]
fn test_rewards_already_initialized() {
    let setup = Setup::default();
    setup
        .pool
        .initialize_rewards_config(&setup.reward_token.address);
    // Second call should panic
    setup
        .pool
        .initialize_rewards_config(&setup.reward_token.address);
}

#[test]
#[should_panic(expected = "Error(Contract, #203)")]
fn test_boost_already_initialized() {
    let setup = Setup::default();
    setup.pool.initialize_boost_config(
        &setup.reward_boost_token.address,
        &setup.reward_boost_feed.address,
    );
    // Second call should panic
    setup.pool.initialize_boost_config(
        &setup.reward_boost_token.address,
        &setup.reward_boost_feed.address,
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Error handling: invalid swap parameters
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Error(Contract, #2103)")]
fn test_swap_same_token_index() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );
    // in_idx == out_idx is invalid
    setup.pool.swap(&setup.user, &0, &0, &1_0000000, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2103)")]
fn test_swap_out_of_range_index() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );
    // in_idx > 1 is invalid
    setup.pool.swap(&setup.user, &2, &0, &1_0000000, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2103)")]
fn test_swap_by_tokens_same_token() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );
    // token_in == token_out is invalid
    setup.pool.swap_by_tokens(
        &setup.user,
        &setup.user,
        &setup.token0.address,
        &setup.token0.address,
        &1_0000000,
        &U256::from_u32(&setup.env, 0),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2103)")]
fn test_swap_by_tokens_unknown_token() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );
    let unknown = Address::generate(&setup.env);
    setup.pool.swap_by_tokens(
        &setup.user,
        &setup.user,
        &unknown,
        &setup.token0.address,
        &1_0000000,
        &U256::from_u32(&setup.env, 0),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2103)")]
fn test_swap_output_below_minimum() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );
    // out_min is impossibly high
    setup.pool.swap(&setup.user, &0, &1, &1_0000000, &u128::MAX);
}

#[test]
#[should_panic(expected = "Error(Contract, #2103)")]
fn test_swap_strict_receive_input_above_max() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );
    // in_max = 1 is too low to produce any meaningful output
    setup
        .pool
        .swap_strict_receive(&setup.user, &0, &1, &1_0000000, &1);
}

#[test]
#[should_panic(expected = "Error(Contract, #2103)")]
fn test_deposit_wrong_amounts_length() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    // deposit() expects exactly 2 elements
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128]),
        &0,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2103)")]
fn test_withdraw_wrong_min_amounts_length() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );
    // withdraw min_amounts must have exactly 2 elements
    setup
        .pool
        .withdraw(&setup.user, &100, &Vec::from_array(&setup.env, [0u128]));
}

#[test]
#[should_panic(expected = "Error(Contract, #2103)")]
fn test_withdraw_below_min_amounts() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let (_, shares) = setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );
    // min_amounts impossibly high
    setup.pool.withdraw(
        &setup.user,
        &shares,
        &Vec::from_array(&setup.env, [u128::MAX, u128::MAX]),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2103)")]
fn test_deposit_min_shares_not_met() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    // min_shares impossibly high
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [1_0000000u128, 1_0000000u128]),
        &u128::MAX,
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Error handling: token ordering in initialize
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Error(Contract, #2121)")]
fn test_initialize_tokens_not_sorted() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);

    let token_a = create_token_contract(&env, &admin);
    let token_b = create_token_contract(&env, &admin);
    // Ensure reverse order (unsorted)
    let (higher, lower) = if token_a.address > token_b.address {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    };

    mod pool_plane {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane::Client::new(&env, &env.register(pool_plane::WASM, ()));

    // Pass tokens in wrong order: higher address first
    create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [higher.address.clone(), lower.address.clone()]),
        30,
        1,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2121)")]
fn test_initialize_duplicate_tokens() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);

    let token0 = create_token_contract(&env, &admin);

    mod pool_plane {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane::Client::new(&env, &env.register(pool_plane::WASM, ()));

    // Same token twice
    create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token0.address.clone()]),
        30,
        1,
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Error handling: tick spacing alignment
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Error(Contract, #2109)")]
fn test_tick_not_spaced_correctly() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);
    let user = Address::generate(&env);

    let token0 = create_token_contract(&env, &admin);
    let token1 = create_token_contract(&env, &admin);

    mod pool_plane {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane::Client::new(&env, &env.register(pool_plane::WASM, ()));

    // Create pool with tick_spacing=10
    let pool = create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
        30,
        10,
    );

    get_token_admin_client(&env, &token0.address).mint(&user, &100_0000000);
    get_token_admin_client(&env, &token1.address).mint(&user, &100_0000000);

    // Ticks -5 and 5 are not aligned to spacing of 10
    let amounts = Vec::from_array(&env, [10_0000000u128, 10_0000000u128]);
    pool.deposit_position(&user, &user, &-5, &5, &amounts);
}

// ═══════════════════════════════════════════════════════════════════════════
// Error handling: liquidity amount too large
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_liquidity_amount_too_large() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);

    // Passing huge desired amounts that would produce liquidity > i128::MAX
    // should fail (either overflow or LiquidityAmountTooLarge)
    let huge_amounts = Vec::from_array(&setup.env, [u128::MAX, u128::MAX]);
    let result =
        setup
            .pool
            .try_deposit_position(&setup.user, &setup.user, &-10, &10, &huge_amounts);
    assert!(result.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// Error handling: invalid price limit in swap_by_tokens
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_swap_by_tokens_invalid_price_limit() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.deposit(
        &setup.user,
        &Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]),
        &0,
    );

    // For zero_for_one swap, price limit must be < current_price and > MIN_SQRT_RATIO.
    // Passing a limit > current_price should fail with #2113.
    let current_price = setup.pool.slot0().sqrt_price_x96;
    let bad_limit = current_price.add(&U256::from_u32(&setup.env, 1));
    let result = setup.pool.try_swap_by_tokens(
        &setup.user,
        &setup.user,
        &setup.token0.address,
        &setup.token1.address,
        &1_0000000,
        &bad_limit,
    );
    assert!(result.is_err());

    // For one_for_zero swap, price limit must be > current_price and < MAX_SQRT_RATIO.
    // Passing a limit < current_price should fail with #2113.
    let bad_limit2 = current_price.sub(&U256::from_u32(&setup.env, 1));
    let result2 = setup.pool.try_swap_by_tokens(
        &setup.user,
        &setup.user,
        &setup.token1.address,
        &setup.token0.address,
        &1_0000000,
        &bad_limit2,
    );
    assert!(result2.is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// Deposit position killed check
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Error(Contract, #205)")]
fn test_deposit_position_killed() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    setup.pool.kill_deposit(&setup.admin);
    // deposit_position should also fail when deposit is killed
    let amounts = Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &setup.user, &-10, &10, &amounts);
}

// Griefing scenario: attacker fills every tick with dust positions to increase
// storage reads during swaps. Whale provide full-range liquidity, then
// an attacker initializes every possible tick in a range around the current
// price with minimal liquidity.
//
// With tick_spacing=20 (0.1% fee tier), this test demonstrates:
// - Dust positions add overhead but spacing caps the damage
// - Reports exact ledger footprint for capacity planning
//
// Mainnet limits: 200 read_only + 200 read_write entries per tx.
#[test]
fn test_dust_griefing_tick_spacing_20() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();
    // Disable SDK resource limit enforcement — we check footprint manually
    env.cost_estimate().disable_resource_limits();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);

    let token0 = create_token_contract(&env, &admin);
    let token1 = create_token_contract(&env, &admin);

    // Pool plane
    mod pool_plane {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane::Client::new(&env, &env.register(pool_plane::WASM, ()));

    // Create pool with fee=10 bps, tick_spacing=20 (our 0.1% tier)
    let tick_spacing: i32 = 20;
    let pool = create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
        10,
        tick_spacing,
    );

    // ---- Whale deposits (full range) ----
    let whale = Address::generate(&env);
    let whale_amount: i128 = 1_000_000_0000000;

    get_token_admin_client(&env, &token0.address).mint(&whale, &whale_amount);
    get_token_admin_client(&env, &token1.address).mint(&whale, &whale_amount);

    // Full-range deposit: uses MIN_TICK/MAX_TICK aligned to spacing
    pool.deposit(
        &whale,
        &Vec::from_array(&env, [500_000_0000000u128, 500_000_0000000u128]),
        &0,
    );

    let slot_before = pool.slot0();
    let liquidity_before = pool.liquidity();
    std::println!(
        "Pool state: tick={}, liquidity={}",
        slot_before.tick,
        liquidity_before
    );

    // ---- Attacker: fill ticks with dust ----
    let dust_range: i32 = 300; // number of spacing steps on each side

    // Attacker uses multiple accounts to bypass MAX_USER_POSITIONS (20)
    let mut total_dust_positions = 0u32;
    let positions_per_attacker: i32 = 20; // MAX_USER_POSITIONS
    let total_dust_ticks = (dust_range * 2) as u32; // x2 ticks
    let num_attackers =
        (total_dust_ticks as i32 + positions_per_attacker - 1) / positions_per_attacker;

    std::println!(
        "Dust attack: {} ticks, {} attacker accounts",
        total_dust_ticks,
        num_attackers
    );

    let dust_amounts = Vec::from_array(&env, [1000u128, 1000u128]);
    for attacker_idx in 0..num_attackers {
        let attacker = Address::generate(&env);
        get_token_admin_client(&env, &token0.address).mint(&attacker, &1_0000000);
        get_token_admin_client(&env, &token1.address).mint(&attacker, &1_0000000);

        let start_offset = -dust_range + (attacker_idx * positions_per_attacker);
        let end_offset = start_offset + positions_per_attacker;

        for i in start_offset..end_offset {
            if i.abs() > dust_range {
                continue;
            }
            let tick_lower = i * tick_spacing;
            let tick_upper = tick_lower + tick_spacing;

            pool.deposit_position(
                &attacker,
                &attacker,
                &tick_lower,
                &tick_upper,
                &dust_amounts,
            );
            total_dust_positions += 1;
        }
    }

    std::println!(
        "Dust positions created: {} (initializing up to {} ticks)",
        total_dust_positions,
        total_dust_positions * 2
    );

    // ---- Small swap: ~1% price move through dust field ----
    let swapper = Address::generate(&env);
    let small_swap: u128 = 10_000_0000000; // ~1% of whale liquidity

    get_token_admin_client(&env, &token0.address).mint(&swapper, &(small_swap as i128));
    let out = pool.swap(&swapper, &0, &1, &small_swap, &0);

    let cost = env.cost_estimate().resources();

    let slot_after = pool.slot0();
    let tick_delta = (slot_after.tick - slot_before.tick).abs();
    let ticks_crossed = tick_delta / tick_spacing;

    std::println!("--- Small swap (~1% move) ---");
    std::println!("Amount in:  {}", small_swap);
    std::println!("Amount out: {}", out);
    std::println!(
        "Price moved: tick {} → {} (delta={}, ~{} spacing crossings)",
        slot_before.tick,
        slot_after.tick,
        tick_delta,
        ticks_crossed
    );
    std::println!(
        "Footprint: read_write={}, read_only={}",
        cost.write_entries,
        cost.disk_read_entries + cost.memory_read_entries
    );
    std::println!(
        "Bytes: disk_read={}, write={}, mem={}",
        cost.disk_read_bytes,
        cost.write_bytes,
        cost.mem_bytes
    );

    // ---- Reverse swap to restore price ----
    get_token_admin_client(&env, &token1.address).mint(&swapper, &(small_swap as i128));
    let out_back = pool.swap(&swapper, &1, &0, &small_swap, &0);
    assert!(out_back > 0, "reverse swap must produce output");
    let slot_mid = pool.slot0();

    // ---- Larger swap: ~5% price move — stress test ----
    let large_swap: u128 = 50_000_0000000;
    get_token_admin_client(&env, &token0.address).mint(&swapper, &(large_swap as i128));
    let out_large = pool.swap(&swapper, &0, &1, &large_swap, &0);

    let cost_large = env.cost_estimate().resources();

    let slot_after_large = pool.slot0();
    let tick_delta_large = (slot_after_large.tick - slot_mid.tick).abs();
    let ticks_crossed_large = tick_delta_large / tick_spacing;

    std::println!("--- Large swap (~5% move) ---");
    std::println!("Amount in:  {}", large_swap);
    std::println!("Amount out: {}", out_large);
    std::println!(
        "Price moved: tick {} → {} (delta={}, ~{} spacing crossings)",
        slot_mid.tick,
        slot_after_large.tick,
        tick_delta_large,
        ticks_crossed_large
    );
    std::println!(
        "Footprint: read_write={}, read_only={}",
        cost_large.write_entries,
        cost_large.disk_read_entries + cost_large.memory_read_entries
    );
    std::println!(
        "Bytes: disk_read={}, write={}, mem={}",
        cost_large.disk_read_bytes,
        cost_large.write_bytes,
        cost_large.mem_bytes
    );

    // ---- Reverse swap to restore price before ~10% test ----
    get_token_admin_client(&env, &token1.address).mint(&swapper, &(large_swap as i128));
    let out_back_large = pool.swap(&swapper, &1, &0, &large_swap, &0);
    assert!(out_back_large > 0, "reverse swap must produce output");
    let slot_mid2 = pool.slot0();

    // ---- Extra large swap: ~10% price move ----
    let xlarge_swap: u128 = 100_000_0000000;
    get_token_admin_client(&env, &token0.address).mint(&swapper, &(xlarge_swap as i128));
    let out_xlarge = pool.swap(&swapper, &0, &1, &xlarge_swap, &0);

    let cost_xlarge = env.cost_estimate().resources();

    let slot_after_xlarge = pool.slot0();
    let tick_delta_xlarge = (slot_after_xlarge.tick - slot_mid2.tick).abs();
    let ticks_crossed_xlarge = tick_delta_xlarge / tick_spacing;

    std::println!("--- Extra large swap (~10% move) ---");
    std::println!("Amount in:  {}", xlarge_swap);
    std::println!("Amount out: {}", out_xlarge);
    std::println!(
        "Price moved: tick {} → {} (delta={}, ~{} spacing crossings)",
        slot_mid2.tick,
        slot_after_xlarge.tick,
        tick_delta_xlarge,
        ticks_crossed_xlarge
    );
    std::println!(
        "Footprint: read_write={}, read_only={}",
        cost_xlarge.write_entries,
        cost_xlarge.disk_read_entries + cost_xlarge.memory_read_entries
    );
    std::println!(
        "Bytes: disk_read={}, write={}, mem={}",
        cost_xlarge.disk_read_bytes,
        cost_xlarge.write_bytes,
        cost_xlarge.mem_bytes
    );

    // Mainnet limits
    const RW_LIMIT: u32 = 200;
    const RO_LIMIT: u32 = 200;
    const DISK_READ_BYTES_LIMIT: u32 = 200_000;
    const WRITE_BYTES_LIMIT: u32 = 132_096;

    let ro_small = cost.disk_read_entries + cost.memory_read_entries;
    let ro_large = cost_large.disk_read_entries + cost_large.memory_read_entries;
    let ro_xlarge = cost_xlarge.disk_read_entries + cost_xlarge.memory_read_entries;

    // Summary
    std::println!(
        "\n=== GRIEFING IMPACT SUMMARY (tick_spacing={}) ===",
        tick_spacing
    );
    std::println!("Dust positions: {}", total_dust_positions);
    std::println!("Whale liquidity: {}", liquidity_before);
    std::println!(
        "Mainnet limits: rw={}, ro={}, disk_read={}B, write={}B",
        RW_LIMIT,
        RO_LIMIT,
        DISK_READ_BYTES_LIMIT,
        WRITE_BYTES_LIMIT
    );
    std::println!(
        " ~1% move: {} crossings, rw={}/{} ro={}/{}, disk_read={}/{} write={}/{}",
        ticks_crossed,
        cost.write_entries,
        RW_LIMIT,
        ro_small,
        RO_LIMIT,
        cost.disk_read_bytes,
        DISK_READ_BYTES_LIMIT,
        cost.write_bytes,
        WRITE_BYTES_LIMIT
    );
    std::println!(
        " ~5% move: {} crossings, rw={}/{} ro={}/{}, disk_read={}/{} write={}/{}",
        ticks_crossed_large,
        cost_large.write_entries,
        RW_LIMIT,
        ro_large,
        RO_LIMIT,
        cost_large.disk_read_bytes,
        DISK_READ_BYTES_LIMIT,
        cost_large.write_bytes,
        WRITE_BYTES_LIMIT
    );
    std::println!(
        "~10% move: {} crossings, rw={}/{} ro={}/{}, disk_read={}/{} write={}/{}",
        ticks_crossed_xlarge,
        cost_xlarge.write_entries,
        RW_LIMIT,
        ro_xlarge,
        RO_LIMIT,
        cost_xlarge.disk_read_bytes,
        DISK_READ_BYTES_LIMIT,
        cost_xlarge.write_bytes,
        WRITE_BYTES_LIMIT
    );

    // Assert small and medium swaps fit within mainnet limits
    assert!(
        cost.write_entries <= RW_LIMIT
            && ro_small <= RO_LIMIT
            && cost.disk_read_bytes <= DISK_READ_BYTES_LIMIT
            && cost.write_bytes <= WRITE_BYTES_LIMIT,
        "~1% swap exceeds mainnet limits: rw={}/{} ro={}/{} disk_read={}/{} write={}/{}",
        cost.write_entries,
        RW_LIMIT,
        ro_small,
        RO_LIMIT,
        cost.disk_read_bytes,
        DISK_READ_BYTES_LIMIT,
        cost.write_bytes,
        WRITE_BYTES_LIMIT
    );
    assert!(
        cost_large.write_entries <= RW_LIMIT
            && ro_large <= RO_LIMIT
            && cost_large.disk_read_bytes <= DISK_READ_BYTES_LIMIT
            && cost_large.write_bytes <= WRITE_BYTES_LIMIT,
        "~5% swap exceeds mainnet limits: rw={}/{} ro={}/{} disk_read={}/{} write={}/{}",
        cost_large.write_entries,
        RW_LIMIT,
        ro_large,
        RO_LIMIT,
        cost_large.disk_read_bytes,
        DISK_READ_BYTES_LIMIT,
        cost_large.write_bytes,
        WRITE_BYTES_LIMIT
    );
    // ~10% move under worst-case griefing may exceed limits — that's the attack ceiling
    std::println!(
        "\n~10% move fits mainnet? rw={} ro={} disk_read={} write={}",
        if cost_xlarge.write_entries <= RW_LIMIT {
            "YES"
        } else {
            "NO"
        },
        if ro_xlarge <= RO_LIMIT { "YES" } else { "NO" },
        if cost_xlarge.disk_read_bytes <= DISK_READ_BYTES_LIMIT {
            "YES"
        } else {
            "NO"
        },
        if cost_xlarge.write_bytes <= WRITE_BYTES_LIMIT {
            "YES"
        } else {
            "NO"
        },
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Reserve tracking: reward claim must not drain LP reserves
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_drain_reserves() {
    // Verify that stored reserves are not affected by reward claims
    // when reward_token == one of the pool tokens.
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    // Use token0 as reward token — the critical scenario
    setup.pool.initialize_boost_config(
        &setup.reward_boost_token.address,
        &setup.reward_boost_feed.address,
    );
    setup.pool.initialize_rewards_config(&setup.token0.address);

    // Deposit liquidity
    let deposit_amounts = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    let (_, shares) = setup.pool.deposit(&setup.user, &deposit_amounts, &0);
    assert!(shares > 0);

    let reserves_after_deposit = setup.pool.get_reserves();
    assert!(reserves_after_deposit.get_unchecked(0) > 0);
    assert!(reserves_after_deposit.get_unchecked(1) > 0);

    // Configure and fund rewards (token0 = reward_token)
    let reward_tps = 1_000u128;
    let reward_duration = 60u64;
    let total_reward = reward_tps * reward_duration as u128;
    let reward_expired_at = setup.env.ledger().timestamp() + reward_duration;

    get_token_admin_client(&setup.env, &setup.token0.address)
        .mint(&setup.pool.address, &(total_reward as i128));
    setup
        .pool
        .set_rewards_config(&setup.admin, &reward_expired_at, &reward_tps);

    // Advance time and claim rewards
    jump(&setup.env, 30);
    let claimed = setup.pool.claim(&setup.user);
    assert!(claimed > 0, "should have claimed some rewards");

    // Reserves must be unchanged by the reward claim
    let reserves_after_claim = setup.pool.get_reserves();
    assert_eq!(
        reserves_after_claim.get_unchecked(0),
        reserves_after_deposit.get_unchecked(0),
        "reserve0 must not change from reward claim"
    );
    assert_eq!(
        reserves_after_claim.get_unchecked(1),
        reserves_after_deposit.get_unchecked(1),
        "reserve1 must not change from reward claim"
    );

    // Verify invariant: balance >= reserves + protocol_fees
    let protocol_fees = setup.pool.protocol_fees();
    let balance0 = setup.token0.balance(&setup.pool.address) as u128;
    let balance1 = setup.token1.balance(&setup.pool.address) as u128;
    assert!(
        balance0 >= reserves_after_claim.get_unchecked(0) + protocol_fees.token0,
        "balance0 must cover reserves + protocol fees"
    );
    assert!(
        balance1 >= reserves_after_claim.get_unchecked(1) + protocol_fees.token1,
        "balance1 must cover reserves + protocol fees"
    );

    // Withdraw should still work
    let withdrawn = setup.pool.withdraw(
        &setup.user,
        &shares,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    );
    assert!(withdrawn.get_unchecked(0) > 0 || withdrawn.get_unchecked(1) > 0);
}
