#![cfg(test)]
extern crate std;

use crate::contracts;
use crate::testutils::{create_token_contract, get_token_admin_client, Setup};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::TokenClient;
use soroban_sdk::{vec, Address, Vec};

#[test]
fn test_integration() {
    let setup = Setup::default();

    // create tokens
    let mut tokens = std::vec![
        create_token_contract(&setup.env, &setup.admin).address,
        create_token_contract(&setup.env, &setup.admin).address,
        create_token_contract(&setup.env, &setup.admin).address,
    ];
    tokens.sort();
    let xlm = TokenClient::new(&setup.env, &tokens[0]);
    let usdc = TokenClient::new(&setup.env, &tokens[1]);
    let usdt = TokenClient::new(&setup.env, &tokens[2]);

    let xlm_admin = get_token_admin_client(&setup.env, &xlm.address);
    let usdc_admin = get_token_admin_client(&setup.env, &usdc.address);
    let usdt_admin = get_token_admin_client(&setup.env, &usdt.address);

    // deploy pools
    let (standard_pool, standard_pool_hash) =
        setup.deploy_standard_pool(&xlm.address, &usdc.address, 30);
    xlm_admin.mint(&setup.admin, &344_000_0000000);
    usdc_admin.mint(&setup.admin, &100_000_0000000);
    standard_pool.deposit(
        &setup.admin,
        &Vec::from_array(&setup.env, [344_000_0000000, 100_000_0000000]),
        &0,
    );

    let (stable_pool, stable_pool_hash) =
        setup.deploy_stableswap_pool(&usdc.address, &usdt.address, 10);
    usdc_admin.mint(&setup.admin, &100_000_0000000);
    usdt_admin.mint(&setup.admin, &100_000_0000000);
    stable_pool.deposit(
        &setup.admin,
        &Vec::from_array(&setup.env, [100_000_0000000, 100_000_0000000]),
        &0,
    );

    // swap through many pools at once
    let user = Address::generate(&setup.env);
    xlm_admin.mint(&user, &10_0000000);

    assert_eq!(
        setup.router.swap_chained(
            &user,
            &vec![
                &setup.env,
                (
                    vec![&setup.env, xlm.address.clone(), usdc.address.clone()],
                    standard_pool_hash.clone(),
                    usdc.address.clone()
                ),
                (
                    vec![&setup.env, usdc.address.clone(), usdt.address.clone()],
                    stable_pool_hash.clone(),
                    usdt.address.clone()
                ),
            ],
            &xlm.address,
            &10_0000000,
            &2_8952734,
        ),
        2_8952734,
    );

    // deploy provider swap fee contract
    let swap_fee = setup.deploy_swap_fee_contract(&setup.operator, &setup.admin, 1000, 10_000);

    // now swap with additional provider fee
    xlm_admin.mint(&user, &10_0000000);
    assert_eq!(
        swap_fee.swap_chained(
            &user,
            &vec![
                &setup.env,
                (
                    vec![&setup.env, xlm.address.clone(), usdc.address.clone()],
                    standard_pool_hash.clone(),
                    usdc.address.clone()
                ),
                (
                    vec![&setup.env, usdc.address.clone(), usdt.address.clone()],
                    stable_pool_hash.clone(),
                    usdt.address.clone()
                ),
            ],
            &xlm.address,
            &10_0000000,
            &2_8864200,
            &30,
        ),
        2_8864200,
    );
}

