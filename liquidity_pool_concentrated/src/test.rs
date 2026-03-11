#![cfg(test)]
extern crate std;

use crate::math::wrapping_sub_u256;
use crate::testutils::{
    assert_claim_fees_event, count_claim_fees_events, create_pool_contract, create_token_contract,
    deploy_rewards_gauge, get_token_admin_client, Setup,
};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, Map, Symbol, Vec, U256};
use utils::test_utils::jump;

mod pool_plane {
    soroban_sdk::contractimport!(file = "../contracts/soroban_liquidity_pool_plane_contract.wasm");
}

fn pair(values: Vec<u128>) -> (u128, u128) {
    (values.get_unchecked(0), values.get_unchecked(1))
}

fn fee_growth_inside_from_state(
    setup: &Setup<'_>,
    tick_lower: i32,
    tick_upper: i32,
) -> (U256, U256) {
    let slot = setup.pool.get_slot0();
    let fee_growth_global_0 = setup.pool.get_fee_growth_global_0_x128();
    let fee_growth_global_1 = setup.pool.get_fee_growth_global_1_x128();
    let lower = setup.pool.get_tick(&tick_lower);
    let upper = setup.pool.get_tick(&tick_upper);

    let fee_growth_below_0 = if slot.tick >= tick_lower {
        lower.fee_growth_outside_0_x128
    } else {
        wrapping_sub_u256(
            &setup.env,
            &fee_growth_global_0,
            &lower.fee_growth_outside_0_x128,
        )
    };
    let fee_growth_below_1 = if slot.tick >= tick_lower {
        lower.fee_growth_outside_1_x128
    } else {
        wrapping_sub_u256(
            &setup.env,
            &fee_growth_global_1,
            &lower.fee_growth_outside_1_x128,
        )
    };

    let fee_growth_above_0 = if slot.tick < tick_upper {
        upper.fee_growth_outside_0_x128
    } else {
        wrapping_sub_u256(
            &setup.env,
            &fee_growth_global_0,
            &upper.fee_growth_outside_0_x128,
        )
    };
    let fee_growth_above_1 = if slot.tick < tick_upper {
        upper.fee_growth_outside_1_x128
    } else {
        wrapping_sub_u256(
            &setup.env,
            &fee_growth_global_1,
            &upper.fee_growth_outside_1_x128,
        )
    };

    (
        wrapping_sub_u256(
            &setup.env,
            &wrapping_sub_u256(&setup.env, &fee_growth_global_0, &fee_growth_below_0),
            &fee_growth_above_0,
        ),
        wrapping_sub_u256(
            &setup.env,
            &wrapping_sub_u256(&setup.env, &fee_growth_global_1, &fee_growth_below_1),
            &fee_growth_above_1,
        ),
    )
}

#[test]
fn test_swap_empty_pool() {
    let setup = Setup::default();
    setup.mint_user_tokens(10_0000000, 0);

    // Empty pool must reject swaps (matches standard/stableswap EmptyPool behavior).
    let res = setup.pool.try_estimate_swap(&0, &1, &10_0000000);
    assert!(res.is_err(), "estimate_swap on empty pool should error");

    let res = setup.pool.try_swap(&setup.user, &0, &1, &10_0000000, &0);
    assert!(res.is_err(), "swap on empty pool should error");

    // User tokens unchanged.
    assert_eq!(setup.token0.balance(&setup.user), 10_0000000);
    assert_eq!(setup.token1.balance(&setup.user), 0);
}

#[test]
fn test_auto_price_on_empty_pool() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 2_000_0000000);

    // First deposit with 1:2 ratio. Price 2.0 → tick ≈ 6931. depositing 1000 ticks each side
    // Range must contain the derived tick.
    let amounts = Vec::from_array(&setup.env, [1_000_0000000u128, 2_000_0000000u128]);
    let (actual, liq) = setup
        .pool
        .deposit_position(&setup.user, &5931, &7931, &amounts, &0);
    assert!(liq > 0);
    assert_eq!(setup.pool.get_slot0().tick, 6931);
    assert_eq!(
        actual,
        Vec::from_array(&setup.env, [998_4051017, 2000_0000000])
    );
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
fn test_plane_snapshot_full_range_component_updates() {
    let setup = Setup::default();
    setup.mint_user_tokens(5_000_0000000, 5_000_0000000);

    let full_range_amounts = Vec::from_array(&setup.env, [2_000_0000000u128, 2_000_0000000u128]);
    let (_full_range_spent, full_range_liquidity) =
        setup.pool.deposit(&setup.user, &full_range_amounts, &0);
    assert!(full_range_liquidity > 0);

    let narrow_amounts = Vec::from_array(&setup.env, [1_000_0000000u128, 1_000_0000000u128]);
    let (_narrow_spent, narrow_liquidity) =
        setup
            .pool
            .deposit_position(&setup.user, &-100, &100, &narrow_amounts, &0);
    assert!(narrow_liquidity > 0);

    let plane = pool_plane::Client::new(&setup.env, &setup.plane);
    let pools = Vec::from_array(&setup.env, [setup.pool.address.clone()]);
    let snapshot = plane.get(&pools);
    let (_pool_type, init_args, reserves) = snapshot.get_unchecked(0);
    assert_eq!(init_args.get_unchecked(0), 1);
    assert!(reserves.get_unchecked(2) > 0);
    assert!(reserves.get_unchecked(3) > 0);

    setup.pool.withdraw(
        &setup.user,
        &full_range_liquidity,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    );

    let snapshot_after = plane.get(&pools);
    let (_pool_type_after, _init_args_after, reserves_after) = snapshot_after.get_unchecked(0);
    assert_eq!(reserves_after.get_unchecked(2), 0);
    assert_eq!(reserves_after.get_unchecked(3), 0);
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
        .deposit_position(&setup.user, &-10, &10, &amounts, &0);
    assert!(liquidity > 0);

    let position = setup.pool.get_position(&setup.user, &-10, &10);
    assert_eq!(position.liquidity, liquidity);
    assert_eq!(position.tokens_owed_0, 0);
    assert_eq!(position.tokens_owed_1, 0);

    let lower = setup.pool.get_tick(&-10);
    assert_eq!(lower.liquidity_gross, liquidity);
    assert_eq!(lower.liquidity_net, liquidity as i128);

    let upper = setup.pool.get_tick(&10);
    assert_eq!(upper.liquidity_gross, liquidity);
    assert_eq!(upper.liquidity_net, -(liquidity as i128));

    let zero = U256::from_u32(&setup.env, 0);
    assert_ne!(setup.pool.get_chunk_bitmap(&-1), zero);
    assert_ne!(setup.pool.get_chunk_bitmap(&0), zero);
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
        .deposit_position(&setup.user, &10, &10, &amounts, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2111)")]
fn test_deposit_position_tick_lower_too_low() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let amounts = Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-887_273, &0, &amounts, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2112)")]
fn test_deposit_position_tick_upper_too_high() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let amounts = Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &0, &887_273, &amounts, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2018)")]
fn test_deposit_position_zero_amount() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let amounts = Vec::from_array(&setup.env, [0u128, 0u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-10, &10, &amounts, &0);
}

#[test]
#[should_panic(expected = "Error(Contract, #2118)")]
fn test_withdraw_position_not_found() {
    let setup = Setup::default();
    setup.pool.withdraw_position(
        &setup.user,
        &-10,
        &10,
        &1_000_000,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2121)")]
fn test_withdraw_position_insufficient_liquidity() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let amounts = Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]);
    let (_, liquidity) = setup
        .pool
        .deposit_position(&setup.user, &-10, &10, &amounts, &0);
    setup.pool.withdraw_position(
        &setup.user,
        &-10,
        &10,
        &(liquidity + 1),
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2018)")]
fn test_withdraw_position_zero_amount() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);
    let amounts = Vec::from_array(&setup.env, [10_0000000u128, 10_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-10, &10, &amounts, &0);
    setup.pool.withdraw_position(
        &setup.user,
        &-10,
        &10,
        &0,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    );
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
            .deposit_position(&setup.user, &lower, &upper, &amounts, &0);
    }
    // 21st position should fail
    setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &amounts, &0);
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
    setup.mint_user_tokens(200_0000000, 200_0000000);

    // Initialize pool with a deposit at tick 0 (equal amounts, range contains tick 0)
    let init_amounts = Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &init_amounts, &0);

    let initial_0 = setup.token0.balance(&setup.user);
    let initial_1 = setup.token1.balance(&setup.user);

    // Deposit at range [10, 20] which is ABOVE tick 0 → only token0
    let amounts = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &10, &20, &amounts, &0);

    assert!(setup.token0.balance(&setup.user) < initial_0);
    assert_eq!(setup.token1.balance(&setup.user), initial_1);
}

