#![cfg(test)]
extern crate std;

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
    assert_eq!(lower.liquidity_gross, 1_0000000);
    assert_eq!(lower.liquidity_net, 1_0000000);

    let upper = setup.pool.ticks(&10);
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

// Griefing scenario: attacker fills every tick with dust positions to increase
// storage reads during swaps. Whale provide full-range liquidity, then
// an attacker initializes every possible tick in a range around the current
// price with minimal liquidity.
///
// With tick_spacing=20 (0.1% fee tier), this test demonstrates:
// - Dust positions add overhead but spacing caps the damage
// - Reports exact ledger footprint for capacity planning
///
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
    let dust_liquidity: u128 = 1; // minimum possible

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
                &dust_liquidity,
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
        slot_before.tick, slot_after.tick, tick_delta, ticks_crossed
    );
    std::println!(
        "Footprint: read_write={}, read_only={}",
        cost.write_entries,
        cost.disk_read_entries + cost.memory_read_entries
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

    // ---- Extra large swap: ~10% price move ----
    let xlarge_swap: u128 = 100_000_0000000;
    get_token_admin_client(&env, &token0.address).mint(&swapper, &(xlarge_swap as i128));
    let out_xlarge = pool.swap(&swapper, &0, &1, &xlarge_swap, &0);

    let cost_xlarge = env.cost_estimate().resources();

    let slot_after_xlarge = pool.slot0();
    let tick_delta_xlarge = (slot_after_xlarge.tick - slot_after_large.tick).abs();
    let ticks_crossed_xlarge = tick_delta_xlarge / tick_spacing;

    std::println!("--- Extra large swap (~10% move) ---");
    std::println!("Amount in:  {}", xlarge_swap);
    std::println!("Amount out: {}", out_xlarge);
    std::println!(
        "Price moved: tick {} → {} (delta={}, ~{} spacing crossings)",
        slot_after_large.tick,
        slot_after_xlarge.tick,
        tick_delta_xlarge,
        ticks_crossed_xlarge
    );
    std::println!(
        "Footprint: read_write={}, read_only={}",
        cost_xlarge.write_entries,
        cost_xlarge.disk_read_entries + cost_xlarge.memory_read_entries
    );

    // Mainnet limits: 200 read_only + 200 read_write entries per tx
    const RW_LIMIT: u32 = 200;
    const RO_LIMIT: u32 = 200;

    let ro_small = cost.disk_read_entries + cost.memory_read_entries;
    let ro_large = cost_large.disk_read_entries + cost_large.memory_read_entries;
    let ro_xlarge = cost_xlarge.disk_read_entries + cost_xlarge.memory_read_entries;

    // Summary
    std::println!("\n=== GRIEFING IMPACT SUMMARY (tick_spacing={}) ===", tick_spacing);
    std::println!("Dust positions: {}", total_dust_positions);
    std::println!("Whale liquidity: {}", liquidity_before);
    std::println!("Mainnet limits: rw={}, ro={}", RW_LIMIT, RO_LIMIT);
    std::println!(
        " ~1% move: {} crossings, rw={}/{} ro={}/{}",
        ticks_crossed, cost.write_entries, RW_LIMIT, ro_small, RO_LIMIT
    );
    std::println!(
        " ~5% move: {} crossings, rw={}/{} ro={}/{}",
        ticks_crossed_large, cost_large.write_entries, RW_LIMIT, ro_large, RO_LIMIT
    );
    std::println!(
        "~10% move: {} crossings, rw={}/{} ro={}/{}",
        ticks_crossed_xlarge, cost_xlarge.write_entries, RW_LIMIT, ro_xlarge, RO_LIMIT
    );

    // Assert small and medium swaps fit within mainnet limits
    assert!(
        cost.write_entries <= RW_LIMIT && ro_small <= RO_LIMIT,
        "~1% swap exceeds mainnet limits: rw={}/{} ro={}/{}",
        cost.write_entries, RW_LIMIT, ro_small, RO_LIMIT
    );
    assert!(
        cost_large.write_entries <= RW_LIMIT && ro_large <= RO_LIMIT,
        "~5% swap exceeds mainnet limits: rw={}/{} ro={}/{}",
        cost_large.write_entries, RW_LIMIT, ro_large, RO_LIMIT
    );
    // ~10% move under worst-case griefing may exceed limits — that's the attack ceiling
    std::println!(
        "\n~10% move fits mainnet? rw={} ro={}",
        if cost_xlarge.write_entries <= RW_LIMIT { "YES" } else { "NO" },
        if ro_xlarge <= RO_LIMIT { "YES" } else { "NO" },
    );
}