// ═══════════════════════════════════════════════════════════════════════════
// Concentrated pool: creation via router, deposit, swap, withdraw
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_concentrated_pool_via_router() {
    let setup = Setup::default();

    // Create two sorted tokens
    let mut tokens = std::vec![
        create_token_contract(&setup.env, &setup.admin).address,
        create_token_contract(&setup.env, &setup.admin).address,
    ];
    tokens.sort();
    let token0 = TokenClient::new(&setup.env, &tokens[0]);
    let token1 = TokenClient::new(&setup.env, &tokens[1]);
    let token0_admin = get_token_admin_client(&setup.env, &token0.address);
    let token1_admin = get_token_admin_client(&setup.env, &token1.address);

    // Deploy concentrated pool via router (0.3% fee tier)
    let (conc_pool, conc_pool_hash) =
        setup.deploy_concentrated_pool(&token0.address, &token1.address, 30);

    // Verify pool was created with correct type
    assert_eq!(
        conc_pool.pool_type(),
        soroban_sdk::Symbol::new(&setup.env, "concentrated")
    );

    // Full-range deposit via router-compatible interface
    token0_admin.mint(&setup.admin, &100_000_0000000);
    token1_admin.mint(&setup.admin, &100_000_0000000);
    let (deposited, shares) = conc_pool.deposit(
        &setup.admin,
        &Vec::from_array(&setup.env, [100_000_0000000u128, 100_000_0000000u128]),
        &0,
    );
    assert!(shares > 0, "should mint liquidity");
    assert!(deposited.get_unchecked(0) > 0);
    assert!(deposited.get_unchecked(1) > 0);

    // Reserves should reflect the deposit
    let reserves = conc_pool.get_reserves();
    assert!(reserves.get_unchecked(0) > 0);
    assert!(reserves.get_unchecked(1) > 0);

    // Swap token0 → token1 via router
    let user = Address::generate(&setup.env);
    token0_admin.mint(&user, &10_0000000);

    let tokens_vec = Vec::from_array(&setup.env, [token0.address.clone(), token1.address.clone()]);
    let out = setup.router.swap(
        &user,
        &tokens_vec,
        &token0.address,
        &token1.address,
        &conc_pool_hash,
        &10_0000000,
        &0,
    );
    assert!(out > 0, "swap should produce output");

    // Verify user received token1
    assert!(token1.balance(&user) > 0);

    // Withdraw via router-compatible interface
    let balances_before_0 = token0.balance(&setup.admin);
    let balances_before_1 = token1.balance(&setup.admin);
    let withdrawn = conc_pool.withdraw(
        &setup.admin,
        &(shares / 2),
        &Vec::from_array(&setup.env, [0u128, 0u128]),
    );
    assert!(withdrawn.get_unchecked(0) > 0, "should withdraw token0");
    assert!(withdrawn.get_unchecked(1) > 0, "should withdraw token1");
    assert!(token0.balance(&setup.admin) > balances_before_0);
    assert!(token1.balance(&setup.admin) > balances_before_1);
}

// ═══════════════════════════════════════════════════════════════════════════
// Concentrated pool in multi-hop swap chain
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_concentrated_pool_multi_hop() {
    let setup = Setup::default();

    // Create three sorted tokens
    let mut tokens = std::vec![
        create_token_contract(&setup.env, &setup.admin).address,
        create_token_contract(&setup.env, &setup.admin).address,
        create_token_contract(&setup.env, &setup.admin).address,
    ];
    tokens.sort();
    let token_a = TokenClient::new(&setup.env, &tokens[0]);
    let token_b = TokenClient::new(&setup.env, &tokens[1]);
    let token_c = TokenClient::new(&setup.env, &tokens[2]);
    let admin_a = get_token_admin_client(&setup.env, &token_a.address);
    let admin_b = get_token_admin_client(&setup.env, &token_b.address);
    let admin_c = get_token_admin_client(&setup.env, &token_c.address);

    // Deploy concentrated pool for A-B
    let (conc_pool_ab, conc_hash_ab) =
        setup.deploy_concentrated_pool(&token_a.address, &token_b.address, 30);
    admin_a.mint(&setup.admin, &100_000_0000000);
    admin_b.mint(&setup.admin, &100_000_0000000);
    conc_pool_ab.deposit(
        &setup.admin,
        &Vec::from_array(&setup.env, [100_000_0000000u128, 100_000_0000000u128]),
        &0,
    );

    // Deploy standard pool for B-C
    let (_std_pool_bc, std_hash_bc) =
        setup.deploy_standard_pool(&token_b.address, &token_c.address, 30);
    admin_b.mint(&setup.admin, &100_000_0000000);
    admin_c.mint(&setup.admin, &100_000_0000000);
    _std_pool_bc.deposit(
        &setup.admin,
        &Vec::from_array(&setup.env, [100_000_0000000, 100_000_0000000]),
        &0,
    );

    // Multi-hop swap: A → B (concentrated) → C (standard)
    let user = Address::generate(&setup.env);
    admin_a.mint(&user, &10_0000000);
    let final_out = setup.router.swap_chained(
        &user,
        &vec![
            &setup.env,
            (
                vec![&setup.env, token_a.address.clone(), token_b.address.clone()],
                conc_hash_ab.clone(),
                token_b.address.clone(),
            ),
            (
                vec![&setup.env, token_b.address.clone(), token_c.address.clone()],
                std_hash_bc.clone(),
                token_c.address.clone(),
            ),
        ],
        &token_a.address,
        &10_0000000,
        &0,
    );
    assert!(final_out > 0, "multi-hop should produce output");
    assert!(token_c.balance(&user) > 0, "user should receive token_c");
}