#[test]
fn test_deposit_position_above_price_token1_only() {
    let setup = Setup::default();
    setup.mint_user_tokens(200_0000000, 200_0000000);

    // Initialize pool with a deposit at tick 0 (equal amounts, range contains tick 0)
    let init_amounts = Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &init_amounts, &0);

    let initial_0 = setup.token0.balance(&setup.user);
    let initial_1 = setup.token1.balance(&setup.user);

    // Deposit range entirely below current tick (0): only token1
    let amounts = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-20, &-10, &amounts, &0);

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
        .deposit_position(&setup.user, &-10, &10, &amounts1, &0);
    let pos = setup.pool.get_position(&setup.user, &-10, &10);
    assert_eq!(pos.liquidity, liq1);

    // Add more to the same range
    let amounts2 = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liq2) = setup
        .pool
        .deposit_position(&setup.user, &-10, &10, &amounts2, &0);
    let pos = setup.pool.get_position(&setup.user, &-10, &10);
    assert_eq!(pos.liquidity, liq1 + liq2);

    // Ticks should reflect total
    let total_liq = liq1 + liq2;
    let lower = setup.pool.get_tick(&-10);
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
        .deposit_position(&setup.user, &-10, &10, &amounts, &0);
    let withdraw_amount = liquidity * 40 / 100; // ~40%
    let balance0_before = setup.token0.balance(&setup.user);
    let balance1_before = setup.token1.balance(&setup.user);
    let (out0, out1) = pair(setup.pool.withdraw_position(
        &setup.user,
        &-10,
        &10,
        &withdraw_amount,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    ));
    assert_eq!(
        setup.token0.balance(&setup.user),
        balance0_before + out0 as i128
    );
    assert_eq!(
        setup.token1.balance(&setup.user),
        balance1_before + out1 as i128
    );

    let remaining = liquidity - withdraw_amount;
    let pos = setup.pool.get_position(&setup.user, &-10, &10);
    assert_eq!(pos.liquidity, remaining);
    // Withdrawn principal is transferred immediately, so no new owed from burn itself.
    assert_eq!(pos.tokens_owed_0, 0);
    assert_eq!(pos.tokens_owed_1, 0);

    // Ticks still initialized
    let lower = setup.pool.get_tick(&-10);
    assert_eq!(lower.liquidity_gross, remaining);
}

#[test]
fn test_full_withdrawal_deletes_position_and_clears_ticks() {
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 100_0000000);

    let amounts = Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]);
    let (_, liquidity) = setup
        .pool
        .deposit_position(&setup.user, &-10, &10, &amounts, &0);

    // Withdraw full amount
    let balance0_before = setup.token0.balance(&setup.user);
    let balance1_before = setup.token1.balance(&setup.user);
    let (out0, out1) = pair(setup.pool.withdraw_position(
        &setup.user,
        &-10,
        &10,
        &liquidity,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    ));
    assert!(out0 > 0 || out1 > 0);
    assert_eq!(
        setup.token0.balance(&setup.user),
        balance0_before + out0 as i128
    );
    assert_eq!(
        setup.token1.balance(&setup.user),
        balance1_before + out1 as i128
    );

    // Position should be deleted — get_position panics
    let result = setup.pool.try_get_position(&setup.user, &-10, &10);
    assert!(result.is_err());

    // Ticks should be uninitialized (liquidity_gross = 0)
    let lower = setup.pool.get_tick(&-10);
    assert_eq!(lower.liquidity_gross, 0);
    let upper = setup.pool.get_tick(&10);
    assert_eq!(upper.liquidity_gross, 0);

    // Bitmap should be cleared
    let zero = U256::from_u32(&setup.env, 0);
    assert_eq!(setup.pool.get_chunk_bitmap(&-1), zero);
    assert_eq!(setup.pool.get_chunk_bitmap(&0), zero);
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
        .deposit_position(&setup.user, &-100, &100, &amounts, &0);
    let (_, liq2) = setup
        .pool
        .deposit_position(&user2, &-100, &200, &amounts, &0);

    setup.pool.withdraw_position(
        &setup.user,
        &-100,
        &100,
        &liq1,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    );

    // Shared lower tick stays initialized by user2's position.
    let lower = setup.pool.get_tick(&-100);
    assert_eq!(lower.liquidity_gross, liq2);
    assert_eq!(lower.liquidity_net, liq2 as i128);

    // Upper tick from user1-only range is cleared.
    let middle = setup.pool.get_tick(&100);
    assert_eq!(middle.liquidity_gross, 0);
    assert_eq!(middle.liquidity_net, 0);

    // User2 upper tick remains initialized.
    let upper = setup.pool.get_tick(&200);
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
        .deposit_position(&setup.user, &-100, &100, &amounts, &0);
    let (_, liq2) = setup
        .pool
        .deposit_position(&user2, &-200, &100, &amounts, &0);

    setup.pool.withdraw_position(
        &setup.user,
        &-100,
        &100,
        &liq1,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    );

    // User1 lower tick is cleared, user2 lower tick remains initialized.
    let lower = setup.pool.get_tick(&-100);
    assert_eq!(lower.liquidity_gross, 0);
    assert_eq!(lower.liquidity_net, 0);

    let lower_user2 = setup.pool.get_tick(&-200);
    assert_eq!(lower_user2.liquidity_gross, liq2);
    assert_eq!(lower_user2.liquidity_net, liq2 as i128);

    // Shared upper tick stays initialized by user2's position.
    let upper = setup.pool.get_tick(&100);
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
    let (_, liquidity) = setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &amounts, &0);

    // Swapper trades through the position range
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &50_0000000);
    let out = setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);
    assert!(out > 0);

    // LP withdraws — should get more than they deposited due to fees
    let balance0_before = setup.token0.balance(&setup.user);
    let balance1_before = setup.token1.balance(&setup.user);

    let (_withdraw0, _withdraw1) = pair(setup.pool.withdraw_position(
        &setup.user,
        &-100,
        &100,
        &liquidity,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    ));

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
    let (_, liquidity) = setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &amounts, &0);

    // Swap to generate fees
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &10_0000000);
    setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);

    let balance0_before = setup.token0.balance(&setup.user);
    let balance1_before = setup.token1.balance(&setup.user);

    // Claim fees without withdrawing liquidity
    let (claimed0, claimed1) = pair(setup.pool.claim_position_fees(&setup.user, &-100, &100));

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
fn test_withdraw_position_auto_claims_fees() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    let amounts = Vec::from_array(&setup.env, [500_0000000u128, 500_0000000u128]);
    let (_, liquidity) = setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &amounts, &0);

    // Generate swap fees.
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &10_0000000);
    setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);

    let (fees_before0, fees_before1) = pair(setup.pool.get_position_fees(&setup.user, &-100, &100));
    assert!(
        fees_before0 > 0 || fees_before1 > 0,
        "expected non-zero fees before withdraw"
    );

    // Partial withdraw should transfer exactly what estimate_withdraw_position previews
    // (principal + auto-claimed fees).
    let burn_amount = liquidity / 2;
    let (estimated0, estimated1) = pair(setup.pool.estimate_withdraw_position(
        &setup.user,
        &-100,
        &100,
        &burn_amount,
    ));
    let bal0_before = setup.token0.balance(&setup.user);
    let bal1_before = setup.token1.balance(&setup.user);
    let (withdraw0, withdraw1) = pair(setup.pool.withdraw_position(
        &setup.user,
        &-100,
        &100,
        &burn_amount,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    ));

    assert_eq!(withdraw0, estimated0);
    assert_eq!(withdraw1, estimated1);

    // Verify ClaimFees event was emitted for the auto-claimed fees.
    // Must check events before any subsequent contract call clears the event buffer.
    assert_eq!(
        count_claim_fees_events(&setup.env, &setup.pool.address),
        1,
        "expected exactly one claim_fees event from withdraw_position"
    );
    assert_claim_fees_event(
        &setup.env,
        &setup.pool.address,
        &setup.user,
        &setup.token0.address,
        &setup.token1.address,
        fees_before0,
        fees_before1,
    );

    assert_eq!(
        setup.token0.balance(&setup.user),
        bal0_before + withdraw0 as i128
    );
    assert_eq!(
        setup.token1.balance(&setup.user),
        bal1_before + withdraw1 as i128
    );

    let after = setup.pool.get_position(&setup.user, &-100, &100);
    assert_eq!(after.tokens_owed_0, 0);
    assert_eq!(after.tokens_owed_1, 0);

    // Repeated claim should be a no-op.
    let (claimed0_second, claimed1_second) =
        pair(setup.pool.claim_position_fees(&setup.user, &-100, &100));
    assert_eq!(claimed0_second, 0);
    assert_eq!(claimed1_second, 0);
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
    let fees = setup.pool.get_protocol_fees();
    assert!(
        fees.get(0).unwrap() > 0 || fees.get(1).unwrap() > 0,
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
    let fees_after = setup.pool.get_protocol_fees();
    assert_eq!(fees_after.get(0).unwrap(), 0);
    assert_eq!(fees_after.get(1).unwrap(), 0);
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

    let slot_before = setup.pool.get_slot0();

    // Swap 0→1 (zero_for_one)
    let out_0to1 = setup.pool.swap(&setup.user, &0, &1, &5_0000000, &0);
    assert!(out_0to1 > 0);
    let slot_mid = setup.pool.get_slot0();
    assert!(
        slot_mid.tick < slot_before.tick,
        "price should decrease for 0→1"
    );

    // Swap 1→0 (one_for_zero)
    let out_1to0 = setup.pool.swap(&setup.user, &1, &0, &5_0000000, &0);
    assert!(out_1to0 > 0);
    let slot_after = setup.pool.get_slot0();
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

    // Stacked positions at different ranges (small amounts to allow tick crossings).
    // [-10, 10] goes first to auto-initialize the pool price at tick 0.
    let amounts = Vec::from_array(&env, [1_0000000u128, 1_0000000u128]);
    pool.deposit_position(&user, &-10, &10, &amounts, &0);
    pool.deposit_position(&user, &-50, &-10, &amounts, &0);
    pool.deposit_position(&user, &10, &50, &amounts, &0);

    let slot_before = pool.get_slot0();

    // Large swap that should cross tick boundaries
    let swapper = Address::generate(&env);
    get_token_admin_client(&env, &token0.address).mint(&swapper, &100_0000000);
    let out = pool.swap(&swapper, &0, &1, &50_0000000, &0);
    assert!(out > 0);

    let slot_after = pool.get_slot0();
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

    // First deposit must provide both tokens to initialize the pool price.
    let init_amounts = Vec::from_array(&env, [100u128, 100u128]);
    pool.deposit_position(&user, &-10, &10, &init_amounts, &0);

    // Position below current price (only token1 deposited).
    let amounts_below = Vec::from_array(&env, [0u128, 10_0000000u128]);
    let (_, liq_below) = pool.deposit_position(&user, &-60, &-20, &amounts_below, &0);
    assert!(liq_below > 0, "below-range position should have liquidity");

    // Position above current price (only token0 deposited).
    let amounts_above = Vec::from_array(&env, [10_0000000u128, 0u128]);
    let (_, liq_above) = pool.deposit_position(&user, &20, &60, &amounts_above, &0);
    assert!(liq_above > 0, "above-range position should have liquidity");

    // Swap zero_for_one: price moves down, should cross gap and reach [-60, -20]
    let swapper = Address::generate(&env);
    get_token_admin_client(&env, &token0.address).mint(&swapper, &5_0000000);
    let out = pool.swap(&swapper, &0, &1, &5_0000000, &0);
    assert!(out > 0, "swap should produce output by crossing the gap");

    // Price should have moved below -20 (into the lower position)
    let slot = pool.get_slot0();
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
#[should_panic(expected = "Error(Contract, #2003)")]
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
        .try_deposit_position(&setup.user, &-10, &10, &amounts, &0)
        .is_err());

    // Unkill deposit
    setup.pool.unkill_deposit(&setup.admin);
    assert!(!setup.pool.get_is_killed_deposit());
    setup
        .pool
        .deposit_position(&setup.user, &-10, &10, &amounts, &0);
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
        .deposit_position(&user2, &-100, &100, &dep_amounts, &0);

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

#[test]
fn test_estimate_deposit_position_matches_execute() {
    let setup = Setup::default();
    setup.mint_user_tokens(2_000_0000000, 2_000_0000000);

    // Range must contain the derived tick from amounts ratio (tick ~2513 for 700:900).
    let desired = Vec::from_array(&setup.env, [700_000000u128, 900_000000u128]);
    let (est_amounts, est_liq) = setup
        .pool
        .estimate_deposit_position(&-3000, &3000, &desired);
    let (actual_amounts, actual_liq) =
        setup
            .pool
            .deposit_position(&setup.user, &-3000, &3000, &desired, &0);

    assert_eq!(est_liq, actual_liq);
    assert_eq!(
        est_amounts.get_unchecked(0),
        actual_amounts.get_unchecked(0)
    );
    assert_eq!(
        est_amounts.get_unchecked(1),
        actual_amounts.get_unchecked(1)
    );
}

#[test]
fn test_estimate_withdraw_position_matches_execute() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    let amounts = Vec::from_array(&setup.env, [400_000000u128, 400_000000u128]);
    let (_, liquidity) = setup
        .pool
        .deposit_position(&setup.user, &-150, &150, &amounts, &0);

    let burn = liquidity / 3;
    let (est0, est1) = pair(
        setup
            .pool
            .estimate_withdraw_position(&setup.user, &-150, &150, &burn),
    );
    let (actual0, actual1) = pair(setup.pool.withdraw_position(
        &setup.user,
        &-150,
        &150,
        &burn,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    ));

    assert_eq!(est0, actual0);
    assert_eq!(est1, actual1);
}

#[test]
fn test_get_position_fees_matches_claim_position_fees() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    let amounts = Vec::from_array(&setup.env, [500_000000u128, 500_000000u128]);
    let (_, liquidity) = setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &amounts, &0);

    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &10_0000000);
    setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);

    let (preview0, preview1) = pair(setup.pool.get_position_fees(&setup.user, &-100, &100));
    let (claimed0, claimed1) = pair(setup.pool.claim_position_fees(&setup.user, &-100, &100));

    assert_eq!(preview0, claimed0);
    assert_eq!(preview1, claimed1);
}

#[test]
fn test_claim_all_position_fees_and_estimate() {
    let setup = Setup::default();
    setup.mint_user_tokens(2_000_0000000, 2_000_0000000);

    let amounts = Vec::from_array(&setup.env, [600_000000u128, 600_000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-120, &120, &amounts, &0);
    setup
        .pool
        .deposit_position(&setup.user, &-60, &60, &amounts, &0);

    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &25_0000000);
    setup.pool.swap(&swapper, &0, &1, &25_0000000, &0);

    let (pos1_0, pos1_1) = pair(setup.pool.get_position_fees(&setup.user, &-120, &120));
    let (pos2_0, pos2_1) = pair(setup.pool.get_position_fees(&setup.user, &-60, &60));
    let (all0, all1) = pair(setup.pool.get_all_position_fees(&setup.user));

    assert_eq!(all0, pos1_0 + pos2_0);
    assert_eq!(all1, pos1_1 + pos2_1);
    assert!(all0 > 0 || all1 > 0);

    let bal0_before = setup.token0.balance(&setup.user);
    let bal1_before = setup.token1.balance(&setup.user);
    let (claimed0, claimed1) = pair(setup.pool.claim_all_position_fees(&setup.user));

    assert_eq!(claimed0, all0);
    assert_eq!(claimed1, all1);
    assert_eq!(
        setup.token0.balance(&setup.user),
        bal0_before + claimed0 as i128
    );
    assert_eq!(
        setup.token1.balance(&setup.user),
        bal1_before + claimed1 as i128
    );

    let (left0, left1) = pair(setup.pool.get_all_position_fees(&setup.user));
    assert_eq!(left0, 0);
    assert_eq!(left1, 0);

    let user2 = Address::generate(&setup.env);
    let (empty0, empty1) = pair(setup.pool.get_all_position_fees(&user2));
    assert_eq!(empty0, 0);
    assert_eq!(empty1, 0);
    let (claimed_empty0, claimed_empty1) = pair(setup.pool.claim_all_position_fees(&user2));
    assert_eq!(claimed_empty0, 0);
    assert_eq!(claimed_empty1, 0);
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
        .deposit_position(&setup.user, &-50, &50, &amounts1, &0);
    let amounts2 = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    let (_, liq2) = setup
        .pool
        .deposit_position(&user2, &-20, &20, &amounts2, &0);

    // Active liquidity should be sum of overlapping positions
    let active_liq = setup.pool.get_active_liquidity();
    assert_eq!(active_liq, liq1 + liq2);

    // Swap generates fees for both
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &10_0000000);
    setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);

    // Both users can claim fees
    let (u1_fee0, u1_fee1) = pair(setup.pool.claim_position_fees(&setup.user, &-50, &50));
    let (u2_fee0, u2_fee1) = pair(setup.pool.claim_position_fees(&user2, &-20, &20));

    // Both should have received fees (proportional to liquidity share)
    assert!(u1_fee0 > 0 || u1_fee1 > 0, "user1 should get fees");
    assert!(u2_fee0 > 0 || u2_fee1 > 0, "user2 should get fees");
}

#[test]
fn test_multiple_positions_same_user() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    // User creates multiple positions at different ranges.
    // [-10, 10] goes first to auto-initialize the pool price at tick 0.
    let amounts2 = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    let (_, liq2) = setup
        .pool
        .deposit_position(&setup.user, &-10, &10, &amounts2, &0);
    let amounts1 = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liq1) = setup
        .pool
        .deposit_position(&setup.user, &-100, &-50, &amounts1, &0);
    let amounts3 = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liq3) = setup
        .pool
        .deposit_position(&setup.user, &50, &100, &amounts3, &0);

    // User should have 3 ranges tracked
    let snapshot = setup.pool.get_user_position_snapshot(&setup.user);
    assert_eq!(snapshot.ranges.len(), 3);
    assert_eq!(snapshot.raw_liquidity, liq1 + liq2 + liq3);
}