// ═══════════════════════════════════════════════════════════════════════════
// Concentrated pool: position-based operations via direct contract calls
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_concentrated_pool_positions() {
    let setup = Setup::default();

    let mut tokens = std::vec![
        create_token_contract(&setup.env, &setup.admin).address,
        create_token_contract(&setup.env, &setup.admin).address,
    ];
    tokens.sort();
    let token0 = TokenClient::new(&setup.env, &tokens[0]);
    let token1 = TokenClient::new(&setup.env, &tokens[1]);
    let token0_admin = get_token_admin_client(&setup.env, &token0.address);
    let token1_admin = get_token_admin_client(&setup.env, &token1.address);

    let (conc_pool, _) = setup.deploy_concentrated_pool(&token0.address, &token1.address, 30);

    // Deposit a narrow-range position
    let user = Address::generate(&setup.env);
    token0_admin.mint(&user, &100_000_0000000);
    token1_admin.mint(&user, &100_000_0000000);

    // tick_spacing for 30bps fee is 60, so use multiples of 60
    let dep_amounts = Vec::from_array(&setup.env, [50_000_0000000u128, 50_000_0000000u128]);
    let (actual_amounts, liquidity) =
        conc_pool.deposit_position(&user, &user, &-120, &120, &dep_amounts);
    let amt0 = actual_amounts.get_unchecked(0);
    let amt1 = actual_amounts.get_unchecked(1);
    assert!(amt0 > 0 || amt1 > 0, "should transfer at least one token");
    assert!(liquidity > 0);

    // Read position back
    let pos = conc_pool.get_position(&user, &-120, &120);
    assert_eq!(pos.liquidity, liquidity);

    // Generate fees via swap
    let swapper = Address::generate(&setup.env);
    token0_admin.mint(&swapper, &10_0000000);
    conc_pool.swap(&swapper, &0, &1, &10_0000000, &0);

    // Collect position fees
    let (fee0, fee1) =
        conc_pool.claim_position_fees(&user, &user, &-120, &120, &u128::MAX, &u128::MAX);
    assert!(fee0 > 0 || fee1 > 0, "should collect fees");

    // Withdraw position
    conc_pool.withdraw_position(&user, &-120, &120, &liquidity);

    // Collect remaining owed tokens
    let (owed0, owed1) =
        conc_pool.claim_position_fees(&user, &user, &-120, &120, &u128::MAX, &u128::MAX);
    assert!(owed0 > 0 || owed1 > 0, "should collect withdrawn tokens");
}

// ═══════════════════════════════════════════════════════════════════════════
// Concentrated pool: gauge scheduling via router
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_concentrated_pool_gauge() {
    let setup = Setup::default();

    let mut tokens = std::vec![
        create_token_contract(&setup.env, &setup.admin).address,
        create_token_contract(&setup.env, &setup.admin).address,
    ];
    tokens.sort();
    let token0 = TokenClient::new(&setup.env, &tokens[0]);
    let token1 = TokenClient::new(&setup.env, &tokens[1]);
    let token0_admin = get_token_admin_client(&setup.env, &token0.address);
    let token1_admin = get_token_admin_client(&setup.env, &token1.address);

    let (conc_pool, conc_pool_hash) =
        setup.deploy_concentrated_pool(&token0.address, &token1.address, 30);

    // Deposit liquidity so the pool has working supply
    token0_admin.mint(&setup.admin, &100_000_0000000);
    token1_admin.mint(&setup.admin, &100_000_0000000);
    conc_pool.deposit(
        &setup.admin,
        &Vec::from_array(&setup.env, [100_000_0000000u128, 100_000_0000000u128]),
        &0,
    );

    // Deploy a gauge reward token and gauge contract
    let gauge_reward = create_token_contract(&setup.env, &setup.admin);
    let gauge = contracts::rewards_gauge::Client::new(
        &setup.env,
        &setup.env.register(
            contracts::rewards_gauge::WASM,
            contracts::rewards_gauge::Args::__constructor(
                &conc_pool.address,
                &gauge_reward.address,
            ),
        ),
    );

    // Attach gauge to pool (pool accepts router as admin)
    conc_pool.gauge_add(&setup.router.address, &gauge.address);

    // Verify gauge was added
    let gauges = conc_pool.get_gauges();
    assert_eq!(gauges.len(), 1, "should have one gauge");
    assert_eq!(
        gauges.get(gauge_reward.address.clone()).unwrap(),
        gauge.address
    );

    // Schedule rewards on the gauge via the pool (router-authorized)
    let gauge_reward_admin = get_token_admin_client(&setup.env, &gauge_reward.address);
    // tps=100 * 604800s = 60_480_000 total needed
    let total_reward: i128 = 100 * 604800;
    gauge_reward_admin.mint(&setup.admin, &total_reward);
    gauge_reward.approve(&setup.admin, &gauge.address, &total_reward, &10000);
    conc_pool.gauge_schedule_reward(
        &setup.router.address,
        &setup.admin,
        &gauge.address,
        &None::<u64>,
        &604800, // 1 week
        &100,    // 100 units per second
    );
}