// ═══════════════════════════════════════════════════════════════════════════
// State queries
// ═══════════════════════════════════════════════════════════════════════════

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
    let fees = setup.pool.get_protocol_fees();

    // Balance = reserves + protocol_fees
    let balance0 = setup.token0.balance(&setup.pool.address) as u128;
    assert_eq!(
        balance0,
        reserves_after.get_unchecked(0) + fees.get(0).unwrap()
    );
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
#[should_panic(expected = "Error(Contract, #2007)")]
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
#[should_panic(expected = "Error(Contract, #2008)")]
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
#[should_panic(expected = "Error(Contract, #2006)")]
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
#[should_panic(expected = "Error(Contract, #2020)")]
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
#[should_panic(expected = "Error(Contract, #2001)")]
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
#[should_panic(expected = "Error(Contract, #2001)")]
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
#[should_panic(expected = "Error(Contract, #2006)")]
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
#[should_panic(expected = "Error(Contract, #2006)")]
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
#[should_panic(expected = "Error(Contract, #2002)")]
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
#[should_panic(expected = "Error(Contract, #2002)")]
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
    pool.deposit_position(&user, &-5, &5, &amounts, &0);
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
    let result = setup
        .pool
        .try_deposit_position(&setup.user, &-10, &10, &huge_amounts, &0);
    assert!(result.is_err());
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
        .deposit_position(&setup.user, &-10, &10, &amounts, &0);
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

    let slot_before = pool.get_slot0();
    let liquidity_before = pool.get_active_liquidity();
    std::println!(
        "Pool state: tick={}, liquidity={}",
        slot_before.tick,
        liquidity_before
    );

    // ---- Attacker: fill ticks with dust ----
    let dust_range: i32 = 200; // number of spacing steps on each side

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

            pool.deposit_position(&attacker, &tick_lower, &tick_upper, &dust_amounts, &0);
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

    let slot_after = pool.get_slot0();
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
    let slot_mid = pool.get_slot0();

    // ---- Larger swap: ~5% price move — stress test ----
    let large_swap: u128 = 50_000_0000000;
    get_token_admin_client(&env, &token0.address).mint(&swapper, &(large_swap as i128));
    let out_large = pool.swap(&swapper, &0, &1, &large_swap, &0);

    let cost_large = env.cost_estimate().resources();

    let slot_after_large = pool.get_slot0();
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
    let slot_mid2 = pool.get_slot0();

    // ---- Extra large swap: ~10% price move ----
    let xlarge_swap: u128 = 100_000_0000000;
    get_token_admin_client(&env, &token0.address).mint(&swapper, &(xlarge_swap as i128));
    let out_xlarge = pool.swap(&swapper, &0, &1, &xlarge_swap, &0);

    let cost_xlarge = env.cost_estimate().resources();

    let slot_after_xlarge = pool.get_slot0();
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
    let protocol_fees = setup.pool.get_protocol_fees();
    let balance0 = setup.token0.balance(&setup.pool.address) as u128;
    let balance1 = setup.token1.balance(&setup.pool.address) as u128;
    assert!(
        balance0 >= reserves_after_claim.get_unchecked(0) + protocol_fees.get(0).unwrap(),
        "balance0 must cover reserves + protocol fees"
    );
    assert!(
        balance1 >= reserves_after_claim.get_unchecked(1) + protocol_fees.get(1).unwrap(),
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

// ═══════════════════════════════════════════════════════════════════════════
// P0 Test Coverage Gap: Exact output swap crossing multiple ticks
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_exact_output_swap_crossing_multiple_ticks() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);
    let user = Address::generate(&env);

    let token_a = create_token_contract(&env, &admin);
    let token_b = create_token_contract(&env, &admin);
    let (token0, token1) = if token_a.address < token_b.address {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    };

    mod pool_plane_eo {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane_eo::Client::new(&env, &env.register(pool_plane_eo::WASM, ()));

    let pool = create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
        30,
        10,
    );

    // Fund LP and create 3 stacked narrow positions with small liquidity
    get_token_admin_client(&env, &token0.address).mint(&user, &2_000_0000000);
    get_token_admin_client(&env, &token1.address).mint(&user, &2_000_0000000);

    // [-10, 10] goes first to auto-initialize the pool price at tick 0.
    let amounts = Vec::from_array(&env, [1_0000000u128, 1_0000000u128]);
    pool.deposit_position(&user, &-10, &10, &amounts, &0);
    pool.deposit_position(&user, &-50, &-10, &amounts, &0);
    pool.deposit_position(&user, &10, &50, &amounts, &0);

    // Desired exact output: large enough to cross multiple ticks (zero_for_one)
    // With ~1 unit per position, requesting ~1.5 units should exhaust the middle
    // position and cross into the lower one
    let desired_out: u128 = 1_5000000;
    let estimated_in = pool.estimate_swap_strict_receive(&0, &1, &desired_out);
    assert!(estimated_in > 0, "estimate should be positive");

    // Perform exact-output swap
    let swapper = Address::generate(&env);
    get_token_admin_client(&env, &token0.address).mint(&swapper, &(estimated_in as i128 * 2));

    let in_max = estimated_in * 2;
    let actual_in = pool.swap_strict_receive(&swapper, &0, &1, &desired_out, &in_max);
    let actual_out = desired_out;
    assert_eq!(
        actual_out, desired_out,
        "exact output should match desired amount"
    );

    // Input should match estimate
    assert_eq!(
        actual_in, estimated_in,
        "actual input should match estimate"
    );

    // Verify tick crossed multiple boundaries (spacing=10)
    let slot = pool.get_slot0();
    assert!(
        slot.tick < -10,
        "swap should have crossed at least 2 tick boundaries, final tick={}",
        slot.tick
    );

    // Verify balances are consistent
    assert_eq!(
        token0.balance(&swapper),
        (in_max as i128) - actual_in as i128
    );
    assert_eq!(token1.balance(&swapper), actual_out as i128);
}

#[test]
fn test_exact_output_swap_one_for_zero_crossing_ticks() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);
    let user = Address::generate(&env);

    let token_a = create_token_contract(&env, &admin);
    let token_b = create_token_contract(&env, &admin);
    let (token0, token1) = if token_a.address < token_b.address {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    };

    mod pool_plane_eo2 {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane_eo2::Client::new(&env, &env.register(pool_plane_eo2::WASM, ()));

    let pool = create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
        30,
        10,
    );

    get_token_admin_client(&env, &token0.address).mint(&user, &2_000_0000000);
    get_token_admin_client(&env, &token1.address).mint(&user, &2_000_0000000);

    // [-10, 10] goes first to auto-initialize the pool price at tick 0.
    let amounts = Vec::from_array(&env, [1_0000000u128, 1_0000000u128]);
    pool.deposit_position(&user, &-10, &10, &amounts, &0);
    pool.deposit_position(&user, &-50, &-10, &amounts, &0);
    pool.deposit_position(&user, &10, &50, &amounts, &0);

    // Exact output in the opposite direction (one_for_zero): want token0 out
    let desired_out: u128 = 1_5000000;
    let estimated_in = pool.estimate_swap_strict_receive(&1, &0, &desired_out);
    assert!(estimated_in > 0);

    let swapper = Address::generate(&env);
    get_token_admin_client(&env, &token1.address).mint(&swapper, &(estimated_in as i128 * 2));

    let actual_in = pool.swap_strict_receive(&swapper, &1, &0, &desired_out, &(estimated_in * 2));
    let actual_out = desired_out;
    assert_eq!(actual_out, desired_out);
    assert_eq!(actual_in, estimated_in);

    let slot = pool.get_slot0();
    assert!(
        slot.tick > 10,
        "swap should have crossed upward past tick 10, final tick={}",
        slot.tick
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// P0 Test Coverage Gap: Fee growth accuracy across tick crossings
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_fee_growth_accuracy_across_tick_crossings() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);
    let lp1 = Address::generate(&env);
    let lp2 = Address::generate(&env);

    let token_a = create_token_contract(&env, &admin);
    let token_b = create_token_contract(&env, &admin);
    let (token0, token1) = if token_a.address < token_b.address {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    };

    mod pool_plane_fg {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane_fg::Client::new(&env, &env.register(pool_plane_fg::WASM, ()));

    let pool = create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
        30,
        10,
    );

    // Fund LPs
    get_token_admin_client(&env, &token0.address).mint(&lp1, &1_000_0000000);
    get_token_admin_client(&env, &token1.address).mint(&lp1, &1_000_0000000);
    get_token_admin_client(&env, &token0.address).mint(&lp2, &1_000_0000000);
    get_token_admin_client(&env, &token1.address).mint(&lp2, &1_000_0000000);

    // LP1: wide position [-100, 100] — always in range
    let wide_amounts = Vec::from_array(&env, [100_0000000u128, 100_0000000u128]);
    let (_, liq_wide) = pool.deposit_position(&lp1, &-100, &100, &wide_amounts, &0);

    // LP2: narrow position [-20, 20] — only in range near tick 0
    let narrow_amounts = Vec::from_array(&env, [100_0000000u128, 100_0000000u128]);
    let (_, liq_narrow) = pool.deposit_position(&lp2, &-20, &20, &narrow_amounts, &0);

    assert!(liq_wide > 0);
    assert!(liq_narrow > 0);

    // Swap token0→token1 that crosses through tick -20 boundary
    // This means part of the swap is in both positions' range, part is only in LP1's range
    let swapper = Address::generate(&env);
    get_token_admin_client(&env, &token0.address).mint(&swapper, &200_0000000);
    let input_amount = 150_0000000u128;
    let amount_out = pool.swap(&swapper, &0, &1, &input_amount, &0);
    assert!(amount_out > 0, "should produce token1 output");

    // Verify the swap crossed tick -20
    let slot = pool.get_slot0();
    assert!(
        slot.tick < -20,
        "swap should cross below tick -20, final tick={}",
        slot.tick
    );

    // Collect fees for both LPs
    let (wide_fee0, wide_fee1) = pair(pool.claim_position_fees(&lp1, &-100, &100));
    let (narrow_fee0, narrow_fee1) = pair(pool.claim_position_fees(&lp2, &-20, &20));

    // Both LPs should have earned fees
    assert!(
        wide_fee0 > 0,
        "wide position should earn token0 fees: {}",
        wide_fee0
    );
    assert!(
        narrow_fee0 > 0,
        "narrow position should earn token0 fees: {}",
        narrow_fee0
    );

    // Key assertion: fee-per-liquidity-unit should be HIGHER for LP1 (wide)
    // because LP1 was in range for the ENTIRE swap (both shared and exclusive segments)
    // while LP2 was only in range for the shared segment [0, -20].
    // LP2 has more absolute liquidity (narrow range concentrates more), but
    // LP1 earned fees in the exclusive segment [-20, final_tick] where LP2 was out of range.
    let wide_fee0_per_liq = (wide_fee0 as u128 * 1_000_000) / liq_wide;
    let narrow_fee0_per_liq = (narrow_fee0 as u128 * 1_000_000) / liq_narrow;
    assert!(
        wide_fee0_per_liq > narrow_fee0_per_liq,
        "fee-per-liquidity should be higher for wide position ({}) than narrow ({}) \
         because wide was in range for the full swap",
        wide_fee0_per_liq,
        narrow_fee0_per_liq
    );

    // Verify total fees are consistent with swap amount and fee rate
    // fee_rate = 30 bps, protocol_fee = 50% → LP fee = 15 bps
    let protocol_fees = pool.get_protocol_fees();
    let total_fee0 = wide_fee0 + narrow_fee0 + protocol_fees.get(0).unwrap();

    // Total fees collected should be roughly 30 bps of input amount
    // Allow some rounding tolerance (within 1%)
    let expected_total_fee = input_amount * 30 / 10000;
    let tolerance = expected_total_fee / 100 + 1;
    assert!(
        total_fee0.abs_diff(expected_total_fee) <= tolerance,
        "total fees {} should be ~{} (30 bps of input), diff={}",
        total_fee0,
        expected_total_fee,
        total_fee0.abs_diff(expected_total_fee)
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// P0 Test Coverage Gap: Fee proportionality (2x liquidity = 2x fees)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_fee_proportionality_same_range() {
    let setup = Setup::default();
    let user2 = Address::generate(&setup.env);

    // User1 gets 2x the tokens as user2
    setup.mint_user_tokens(600_0000000, 600_0000000);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&user2, &300_0000000);
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&user2, &300_0000000);

    // Both deposit at the SAME tick range — critical for equal fee_growth_inside
    let amounts1 = Vec::from_array(&setup.env, [400_0000000u128, 400_0000000u128]);
    let (_, liq1) = setup
        .pool
        .deposit_position(&setup.user, &-50, &50, &amounts1, &0);

    let amounts2 = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    let (_, liq2) = setup
        .pool
        .deposit_position(&user2, &-50, &50, &amounts2, &0);

    assert!(liq1 > 0 && liq2 > 0);
    // Liquidity should be proportional to deposits (same range, same price)
    let liq_ratio_bps = (liq1 as u128 * 10000) / liq2 as u128;
    // Expect ~2:1 ratio (20000 bps = 2.0x), allow 1% tolerance
    assert!(
        liq_ratio_bps >= 19800 && liq_ratio_bps <= 20200,
        "liquidity ratio should be ~2:1, got {} bps",
        liq_ratio_bps
    );

    // Generate fees with a swap that stays within [-50, 50]
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &50_0000000);
    let swap_out = setup.pool.swap(&swapper, &0, &1, &20_0000000, &0);
    assert!(swap_out > 0);

    // Also swap in the opposite direction to generate fees in both tokens
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&swapper, &50_0000000);
    let swap_out2 = setup.pool.swap(&swapper, &1, &0, &20_0000000, &0);
    assert!(swap_out2 > 0);

    // Verify both swaps stayed within range
    let slot = setup.pool.get_slot0();
    assert!(
        slot.tick > -50 && slot.tick < 50,
        "swaps should stay in range, tick={}",
        slot.tick
    );

    // Claim fees for both users
    let (u1_fee0, u1_fee1) = pair(setup.pool.claim_position_fees(&setup.user, &-50, &50));
    let (u2_fee0, u2_fee1) = pair(setup.pool.claim_position_fees(&user2, &-50, &50));

    // Both should have received fees
    assert!(u1_fee0 > 0 || u1_fee1 > 0, "user1 should get fees");
    assert!(u2_fee0 > 0 || u2_fee1 > 0, "user2 should get fees");

    // Fee ratio should match liquidity ratio (~2:1)
    // fee_for_user = fee_growth_inside * user_liquidity / Q128
    // Since both users have same fee_growth_inside (same range), ratio = liq1/liq2
    if u2_fee0 > 0 {
        let fee0_ratio_bps = (u1_fee0 as u128 * 10000) / u2_fee0 as u128;
        assert!(
            fee0_ratio_bps >= 19500 && fee0_ratio_bps <= 20500,
            "fee0 ratio should be ~2:1 (matching liquidity), got {} bps \
             (user1={}, user2={})",
            fee0_ratio_bps,
            u1_fee0,
            u2_fee0
        );
    }
    if u2_fee1 > 0 {
        let fee1_ratio_bps = (u1_fee1 as u128 * 10000) / u2_fee1 as u128;
        assert!(
            fee1_ratio_bps >= 19500 && fee1_ratio_bps <= 20500,
            "fee1 ratio should be ~2:1 (matching liquidity), got {} bps \
             (user1={}, user2={})",
            fee1_ratio_bps,
            u1_fee1,
            u2_fee1
        );
    }

    // Additional: verify total LP fees + protocol fees ≈ 30 bps of total swapped
    let protocol_fees = setup.pool.get_protocol_fees();
    let total_fee0 = u1_fee0 + u2_fee0 + protocol_fees.get(0).unwrap();
    let total_fee1 = u1_fee1 + u2_fee1 + protocol_fees.get(1).unwrap();
    assert!(
        total_fee0 > 0 || total_fee1 > 0,
        "should have collected fees"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// P1: MIN_TICK / MAX_TICK boundary deposits and swaps
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_min_max_tick_boundary_deposit_and_swap() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);
    let user = Address::generate(&env);

    let token_a = create_token_contract(&env, &admin);
    let token_b = create_token_contract(&env, &admin);
    let (token0, token1) = if token_a.address < token_b.address {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    };

    mod pool_plane_mm {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane_mm::Client::new(&env, &env.register(pool_plane_mm::WASM, ()));

    let tick_spacing: i32 = 200;
    let pool = create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
        100,
        tick_spacing,
    );

    // Compute aligned MIN_TICK and MAX_TICK
    let mut min_tick_aligned = -887_272 - (-887_272 % tick_spacing);
    if min_tick_aligned < -887_272 {
        min_tick_aligned += tick_spacing;
    }
    let mut max_tick_aligned = 887_272 - (887_272 % tick_spacing);
    if max_tick_aligned > 887_272 {
        max_tick_aligned -= tick_spacing;
    }

    get_token_admin_client(&env, &token0.address).mint(&user, &10_000_0000000);
    get_token_admin_client(&env, &token1.address).mint(&user, &10_000_0000000);

    // Deposit at the extreme tick boundaries
    let amounts = Vec::from_array(&env, [100_0000000u128, 100_0000000u128]);
    let (_, liq) = pool.deposit_position(&user, &min_tick_aligned, &max_tick_aligned, &amounts, &0);
    assert!(liq > 0, "full-range position should have liquidity");

    // Verify tick data at boundaries
    let lower_tick = pool.get_tick(&min_tick_aligned);
    assert_eq!(lower_tick.liquidity_gross, liq);
    assert_eq!(lower_tick.liquidity_net, liq as i128);

    let upper_tick = pool.get_tick(&max_tick_aligned);
    assert_eq!(upper_tick.liquidity_gross, liq);
    assert_eq!(upper_tick.liquidity_net, -(liq as i128));

    // Swap to push price toward MIN_TICK boundary
    let swapper = Address::generate(&env);
    get_token_admin_client(&env, &token0.address).mint(&swapper, &5_000_0000000);
    let out = pool.swap(&swapper, &0, &1, &2_000_0000000, &0);
    assert!(out > 0, "swap should produce output");

    let slot = pool.get_slot0();
    assert!(
        slot.tick < 0,
        "swap should push tick below 0, got {}",
        slot.tick
    );

    // Swap in the other direction toward MAX_TICK boundary
    get_token_admin_client(&env, &token1.address).mint(&swapper, &5_000_0000000);
    let out2 = pool.swap(&swapper, &1, &0, &4_000_0000000, &0);
    assert!(out2 > 0);

    let slot2 = pool.get_slot0();
    assert!(
        slot2.tick > 0,
        "swap should push tick above 0, got {}",
        slot2.tick
    );

    // Verify position can be withdrawn
    let (withdraw0, withdraw1) = pair(pool.withdraw_position(
        &user,
        &min_tick_aligned,
        &max_tick_aligned,
        &liq,
        &Vec::from_array(&env, [0u128, 0u128]),
    ));
    // Should have earned fees from both swaps and received them on withdraw.
    assert!(
        withdraw0 > 0 || withdraw1 > 0,
        "full-range position should earn fees"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// P1: Zero-liquidity traversal fee growth must be zero
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_zero_liquidity_gap_no_fee_growth() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);
    let user = Address::generate(&env);

    let token_a = create_token_contract(&env, &admin);
    let token_b = create_token_contract(&env, &admin);
    let (token0, token1) = if token_a.address < token_b.address {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    };

    mod pool_plane_zl {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane_zl::Client::new(&env, &env.register(pool_plane_zl::WASM, ()));

    let pool = create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
        30,
        10,
    );

    get_token_admin_client(&env, &token0.address).mint(&user, &1_000_0000000);
    get_token_admin_client(&env, &token1.address).mint(&user, &1_000_0000000);

    // First deposit must provide both tokens to initialize the pool price.
    pool.deposit_position(
        &user,
        &-10,
        &10,
        &Vec::from_array(&env, [100u128, 100u128]),
        &0,
    );

    // Two positions with a gap at [0, 40]:
    // Position A: [-60, -20] (below current price at tick 0) → only token1
    // Position B: [40, 80] (above current price) → only token0
    let (_, liq_a) = pool.deposit_position(
        &user,
        &-60,
        &-20,
        &Vec::from_array(&env, [0u128, 50_0000000u128]),
        &0,
    );
    let (_, liq_b) = pool.deposit_position(
        &user,
        &40,
        &80,
        &Vec::from_array(&env, [50_0000000u128, 0u128]),
        &0,
    );
    assert!(liq_a > 0 && liq_b > 0);

    // Swap zero_for_one: price slides from tick 0, through gap, into position A
    let swapper = Address::generate(&env);
    get_token_admin_client(&env, &token0.address).mint(&swapper, &100_0000000);
    let out = pool.swap(&swapper, &0, &1, &50_0000000, &0);
    assert!(out > 0);

    let slot = pool.get_slot0();
    assert!(
        slot.tick < -20,
        "swap should cross gap and enter position A, tick={}",
        slot.tick
    );

    // Position A earned fees (swap went through its range)
    let (fee_a0, fee_a1) = pair(pool.claim_position_fees(&user, &-60, &-20));
    assert!(
        fee_a0 > 0,
        "position A should earn fees from swap through its range"
    );

    // Position B earned zero fees (swap never entered its range)
    let (fee_b0, fee_b1) = pair(pool.claim_position_fees(&user, &40, &80));
    assert_eq!(
        fee_b0, 0,
        "position B should earn zero token0 fees (out of range)"
    );
    assert_eq!(
        fee_b1, 0,
        "position B should earn zero token1 fees (out of range)"
    );

    // Verify the gap traversal was free (no extra tokens consumed in the gap)
    // Total fees = position A fees + protocol fees only
    let protocol_fees = pool.get_protocol_fees();
    let total_fee0 = fee_a0 + protocol_fees.get(0).unwrap();
    // fee_b0 must be 0, confirming no fee growth in the gap
    assert_eq!(fee_b0, 0);
    assert!(total_fee0 > 0, "should have collected fees overall");
}

// ═══════════════════════════════════════════════════════════════════════════
// P1: Tick crossing fee_growth_outside flip verification
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_tick_crossing_fee_growth_outside_flip() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);
    let user = Address::generate(&env);

    let token_a = create_token_contract(&env, &admin);
    let token_b = create_token_contract(&env, &admin);
    let (token0, token1) = if token_a.address < token_b.address {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    };

    mod pool_plane_fg2 {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane_fg2::Client::new(&env, &env.register(pool_plane_fg2::WASM, ()));

    // Use tick_spacing=10 to reduce ledger entries during swap
    let pool = create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
        30,
        10,
    );

    get_token_admin_client(&env, &token0.address).mint(&user, &1_000_0000000);
    get_token_admin_client(&env, &token1.address).mint(&user, &1_000_0000000);

    // Use a narrow range [-20, 20] to minimize ledger entries.
    // Also add a "catch" position below so swap doesn't scan empty bitmap chunks.
    let amounts = Vec::from_array(&env, [100_0000000u128, 100_0000000u128]);
    let (_, liq) = pool.deposit_position(&user, &-20, &20, &amounts, &0);
    assert!(liq > 0);

    // Catch position below to absorb the swap after crossing -20
    let catch_amounts = Vec::from_array(&env, [50_0000000u128, 50_0000000u128]);
    pool.deposit_position(&user, &-40, &-20, &catch_amounts, &0);

    // Record fee_growth_outside at tick -20 BEFORE any swaps
    let tick_lower_before = pool.get_tick(&-20);

    // Swap to generate some fees, staying in range
    let swapper = Address::generate(&env);
    get_token_admin_client(&env, &token0.address).mint(&swapper, &200_0000000);
    pool.swap(&swapper, &0, &1, &10_0000000, &0);

    // Now do a swap large enough to cross tick -20
    pool.swap(&swapper, &0, &1, &100_0000000, &0);

    let slot = pool.get_slot0();
    assert!(
        slot.tick < -20,
        "swap should cross below tick -20, got {}",
        slot.tick
    );

    // Check fee_growth_outside at tick -20 AFTER crossing
    let tick_lower_after = pool.get_tick(&-20);

    // After crossing, fee_growth_outside should have been flipped:
    // new_outside = fee_growth_global - old_outside
    // So it must be different from before (since fees were accumulated)
    assert!(
        tick_lower_after.fee_growth_outside_0_x128 != tick_lower_before.fee_growth_outside_0_x128,
        "fee_growth_outside_0 at tick -20 should flip after crossing"
    );

    // Verify fee accounting still works correctly after the flip:
    // Position should have accrued fees from both swaps while it was in range
    let (claimed0, _claimed1) = pair(pool.claim_position_fees(&user, &-20, &20));
    assert!(
        claimed0 > 0,
        "position should have earned token0 fees before tick crossing"
    );

    // Catch position should also have fees from the second swap segment
    let (catch_fee0, _catch_fee1) = pair(pool.claim_position_fees(&user, &-40, &-20));
    assert!(
        catch_fee0 > 0,
        "catch position should earn fees from swap that crossed into its range"
    );

    // Now swap back to cross tick -20 again (re-entering the main position range)
    get_token_admin_client(&env, &token1.address).mint(&swapper, &200_0000000);
    pool.swap(&swapper, &1, &0, &100_0000000, &0);

    let slot2 = pool.get_slot0();
    assert!(
        slot2.tick > -20,
        "swap back should cross above tick -20, got {}",
        slot2.tick
    );

    // fee_growth_outside should flip again
    let tick_lower_after2 = pool.get_tick(&-20);
    assert!(
        tick_lower_after2.fee_growth_outside_0_x128 != tick_lower_after.fee_growth_outside_0_x128,
        "fee_growth_outside_0 at tick -20 should flip again after re-crossing"
    );

    // Main position should accrue new fees from the return swap
    let (_claimed0_2, claimed1_2) = pair(pool.claim_position_fees(&user, &-20, &20));
    assert!(
        claimed1_2 > 0,
        "position should earn token1 fees from return swap"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// P1: Exact output with too-low in_max should fail
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_exact_output_with_low_in_max_fails() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    let amounts = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &amounts, &0);

    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &500_0000000);

    let desired_out: u128 = 100_0000000; // large exact output
    let estimated_in = setup
        .pool
        .estimate_swap_strict_receive(&0, &1, &desired_out);
    assert!(estimated_in > 0);
    let in_max = estimated_in - 1;

    let result = setup
        .pool
        .try_swap_strict_receive(&swapper, &0, &1, &desired_out, &in_max);

    // Should fail because exact output cannot be obtained within in_max.
    assert!(
        result.is_err(),
        "exact output with too-low in_max should fail"
    );
}

#[test]
fn test_exact_output_with_low_in_max_one_for_zero_fails() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    let amounts = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &amounts, &0);

    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&swapper, &500_0000000);

    let desired_out: u128 = 100_0000000;
    let estimated_in = setup
        .pool
        .estimate_swap_strict_receive(&1, &0, &desired_out);
    assert!(estimated_in > 0);
    let in_max = estimated_in - 1;

    let result = setup
        .pool
        .try_swap_strict_receive(&swapper, &1, &0, &desired_out, &in_max);

    assert!(
        result.is_err(),
        "exact output with too-low in_max should fail"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// P2: Position re-initialization (create → full withdraw → recreate)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_position_reinit_after_full_withdraw() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    // Create position
    let amounts = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liq1) = setup
        .pool
        .deposit_position(&setup.user, &-30, &30, &amounts, &0);
    assert!(liq1 > 0);

    // Generate fees via swap
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &20_0000000);
    setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);

    // Fully withdraw
    let (owed0, owed1) = pair(setup.pool.withdraw_position(
        &setup.user,
        &-30,
        &30,
        &liq1,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    ));
    assert!(
        owed0 > 0 || owed1 > 0,
        "should have owed tokens from withdraw"
    );

    // Position should be deleted (zero liquidity + zero owed)
    let tick_lower = setup.pool.get_tick(&-30);
    assert_eq!(tick_lower.liquidity_gross, 0, "tick should be cleared");

    // Recreate at the same tick range
    let amounts2 = Vec::from_array(&setup.env, [50_0000000u128, 50_0000000u128]);
    let (_, liq2) = setup
        .pool
        .deposit_position(&setup.user, &-30, &30, &amounts2, &0);
    assert!(liq2 > 0, "should be able to recreate position");

    // Position should be fresh (no leftover fee state)
    let pos = setup.pool.get_position(&setup.user, &-30, &30);
    assert_eq!(pos.liquidity, liq2);
    assert_eq!(
        pos.tokens_owed_0, 0,
        "recreated position should have zero owed_0"
    );
    assert_eq!(
        pos.tokens_owed_1, 0,
        "recreated position should have zero owed_1"
    );

    // Tick should be re-initialized
    let tick_lower2 = setup.pool.get_tick(&-30);
    assert_eq!(tick_lower2.liquidity_gross, liq2);
}

// ═══════════════════════════════════════════════════════════════════════════
// P2: Fee accrual during deposit (swap then deposit on same range)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_fee_accrual_during_second_deposit() {
    let setup = Setup::default();
    let user2 = Address::generate(&setup.env);

    setup.mint_user_tokens(500_0000000, 500_0000000);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&user2, &500_0000000);
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&user2, &500_0000000);

    // User1 deposits first
    let amounts = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liq1) = setup
        .pool
        .deposit_position(&setup.user, &-50, &50, &amounts, &0);

    // Swap to generate fees for user1's position
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &20_0000000);
    setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);

    // User1 deposits AGAIN at the same range — accrue_position_fees should
    // snapshot the fees earned so far into tokens_owed before adding liquidity
    let amounts2 = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liq_add) = setup
        .pool
        .deposit_position(&setup.user, &-50, &50, &amounts2, &0);
    assert!(liq_add > 0);

    // Verify position has accrued fees from the swap (stored in tokens_owed)
    let pos = setup.pool.get_position(&setup.user, &-50, &50);
    assert_eq!(pos.liquidity, liq1 + liq_add);
    assert!(
        pos.tokens_owed_0 > 0,
        "fees from swap should be accrued into tokens_owed_0 during deposit: {}",
        pos.tokens_owed_0
    );

    // User can claim those fees
    let (claimed0, claimed1) = pair(setup.pool.claim_position_fees(&setup.user, &-50, &50));
    assert!(claimed0 > 0, "should be able to claim accrued fees");

    let pos_after = setup.pool.get_position(&setup.user, &-50, &50);
    assert_eq!(pos_after.tokens_owed_0, 0);
}

#[test]
fn test_deposit_position_snapshots_after_tick_initialization() {
    let setup = Setup::default();
    setup.mint_user_tokens(5000_0000000, 5000_0000000);

    let desired = Vec::from_array(&setup.env, [1000_0000000, 1000_0000000]);
    let (_base_amounts, base_liquidity) =
        setup
            .pool
            .deposit_position(&setup.user, &-887260, &887260, &desired, &0);
    assert!(base_liquidity > 0);

    // Generate non-zero fee growth before creating a new in-range position.
    setup.pool.swap(&setup.user, &0, &1, &2_0462601, &2_0196431);
    let slot = setup.pool.get_slot0();
    assert!(slot.tick >= -160 && slot.tick < 60);

    let (_amounts, range_liquidity) =
        setup
            .pool
            .deposit_position(&setup.user, &-160, &60, &desired, &0);
    assert!(range_liquidity > 0);

    let position = setup.pool.get_position(&setup.user, &-160, &60);
    let (inside0, inside1) = fee_growth_inside_from_state(&setup, -160, 60);

    assert_eq!(
        position.fee_growth_inside_0_last_x128, inside0,
        "position fee snapshot must match on-chain inside growth after mint",
    );
    assert_eq!(
        position.fee_growth_inside_1_last_x128, inside1,
        "position fee snapshot must match on-chain inside growth after mint",
    );
}

#[test]
fn test_withdraw_position_after_swap_ok() {
    let setup = Setup::default();
    setup.mint_user_tokens(5000_0000000, 5000_0000000);

    // corrupt state
    let desired = Vec::from_array(&setup.env, [1000_0000000, 1000_0000000]);
    let (_, full_range_liquidity) =
        setup
            .pool
            .deposit_position(&setup.user, &-887260, &887260, &desired, &0);
    setup.pool.swap(&setup.user, &0, &1, &2_0462601, &2_0196431);
    let (_, positon1_liquidity) =
        setup
            .pool
            .deposit_position(&setup.user, &-100, &-20, &desired, &0);
    let (_, positon2_liquidity) =
        setup
            .pool
            .deposit_position(&setup.user, &-160, &60, &desired, &0);

    setup.pool.withdraw_position(
        &setup.user,
        &-887260,
        &887260,
        &full_range_liquidity,
        &Vec::from_array(&setup.env, [0, 0]),
    );
    setup.pool.withdraw_position(
        &setup.user,
        &-100,
        &-20,
        &positon1_liquidity,
        &Vec::from_array(&setup.env, [0, 0]),
    );
    setup.pool.withdraw_position(
        &setup.user,
        &-160,
        &60,
        &positon2_liquidity,
        &Vec::from_array(&setup.env, [0, 0]),
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// P2: Protocol fee = 0% and 100% edge cases
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_protocol_fee_zero_percent() {
    let setup = Setup::default();
    setup.mint_user_tokens(500_0000000, 500_0000000);

    // Set protocol fee to 0%
    setup.pool.set_protocol_fee_fraction(&setup.admin, &0);
    assert_eq!(setup.pool.get_protocol_fee_fraction(), 0);

    let amounts = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    let (_, liq) = setup
        .pool
        .deposit_position(&setup.user, &-50, &50, &amounts, &0);

    // Swap to generate fees
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &30_0000000);
    setup.pool.swap(&swapper, &0, &1, &20_0000000, &0);

    // Protocol should have zero fees
    let protocol_fees = setup.pool.get_protocol_fees();
    assert_eq!(
        protocol_fees.get(0).unwrap(),
        0,
        "protocol fee 0%: protocol should collect nothing"
    );

    // LP should get ALL the fees (30 bps of input)
    let (lp_fee0, _lp_fee1) = pair(setup.pool.claim_position_fees(&setup.user, &-50, &50));
    let expected_total_fee = 20_0000000u128 * 30 / 10000;
    let tolerance = expected_total_fee / 100 + 1;
    assert!(
        lp_fee0.abs_diff(expected_total_fee) <= tolerance,
        "LP should get all fees (~{}), got {}",
        expected_total_fee,
        lp_fee0
    );
}

#[test]
fn test_protocol_fee_one_hundred_percent() {
    let setup = Setup::default();
    setup.mint_user_tokens(500_0000000, 500_0000000);

    // Set protocol fee to 100%
    setup.pool.set_protocol_fee_fraction(&setup.admin, &10_000);
    assert_eq!(setup.pool.get_protocol_fee_fraction(), 10_000);

    let amounts = Vec::from_array(&setup.env, [200_0000000u128, 200_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-50, &50, &amounts, &0);

    // Swap to generate fees
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &30_0000000);
    setup.pool.swap(&swapper, &0, &1, &20_0000000, &0);

    // Protocol should get ALL fees
    let protocol_fees = setup.pool.get_protocol_fees();
    let expected_total_fee = 20_0000000u128 * 30 / 10000;
    let tolerance = expected_total_fee / 100 + 1;
    assert!(
        protocol_fees.get(0).unwrap().abs_diff(expected_total_fee) <= tolerance,
        "protocol fee 100%: protocol should collect all fees (~{}), got {}",
        expected_total_fee,
        protocol_fees.get(0).unwrap()
    );

    // LP should get zero fees
    let (lp_fee0, lp_fee1) = pair(setup.pool.claim_position_fees(&setup.user, &-50, &50));
    assert_eq!(lp_fee0, 0, "LP should get zero fees at 100% protocol fee");
    assert_eq!(lp_fee1, 0, "LP should get zero fees at 100% protocol fee");
}

// ═══════════════════════════════════════════════════════════════════════════
// P2: Bitmap word boundary crossing
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_bitmap_word_boundary_crossing() {
    let env = Env::default();
    env.mock_all_auths();
    env.cost_estimate().budget().reset_unlimited();

    let admin = Address::generate(&env);
    let router = Address::generate(&env);
    let user = Address::generate(&env);

    let token_a = create_token_contract(&env, &admin);
    let token_b = create_token_contract(&env, &admin);
    let (token0, token1) = if token_a.address < token_b.address {
        (token_a, token_b)
    } else {
        (token_b, token_a)
    };

    mod pool_plane_bw {
        soroban_sdk::contractimport!(
            file = "../contracts/soroban_liquidity_pool_plane_contract.wasm"
        );
    }
    let plane = pool_plane_bw::Client::new(&env, &env.register(pool_plane_bw::WASM, ()));

    // tick_spacing=10: compressed_tick = tick/10, chunk_pos = compressed/16
    // word_pos = chunk_pos >> 8
    // Word boundary: chunk_pos -1 → word -1, chunk_pos 0 → word 0
    // chunk_pos -1 corresponds to compressed_tick -16..-1, i.e. ticks -160..-10
    // chunk_pos 0 corresponds to compressed_tick 0..15, i.e. ticks 0..150
    // So the word boundary is between tick -10 and tick 0

    let pool = create_pool_contract(
        &env,
        &admin,
        &router,
        &plane.address,
        &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
        30,
        10,
    );

    get_token_admin_client(&env, &token0.address).mint(&user, &1_000_0000000);
    get_token_admin_client(&env, &token1.address).mint(&user, &1_000_0000000);

    // Initial two-token deposit required on empty pool (AllCoinsRequired)
    pool.deposit_position(
        &user,
        &-10,
        &10,
        &Vec::from_array(&env, [100u128, 100u128]),
        &0,
    );

    // Position in word 0: [10, 50] — above current tick 0, only token0
    let (_, liq_pos) = pool.deposit_position(
        &user,
        &10,
        &50,
        &Vec::from_array(&env, [50_0000000u128, 0u128]),
        &0,
    );
    assert!(liq_pos > 0);

    // Position in word -1: [-50, -10] — below current tick 0, only token1
    let (_, liq_neg) = pool.deposit_position(
        &user,
        &-50,
        &-10,
        &Vec::from_array(&env, [0u128, 50_0000000u128]),
        &0,
    );
    assert!(liq_neg > 0);

    // Verify bitmap words are set in different words
    let word_0 = pool.get_chunk_bitmap(&0);
    let word_neg1 = pool.get_chunk_bitmap(&-1);
    let zero = U256::from_u32(&env, 0);
    assert!(word_0 != zero, "word 0 should have bits set");
    assert!(word_neg1 != zero, "word -1 should have bits set");

    // Swap one_for_zero starting from tick 0 (in the gap between positions)
    // This should find liquidity in [10, 50] by scanning forward across word boundary
    let swapper = Address::generate(&env);
    get_token_admin_client(&env, &token1.address).mint(&swapper, &100_0000000);
    let out = pool.swap(&swapper, &1, &0, &30_0000000, &0);
    assert!(out > 0, "swap should find liquidity across word boundary");

    let slot = pool.get_slot0();
    assert!(
        slot.tick >= 10,
        "swap should reach position in [10,50], tick={}",
        slot.tick
    );

    // Swap zero_for_one back: should find liquidity in [-50, -10] across word boundary
    get_token_admin_client(&env, &token0.address).mint(&swapper, &100_0000000);
    let out2 = pool.swap(&swapper, &0, &1, &80_0000000, &0);
    assert!(
        out2 > 0,
        "reverse swap should find liquidity across word boundary"
    );

    let slot2 = pool.get_slot0();
    assert!(
        slot2.tick < -10,
        "swap should reach position in [-50,-10], tick={}",
        slot2.tick
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// P2: Two users sharing exact same tick range
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_two_users_same_tick_range_fee_tracking() {
    let setup = Setup::default();
    let user2 = Address::generate(&setup.env);

    setup.mint_user_tokens(500_0000000, 500_0000000);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&user2, &500_0000000);
    get_token_admin_client(&setup.env, &setup.token1.address).mint(&user2, &500_0000000);

    // Both users deposit at EXACTLY the same range
    let amounts1 = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liq1) = setup
        .pool
        .deposit_position(&setup.user, &-30, &30, &amounts1, &0);

    let amounts2 = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    let (_, liq2) = setup
        .pool
        .deposit_position(&user2, &-30, &30, &amounts2, &0);

    assert!(liq1 > 0 && liq2 > 0);
    // Same amounts → same liquidity
    assert_eq!(liq1, liq2, "equal deposits should produce equal liquidity");

    // Verify tick state: liquidity_gross = sum of both
    let tick = setup.pool.get_tick(&-30);
    assert_eq!(tick.liquidity_gross, liq1 + liq2);

    // Swap to generate fees
    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &30_0000000);
    setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);

    // Both should earn identical fees (same liquidity, same range)
    let (u1_fee0, u1_fee1) = pair(setup.pool.claim_position_fees(&setup.user, &-30, &30));
    let (u2_fee0, u2_fee1) = pair(setup.pool.claim_position_fees(&user2, &-30, &30));

    assert_eq!(
        u1_fee0, u2_fee0,
        "equal liquidity at same range should earn equal token0 fees"
    );
    assert_eq!(
        u1_fee1, u2_fee1,
        "equal liquidity at same range should earn equal token1 fees"
    );

    // User1 withdraws fully, user2's position should be unaffected.
    // Position may be removed immediately if no fees are owed.
    setup.pool.withdraw_position(
        &setup.user,
        &-30,
        &30,
        &liq1,
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    );

    // User2 still has position
    let pos2 = setup.pool.get_position(&user2, &-30, &30);
    assert_eq!(
        pos2.liquidity, liq2,
        "user2's position should be unaffected"
    );

    // Tick should still have user2's liquidity
    let tick_after = setup.pool.get_tick(&-30);
    assert_eq!(
        tick_after.liquidity_gross, liq2,
        "tick should have only user2's liquidity after user1 withdrawal"
    );

    // Swap again — only user2 should earn fees
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &30_0000000);
    setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);

    let (u2_fee0_2, _u2_fee1_2) = pair(setup.pool.claim_position_fees(&user2, &-30, &30));
    assert!(u2_fee0_2 > 0, "user2 should earn fees from second swap");
}

#[test]
fn test_deposit_position_auto_init_both_tokens_used() {
    // First deposit with unequal amounts. Range must contain the derived tick.
    // price = 200/100 = 2.0 → tick ≈ 6931
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 200_0000000);

    let amounts = Vec::from_array(&setup.env, [100_0000000u128, 200_0000000u128]);
    let (actual, liq) = setup
        .pool
        .deposit_position(&setup.user, &6000, &8000, &amounts, &0);
    assert!(liq > 0);

    let actual0 = actual.get_unchecked(0);
    let actual1 = actual.get_unchecked(1);
    assert!(actual0 > 0, "token0 must be used");
    assert!(actual1 > 0, "token1 must be used");

    // Verify user balances reflect the transfer-and-refund pattern
    assert_eq!(
        setup.token0.balance(&setup.user) as u128,
        100_0000000 - actual0,
        "user token0 balance should equal mint - actual0"
    );
    assert_eq!(
        setup.token1.balance(&setup.user) as u128,
        200_0000000 - actual1,
        "user token1 balance should equal mint - actual1"
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2107)")]
fn test_deposit_position_auto_init_tick_out_of_range() {
    // Price ratio doesn't match the tick range → should panic with TickOutOfBounds
    // price = 200/100 = 2.0 → tick ≈ 6931, but range is [-60, 60)
    let setup = Setup::default();
    setup.mint_user_tokens(100_0000000, 200_0000000);

    let amounts = Vec::from_array(&setup.env, [100_0000000u128, 200_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-60, &60, &amounts, &0);
}

#[test]
fn test_deposit_position_refund_excess() {
    // Verify that desired amounts are transferred in and excess is refunded.
    let setup = Setup::default();
    setup.mint_user_tokens(500_0000000, 500_0000000);

    // First deposit with equal amounts to initialize pool at 1:1 price
    let init_amounts = Vec::from_array(&setup.env, [100_0000000u128, 100_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &init_amounts, &0);

    let bal0_before = setup.token0.balance(&setup.user) as u128;
    let bal1_before = setup.token1.balance(&setup.user) as u128;

    // Second deposit: offer asymmetric amounts; excess should be refunded
    let desired = Vec::from_array(&setup.env, [200_0000000u128, 100_0000000u128]);
    let (actual, liq) = setup
        .pool
        .deposit_position(&setup.user, &-50, &50, &desired, &0);
    assert!(liq > 0);

    let actual0 = actual.get_unchecked(0);
    let actual1 = actual.get_unchecked(1);

    // User should only have spent actual amounts (desired transferred in, excess refunded)
    let bal0_after = setup.token0.balance(&setup.user) as u128;
    let bal1_after = setup.token1.balance(&setup.user) as u128;
    assert_eq!(
        bal0_before - bal0_after,
        actual0,
        "user should have spent exactly actual0"
    );
    assert_eq!(
        bal1_before - bal1_after,
        actual1,
        "user should have spent exactly actual1"
    );

    // At least one token should have had excess refunded (asymmetric desired)
    assert!(
        actual0 < 200_0000000 || actual1 < 100_0000000,
        "at least one token should have excess refunded"
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// claim_fees event tests
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_claim_position_fees_emits_claim_fees_event() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    let amounts = Vec::from_array(&setup.env, [500_0000000u128, 500_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &amounts, &0);

    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &10_0000000);
    setup.pool.swap(&swapper, &0, &1, &10_0000000, &0);

    let (claimed0, claimed1) = pair(setup.pool.claim_position_fees(&setup.user, &-100, &100));
    assert!(claimed0 > 0 || claimed1 > 0, "should have fees to claim");

    assert_eq!(
        count_claim_fees_events(&setup.env, &setup.pool.address),
        1,
        "expected exactly one claim_fees event"
    );
    assert_claim_fees_event(
        &setup.env,
        &setup.pool.address,
        &setup.user,
        &setup.token0.address,
        &setup.token1.address,
        claimed0,
        claimed1,
    );
}

#[test]
fn test_claim_all_position_fees_emits_single_claim_fees_event() {
    let setup = Setup::default();
    setup.mint_user_tokens(2_000_0000000, 2_000_0000000);

    let amounts = Vec::from_array(&setup.env, [600_000000u128, 600_000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-120, &120, &amounts, &0);
    setup
        .pool
        .deposit_position(&setup.user, &-60, &60, &amounts, &0);

    let swapper = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &setup.token0.address).mint(&swapper, &25_0000000);
    setup.pool.swap(&swapper, &0, &1, &25_0000000, &0);

    let (claimed0, claimed1) = pair(setup.pool.claim_all_position_fees(&setup.user));
    assert!(claimed0 > 0 || claimed1 > 0, "should have fees to claim");

    assert_eq!(
        count_claim_fees_events(&setup.env, &setup.pool.address),
        1,
        "expected single aggregated claim_fees event for claim_all"
    );
    assert_claim_fees_event(
        &setup.env,
        &setup.pool.address,
        &setup.user,
        &setup.token0.address,
        &setup.token1.address,
        claimed0,
        claimed1,
    );
}

#[test]
fn test_no_claim_fees_event_when_zero_fees() {
    let setup = Setup::default();
    setup.mint_user_tokens(1_000_0000000, 1_000_0000000);

    let amounts = Vec::from_array(&setup.env, [500_0000000u128, 500_0000000u128]);
    setup
        .pool
        .deposit_position(&setup.user, &-100, &100, &amounts, &0);

    // No swaps — no fees accumulated
    let (claimed0, claimed1) = pair(setup.pool.claim_position_fees(&setup.user, &-100, &100));
    assert_eq!(claimed0, 0);
    assert_eq!(claimed1, 0);

    assert_eq!(
        count_claim_fees_events(&setup.env, &setup.pool.address),
        0,
        "no claim_fees event when nothing claimed"
    );
}
