#![cfg(test)]
extern crate std;

use crate::testutils::{
    create_token_contract, estimate_vm_overhead, estimate_vm_overhead_mixed,
    get_token_admin_client, measure_budget_with_vm, measure_budget_with_vm_mixed, Setup, SetupV170,
    CONC_POOL_MASTER, PLANE_MASTER, PLANE_V170, POOL_MASTER, POOL_V170, ROUTER_MASTER, ROUTER_V170,
    STABLESWAP_POOL_MASTER, TOKEN_MASTER, TOKEN_V170, YTIME1_TOKEN,
};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::TokenClient;
use soroban_sdk::{Address, Vec};

fn setup_tokens_and_pools<'a>(
    setup: &'a Setup<'a>,
    count: usize,
) -> (
    std::vec::Vec<TokenClient<'a>>,
    std::vec::Vec<soroban_sdk::BytesN<32>>,
) {
    let mut tokens: std::vec::Vec<Address> = (0..=count)
        .map(|_| create_token_contract(&setup.env, &setup.admin).address)
        .collect();
    tokens.sort();

    let token_clients: std::vec::Vec<TokenClient> = tokens
        .iter()
        .map(|t| TokenClient::new(&setup.env, t))
        .collect();

    let mut pool_hashes = std::vec::Vec::new();

    for i in 0..count {
        let admin_a = get_token_admin_client(&setup.env, &token_clients[i].address);
        let admin_b = get_token_admin_client(&setup.env, &token_clients[i + 1].address);

        let (_pool, pool_hash) = setup.deploy_standard_pool(
            &token_clients[i].address,
            &token_clients[i + 1].address,
            30,
        );

        admin_a.mint(&setup.admin, &1_000_000_0000000);
        admin_b.mint(&setup.admin, &1_000_000_0000000);
        _pool.deposit(
            &setup.admin,
            &Vec::from_array(&setup.env, [1_000_000_0000000, 1_000_000_0000000]),
            &0,
        );

        pool_hashes.push(pool_hash);
    }

    (token_clients, pool_hashes)
}

fn build_swap_chain(
    env: &soroban_sdk::Env,
    tokens: &[TokenClient],
    pool_hashes: &[soroban_sdk::BytesN<32>],
) -> Vec<(Vec<Address>, soroban_sdk::BytesN<32>, Address)> {
    let mut chain = Vec::new(env);
    for i in 0..pool_hashes.len() {
        let mut pool_tokens = std::vec![tokens[i].address.clone(), tokens[i + 1].address.clone(),];
        pool_tokens.sort();
        chain.push_back((
            Vec::from_array(env, [pool_tokens[0].clone(), pool_tokens[1].clone()]),
            pool_hashes[i].clone(),
            tokens[i + 1].address.clone(),
        ));
    }
    chain
}

// Token calls per scenario (master, with _sync_reserves + skip-dup-sync):
//   swap_chained (strict_send):    4*N + 2  (per pool: 2 balance + 1 transfer_in + 1 transfer_out; router: 2 transfers)
//   swap_chained_strict_receive:   4*N + 5  (reverse: 2*N balance; forward: 2*N transfer, 0 sync, 0 refund; router: 5)
//
// Token calls per scenario (v1.7.0, no _sync_reserves):
//   swap_chained (strict_send):    2*N + 2  (per pool: 1 transfer_in + 1 transfer_out; router: 2)
//   swap_chained_strict_receive:   3*N + 3  (per pool: 1 transfer_in + 1 refund + 1 transfer_out; router: 3)

// --- Master: Strict Send benchmarks ---

#[test]
fn bench_2pool_strict_send() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_tokens_and_pools(&setup, 2);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &10_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let in_amount: u128 = 10_0000000;
    let n = 2u64;

    measure_budget_with_vm(
        &setup.env,
        "master 2-pool strict_send",
        &ROUTER_MASTER,
        &POOL_MASTER,
        &TOKEN_MASTER,
        &PLANE_MASTER,
        n,
        4 * n + 2,
        n,
        || {
            setup
                .router
                .swap_chained(&user, &chain, &token_in, &in_amount, &0);
        },
    );
}

#[test]
fn bench_3pool_strict_send() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_tokens_and_pools(&setup, 3);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &10_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let in_amount: u128 = 10_0000000;
    let n = 3u64;

    measure_budget_with_vm(
        &setup.env,
        "master 3-pool strict_send",
        &ROUTER_MASTER,
        &POOL_MASTER,
        &TOKEN_MASTER,
        &PLANE_MASTER,
        n,
        4 * n + 2,
        n,
        || {
            setup
                .router
                .swap_chained(&user, &chain, &token_in, &in_amount, &0);
        },
    );
}

#[test]
fn bench_4pool_strict_send() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_tokens_and_pools(&setup, 4);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &10_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let in_amount: u128 = 10_0000000;
    let n = 4u64;

    measure_budget_with_vm(
        &setup.env,
        "master 4-pool strict_send",
        &ROUTER_MASTER,
        &POOL_MASTER,
        &TOKEN_MASTER,
        &PLANE_MASTER,
        n,
        4 * n + 2,
        n,
        || {
            setup
                .router
                .swap_chained(&user, &chain, &token_in, &in_amount, &0);
        },
    );
}

#[test]
fn bench_5pool_strict_send() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_tokens_and_pools(&setup, 5);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &10_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let in_amount: u128 = 10_0000000;
    let n = 5u64;

    measure_budget_with_vm(
        &setup.env,
        "master 5-pool strict_send",
        &ROUTER_MASTER,
        &POOL_MASTER,
        &TOKEN_MASTER,
        &PLANE_MASTER,
        n,
        4 * n + 2,
        n,
        || {
            setup
                .router
                .swap_chained(&user, &chain, &token_in, &in_amount, &0);
        },
    );
}

// --- Master: Strict Receive benchmarks ---

#[test]
fn bench_2pool_strict_receive() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_tokens_and_pools(&setup, 2);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let out_amount: u128 = 5_0000000;
    let in_max: u128 = 100_0000000;
    let n = 2u64;

    measure_budget_with_vm(
        &setup.env,
        "master 2-pool strict_receive",
        &ROUTER_MASTER,
        &POOL_MASTER,
        &TOKEN_MASTER,
        &PLANE_MASTER,
        n,
        4 * n + 5,
        n,
        || {
            setup.router.swap_chained_strict_receive(
                &user,
                &chain,
                &token_in,
                &out_amount,
                &in_max,
            );
        },
    );
}

#[test]
fn bench_3pool_strict_receive() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_tokens_and_pools(&setup, 3);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let out_amount: u128 = 5_0000000;
    let in_max: u128 = 100_0000000;
    let n = 3u64;

    measure_budget_with_vm(
        &setup.env,
        "master 3-pool strict_receive",
        &ROUTER_MASTER,
        &POOL_MASTER,
        &TOKEN_MASTER,
        &PLANE_MASTER,
        n,
        4 * n + 5,
        n,
        || {
            setup.router.swap_chained_strict_receive(
                &user,
                &chain,
                &token_in,
                &out_amount,
                &in_max,
            );
        },
    );
}

#[test]
fn bench_4pool_strict_receive() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_tokens_and_pools(&setup, 4);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let out_amount: u128 = 5_0000000;
    let in_max: u128 = 100_0000000;
    let n = 4u64;

    measure_budget_with_vm(
        &setup.env,
        "master 4-pool strict_receive",
        &ROUTER_MASTER,
        &POOL_MASTER,
        &TOKEN_MASTER,
        &PLANE_MASTER,
        n,
        4 * n + 5,
        n,
        || {
            setup.router.swap_chained_strict_receive(
                &user,
                &chain,
                &token_in,
                &out_amount,
                &in_max,
            );
        },
    );
}

#[test]
fn bench_5pool_strict_receive() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_tokens_and_pools(&setup, 5);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let out_amount: u128 = 5_0000000;
    let in_max: u128 = 100_0000000;
    let n = 5u64;

    measure_budget_with_vm(
        &setup.env,
        "master 5-pool strict_receive",
        &ROUTER_MASTER,
        &POOL_MASTER,
        &TOKEN_MASTER,
        &PLANE_MASTER,
        n,
        4 * n + 5,
        n,
        || {
            setup.router.swap_chained_strict_receive(
                &user,
                &chain,
                &token_in,
                &out_amount,
                &in_max,
            );
        },
    );
}

// --- v1.7.0 comparison benchmarks ---

fn setup_tokens_and_pools_v170<'a>(
    setup: &'a SetupV170<'a>,
    count: usize,
) -> (
    std::vec::Vec<TokenClient<'a>>,
    std::vec::Vec<soroban_sdk::BytesN<32>>,
) {
    let mut tokens: std::vec::Vec<Address> = (0..=count)
        .map(|_| create_token_contract(&setup.env, &setup.admin).address)
        .collect();
    tokens.sort();

    let token_clients: std::vec::Vec<TokenClient> = tokens
        .iter()
        .map(|t| TokenClient::new(&setup.env, t))
        .collect();

    let mut pool_hashes = std::vec::Vec::new();

    for i in 0..count {
        let admin_a = get_token_admin_client(&setup.env, &token_clients[i].address);
        let admin_b = get_token_admin_client(&setup.env, &token_clients[i + 1].address);

        let (_pool, pool_hash) = setup.deploy_standard_pool(
            &token_clients[i].address,
            &token_clients[i + 1].address,
            30,
        );

        admin_a.mint(&setup.admin, &1_000_000_0000000);
        admin_b.mint(&setup.admin, &1_000_000_0000000);
        _pool.deposit(
            &setup.admin,
            &Vec::from_array(&setup.env, [1_000_000_0000000, 1_000_000_0000000]),
            &0,
        );

        pool_hashes.push(pool_hash);
    }

    (token_clients, pool_hashes)
}

#[test]
fn bench_v170_2pool_strict_send() {
    let setup = SetupV170::setup();
    let (tokens, pool_hashes) = setup_tokens_and_pools_v170(&setup, 2);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &10_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let in_amount: u128 = 10_0000000;
    let n = 2u64;

    measure_budget_with_vm(
        &setup.env,
        "v1.7.0 2-pool strict_send",
        &ROUTER_V170,
        &POOL_V170,
        &TOKEN_V170,
        &PLANE_V170,
        n,
        2 * n + 2,
        n,
        || {
            setup
                .router
                .swap_chained(&user, &chain, &token_in, &in_amount, &0);
        },
    );
}

#[test]
fn bench_v170_3pool_strict_send() {
    let setup = SetupV170::setup();
    let (tokens, pool_hashes) = setup_tokens_and_pools_v170(&setup, 3);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &10_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let in_amount: u128 = 10_0000000;
    let n = 3u64;

    measure_budget_with_vm(
        &setup.env,
        "v1.7.0 3-pool strict_send",
        &ROUTER_V170,
        &POOL_V170,
        &TOKEN_V170,
        &PLANE_V170,
        n,
        2 * n + 2,
        n,
        || {
            setup
                .router
                .swap_chained(&user, &chain, &token_in, &in_amount, &0);
        },
    );
}

#[test]
fn bench_v170_2pool_strict_receive() {
    let setup = SetupV170::setup();
    let (tokens, pool_hashes) = setup_tokens_and_pools_v170(&setup, 2);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let out_amount: u128 = 5_0000000;
    let in_max: u128 = 100_0000000;
    let n = 2u64;

    measure_budget_with_vm(
        &setup.env,
        "v1.7.0 2-pool strict_receive",
        &ROUTER_V170,
        &POOL_V170,
        &TOKEN_V170,
        &PLANE_V170,
        n,
        3 * n + 3,
        n,
        || {
            setup.router.swap_chained_strict_receive(
                &user,
                &chain,
                &token_in,
                &out_amount,
                &in_max,
            );
        },
    );
}

#[test]
fn bench_v170_3pool_strict_receive() {
    let setup = SetupV170::setup();
    let (tokens, pool_hashes) = setup_tokens_and_pools_v170(&setup, 3);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let out_amount: u128 = 5_0000000;
    let in_max: u128 = 100_0000000;
    let n = 3u64;

    measure_budget_with_vm(
        &setup.env,
        "v1.7.0 3-pool strict_receive",
        &ROUTER_V170,
        &POOL_V170,
        &TOKEN_V170,
        &PLANE_V170,
        n,
        3 * n + 3,
        n,
        || {
            setup.router.swap_chained_strict_receive(
                &user,
                &chain,
                &token_in,
                &out_amount,
                &in_max,
            );
        },
    );
}

#[test]
fn bench_v170_4pool_strict_send() {
    let setup = SetupV170::setup();
    let (tokens, pool_hashes) = setup_tokens_and_pools_v170(&setup, 4);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &10_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let in_amount: u128 = 10_0000000;
    let n = 4u64;

    measure_budget_with_vm(
        &setup.env,
        "v1.7.0 4-pool strict_send",
        &ROUTER_V170,
        &POOL_V170,
        &TOKEN_V170,
        &PLANE_V170,
        n,
        2 * n + 2,
        n,
        || {
            setup
                .router
                .swap_chained(&user, &chain, &token_in, &in_amount, &0);
        },
    );
}

#[test]
fn bench_v170_5pool_strict_send() {
    let setup = SetupV170::setup();
    let (tokens, pool_hashes) = setup_tokens_and_pools_v170(&setup, 5);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &10_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let in_amount: u128 = 10_0000000;
    let n = 5u64;

    measure_budget_with_vm(
        &setup.env,
        "v1.7.0 5-pool strict_send",
        &ROUTER_V170,
        &POOL_V170,
        &TOKEN_V170,
        &PLANE_V170,
        n,
        2 * n + 2,
        n,
        || {
            setup
                .router
                .swap_chained(&user, &chain, &token_in, &in_amount, &0);
        },
    );
}

#[test]
fn bench_v170_4pool_strict_receive() {
    let setup = SetupV170::setup();
    let (tokens, pool_hashes) = setup_tokens_and_pools_v170(&setup, 4);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let out_amount: u128 = 5_0000000;
    let in_max: u128 = 100_0000000;
    let n = 4u64;

    measure_budget_with_vm(
        &setup.env,
        "v1.7.0 4-pool strict_receive",
        &ROUTER_V170,
        &POOL_V170,
        &TOKEN_V170,
        &PLANE_V170,
        n,
        3 * n + 3,
        n,
        || {
            setup.router.swap_chained_strict_receive(
                &user,
                &chain,
                &token_in,
                &out_amount,
                &in_max,
            );
        },
    );
}

#[test]
fn bench_v170_5pool_strict_receive() {
    let setup = SetupV170::setup();
    let (tokens, pool_hashes) = setup_tokens_and_pools_v170(&setup, 5);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let out_amount: u128 = 5_0000000;
    let in_max: u128 = 100_0000000;
    let n = 5u64;

    measure_budget_with_vm(
        &setup.env,
        "v1.7.0 5-pool strict_receive",
        &ROUTER_V170,
        &POOL_V170,
        &TOKEN_V170,
        &PLANE_V170,
        n,
        3 * n + 3,
        n,
        || {
            setup.router.swap_chained_strict_receive(
                &user,
                &chain,
                &token_in,
                &out_amount,
                &in_max,
            );
        },
    );
}

// --- Optimization projection ---

/// Prints a comparison table of VM overhead for different optimization strategies.
/// Uses measured memory from actual test runs + projected VM overhead.
#[test]
fn bench_optimization_projections() {
    // Run a real 3-pool strict_receive to get measured memory baseline
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_tokens_and_pools(&setup, 3);
    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);
    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();

    setup.env.cost_estimate().budget().reset_unlimited();
    setup.env.cost_estimate().budget().reset_tracker();
    setup.router.swap_chained_strict_receive(
        &user,
        &chain,
        &token_in,
        &5_0000000u128,
        &100_0000000u128,
    );
    let measured_mem_per_pool = setup.env.cost_estimate().budget().memory_bytes_cost() / 3;

    std::println!("\n========================================================");
    std::println!("  OPTIMIZATION PROJECTIONS: strict_receive");
    std::println!("  Token calls formula per N pools:");
    std::println!("    pre-optimization:     7N + 5  (estimate: 2N sync, swap_strict_receive: 3N transfer + 2N sync, router: 5)");
    std::println!("    IMPLEMENTED skip sync: 5N + 5  (estimate: 2N sync, swap_strict_receive: 3N transfer + 0 sync, router: 5)");
    std::println!("    opt: swap in forward: 6N + 5  (estimate: 2N sync, swap: 2N transfer + 2N sync, router: 5)");
    std::println!("    opt: + skip dup sync: 4N + 5  (estimate: 2N sync, swap: 2N transfer + 0 sync, router: 5)");
    std::println!("    opt: + no refund chk: 4N + 3  (estimate: 2N sync, swap: 2N transfer + 0 sync, router: 3)");
    std::println!("========================================================");

    for n in 2u64..=5 {
        let measured = measured_mem_per_pool * n;
        let pre_opt = 7 * n + 5;
        let implemented = 4 * n + 5;
        let opt_swap = 6 * n + 5;
        let opt_swap_skip_sync = 4 * n + 5;
        let opt_swap_no_refund = 4 * n + 3;

        // plane_calls: pre-opt has update_plane in both sync_reserves and swap end = 2*n
        // implemented: skip-dup-sync means only swap-end update_plane = n
        let vm_pre_opt = estimate_vm_overhead(
            "",
            &ROUTER_MASTER,
            &POOL_MASTER,
            &TOKEN_MASTER,
            &PLANE_MASTER,
            n,
            pre_opt,
            2 * n,
        );
        let vm_implemented = estimate_vm_overhead(
            "",
            &ROUTER_MASTER,
            &POOL_MASTER,
            &TOKEN_MASTER,
            &PLANE_MASTER,
            n,
            implemented,
            n,
        );
        let vm_opt_swap = estimate_vm_overhead(
            "",
            &ROUTER_MASTER,
            &POOL_MASTER,
            &TOKEN_MASTER,
            &PLANE_MASTER,
            n,
            opt_swap,
            2 * n,
        );
        let vm_opt_swap_skip_sync = estimate_vm_overhead(
            "",
            &ROUTER_MASTER,
            &POOL_MASTER,
            &TOKEN_MASTER,
            &PLANE_MASTER,
            n,
            opt_swap_skip_sync,
            n,
        );
        let vm_opt_swap_no_refund = estimate_vm_overhead(
            "",
            &ROUTER_MASTER,
            &POOL_MASTER,
            &TOKEN_MASTER,
            &PLANE_MASTER,
            n,
            opt_swap_no_refund,
            n,
        );

        let total = |vm: u64| -> f64 { (measured + vm) as f64 / 1_048_576.0 };
        let limit = 40.0f64;

        std::println!("\n--- {}-pool strict_receive ---", n);
        std::println!(
            "  {:30} token_calls={:2}  VM={:6.2} MB  total≈{:5.2} MB  headroom={:+.2} MB",
            "pre-optimization (7N+5)",
            pre_opt,
            vm_pre_opt as f64 / 1_048_576.0,
            total(vm_pre_opt),
            limit - total(vm_pre_opt)
        );
        std::println!("  {:30} token_calls={:2}  VM={:6.2} MB  total≈{:5.2} MB  headroom={:+.2} MB  saved={:.2} MB",
            ">>> IMPLEMENTED (5N+5)", implemented, vm_implemented as f64 / 1_048_576.0, total(vm_implemented), limit - total(vm_implemented),
            total(vm_pre_opt) - total(vm_implemented));
        std::println!("  {:30} token_calls={:2}  VM={:6.2} MB  total≈{:5.2} MB  headroom={:+.2} MB  saved={:.2} MB",
            "opt: swap in fwd (6N+5)", opt_swap, vm_opt_swap as f64 / 1_048_576.0, total(vm_opt_swap), limit - total(vm_opt_swap),
            total(vm_pre_opt) - total(vm_opt_swap));
        std::println!("  {:30} token_calls={:2}  VM={:6.2} MB  total≈{:5.2} MB  headroom={:+.2} MB  saved={:.2} MB",
            "opt: fwd + skip sync (4N+5)", opt_swap_skip_sync, vm_opt_swap_skip_sync as f64 / 1_048_576.0, total(vm_opt_swap_skip_sync), limit - total(vm_opt_swap_skip_sync),
            total(vm_pre_opt) - total(vm_opt_swap_skip_sync));
        std::println!("  {:30} token_calls={:2}  VM={:6.2} MB  total≈{:5.2} MB  headroom={:+.2} MB  saved={:.2} MB",
            "opt: fwd + no refund (4N+3)", opt_swap_no_refund, vm_opt_swap_no_refund as f64 / 1_048_576.0, total(vm_opt_swap_no_refund), limit - total(vm_opt_swap_no_refund),
            total(vm_pre_opt) - total(vm_opt_swap_no_refund));
    }
    std::println!("\n========================================================\n");
}

// --- Mixed pool benchmarks (realistic mainnet scenario) ---
//
// Topology: standard → stableswap → concentrated → standard
// This matches real mainnet multi-hop paths with all pool types.
//
// Token call formulas for mixed (N_std standard/stableswap pools, N_conc concentrated pools):
//   strict_send:    4*N_std + 2*N_conc + 2  (std/stable: 2 balance + 2 transfer, conc: 0 balance + 2 transfer, router: 2)
//   strict_receive: 2*N_std + 2*N + 5       (estimate: 2*N_std balance, swap: 2*N transfer, router: 5)
// Plane calls: N (total pools)

/// Set up a mixed 4-pool chain: standard → stableswap → concentrated → standard
fn setup_mixed_4pool_chain<'a>(
    setup: &'a Setup<'a>,
) -> (
    std::vec::Vec<TokenClient<'a>>,
    std::vec::Vec<soroban_sdk::BytesN<32>>,
) {
    // Create 5 sorted tokens for 4-pool chain
    let mut tokens: std::vec::Vec<Address> = (0..=4)
        .map(|_| create_token_contract(&setup.env, &setup.admin).address)
        .collect();
    tokens.sort();

    let token_clients: std::vec::Vec<TokenClient> = tokens
        .iter()
        .map(|t| TokenClient::new(&setup.env, t))
        .collect();

    let mut pool_hashes = std::vec::Vec::new();

    // Pool 0: standard (tokens 0-1)
    {
        let admin_a = get_token_admin_client(&setup.env, &token_clients[0].address);
        let admin_b = get_token_admin_client(&setup.env, &token_clients[1].address);
        let (_pool, pool_hash) =
            setup.deploy_standard_pool(&token_clients[0].address, &token_clients[1].address, 30);
        admin_a.mint(&setup.admin, &1_000_000_0000000);
        admin_b.mint(&setup.admin, &1_000_000_0000000);
        _pool.deposit(
            &setup.admin,
            &Vec::from_array(&setup.env, [1_000_000_0000000, 1_000_000_0000000]),
            &0,
        );
        pool_hashes.push(pool_hash);
    }

    // Pool 1: stableswap (tokens 1-2)
    {
        let admin_a = get_token_admin_client(&setup.env, &token_clients[1].address);
        let admin_b = get_token_admin_client(&setup.env, &token_clients[2].address);
        let (_pool, pool_hash) =
            setup.deploy_stableswap_pool(&token_clients[1].address, &token_clients[2].address, 30);
        admin_a.mint(&setup.admin, &1_000_000_0000000);
        admin_b.mint(&setup.admin, &1_000_000_0000000);
        _pool.deposit(
            &setup.admin,
            &Vec::from_array(&setup.env, [1_000_000_0000000, 1_000_000_0000000]),
            &0,
        );
        pool_hashes.push(pool_hash);
    }

    // Pool 2: concentrated (tokens 2-3)
    {
        let admin_a = get_token_admin_client(&setup.env, &token_clients[2].address);
        let admin_b = get_token_admin_client(&setup.env, &token_clients[3].address);
        let (_pool, pool_hash) = setup.deploy_concentrated_pool(
            &token_clients[2].address,
            &token_clients[3].address,
            30,
        );
        admin_a.mint(&setup.admin, &100_000_0000000);
        admin_b.mint(&setup.admin, &100_000_0000000);
        _pool.deposit(
            &setup.admin,
            &Vec::from_array(&setup.env, [100_000_0000000u128, 100_000_0000000u128]),
            &0,
        );
        pool_hashes.push(pool_hash);
    }

    // Pool 3: standard (tokens 3-4)
    {
        let admin_a = get_token_admin_client(&setup.env, &token_clients[3].address);
        let admin_b = get_token_admin_client(&setup.env, &token_clients[4].address);
        let (_pool, pool_hash) =
            setup.deploy_standard_pool(&token_clients[3].address, &token_clients[4].address, 30);
        admin_a.mint(&setup.admin, &1_000_000_0000000);
        admin_b.mint(&setup.admin, &1_000_000_0000000);
        _pool.deposit(
            &setup.admin,
            &Vec::from_array(&setup.env, [1_000_000_0000000, 1_000_000_0000000]),
            &0,
        );
        pool_hashes.push(pool_hash);
    }

    (token_clients, pool_hashes)
}

// Mixed pool VM call counts:
// strict_send 4-pool (2 std + 1 stable + 1 conc):
//   each std/stable swap: 2 balance (sync) + 2 transfer = 4 token calls
//   each conc swap: 0 balance + 2 transfer = 2 token calls
//   router: 1 transfer in + 1 transfer out = 2
//   total token calls: 3*4 + 1*2 + 2 = 16
//   pool calls: 2 std + 1 stable + 1 conc
//   plane calls: 4
//
// strict_receive 4-pool (2 std + 1 stable + 1 conc):
//   estimate pass: 3 pools with sync_reserves * 2 balance = 6, conc = 0 → 6
//   swap pass: 0 balance (skipped), 2 transfer * 4 = 8
//   router: 5 (1 bal pre + 1 xfer in + 1 xfer out + 1 bal refund + 1 xfer refund)
//   total token calls: 6 + 8 + 5 = 19
//   pool calls: 2*2 std + 2*1 stable + 2*1 conc = 8 (estimate + swap for each)
//   plane calls: 4

#[test]
fn bench_mixed_4pool_strict_send() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_mixed_4pool_chain(&setup);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &10_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let in_amount: u128 = 10_0000000;

    // 2 std + 1 stable + 1 conc: total token calls = 3*4 + 1*2 + 2 = 16
    // pool calls: 1 swap per pool = 2 std + 1 stable + 1 conc
    // plane calls: 4
    // custom_token_calls: 0 (all SAC in test; on mainnet pass actual count)
    measure_budget_with_vm_mixed(
        &setup.env,
        "mixed 4-pool strict_send (2std+1stable+1conc)",
        &ROUTER_MASTER,
        &[
            ("standard", &POOL_MASTER, 2),
            ("stableswap", &STABLESWAP_POOL_MASTER, 1),
            ("concentrated", &CONC_POOL_MASTER, 1),
        ],
        &TOKEN_MASTER,
        &PLANE_MASTER,
        0, // all SAC tokens in test
        4,
        || {
            setup
                .router
                .swap_chained(&user, &chain, &token_in, &in_amount, &0);
        },
    );
}

#[test]
fn bench_mixed_4pool_strict_receive() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_mixed_4pool_chain(&setup);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let out_amount: u128 = 5_0000000;
    let in_max: u128 = 100_0000000;

    // estimate + swap for each pool: 4 std + 2 stable + 2 conc
    // total token calls: 6 balance (estimate, 3 non-conc pools) + 8 transfer (swap) + 5 router = 19
    // plane calls: 4
    // custom_token_calls: 0 (all SAC in test)
    measure_budget_with_vm_mixed(
        &setup.env,
        "mixed 4-pool strict_receive (2std+1stable+1conc)",
        &ROUTER_MASTER,
        &[
            ("standard", &POOL_MASTER, 4),
            ("stableswap", &STABLESWAP_POOL_MASTER, 2),
            ("concentrated", &CONC_POOL_MASTER, 2),
        ],
        &TOKEN_MASTER,
        &PLANE_MASTER,
        0, // all SAC tokens in test
        4,
        || {
            setup.router.swap_chained_strict_receive(
                &user,
                &chain,
                &token_in,
                &out_amount,
                &in_max,
            );
        },
    );
}

// Also test the testnet-matching topology: 3 standard + 1 concentrated

/// Set up a 4-pool chain matching testnet: standard → standard → concentrated → standard
fn setup_testnet_4pool_chain<'a>(
    setup: &'a Setup<'a>,
) -> (
    std::vec::Vec<TokenClient<'a>>,
    std::vec::Vec<soroban_sdk::BytesN<32>>,
) {
    let mut tokens: std::vec::Vec<Address> = (0..=4)
        .map(|_| create_token_contract(&setup.env, &setup.admin).address)
        .collect();
    tokens.sort();

    let token_clients: std::vec::Vec<TokenClient> = tokens
        .iter()
        .map(|t| TokenClient::new(&setup.env, t))
        .collect();

    let mut pool_hashes = std::vec::Vec::new();

    // Pools 0, 1, 3: standard. Pool 2: concentrated.
    for i in 0..4usize {
        let admin_a = get_token_admin_client(&setup.env, &token_clients[i].address);
        let admin_b = get_token_admin_client(&setup.env, &token_clients[i + 1].address);

        if i == 2 {
            // concentrated pool
            let (_pool, pool_hash) = setup.deploy_concentrated_pool(
                &token_clients[i].address,
                &token_clients[i + 1].address,
                30,
            );
            admin_a.mint(&setup.admin, &100_000_0000000);
            admin_b.mint(&setup.admin, &100_000_0000000);
            _pool.deposit(
                &setup.admin,
                &Vec::from_array(&setup.env, [100_000_0000000u128, 100_000_0000000u128]),
                &0,
            );
            pool_hashes.push(pool_hash);
        } else {
            // standard pool
            let (_pool, pool_hash) = setup.deploy_standard_pool(
                &token_clients[i].address,
                &token_clients[i + 1].address,
                30,
            );
            admin_a.mint(&setup.admin, &1_000_000_0000000);
            admin_b.mint(&setup.admin, &1_000_000_0000000);
            _pool.deposit(
                &setup.admin,
                &Vec::from_array(&setup.env, [1_000_000_0000000, 1_000_000_0000000]),
                &0,
            );
            pool_hashes.push(pool_hash);
        }
    }

    (token_clients, pool_hashes)
}

#[test]
fn bench_testnet_4pool_strict_send() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_testnet_4pool_chain(&setup);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &10_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let in_amount: u128 = 10_0000000;

    // 3 std + 1 conc: total token calls = 3*4 + 1*2 + 2 = 16
    // On testnet: 3 custom token calls (yTIME1), rest SAC. In test: all SAC → 0.
    measure_budget_with_vm_mixed(
        &setup.env,
        "testnet 4-pool strict_send (3std+1conc)",
        &ROUTER_MASTER,
        &[
            ("standard", &POOL_MASTER, 3),
            ("concentrated", &CONC_POOL_MASTER, 1),
        ],
        &TOKEN_MASTER,
        &PLANE_MASTER,
        0,
        4,
        || {
            setup
                .router
                .swap_chained(&user, &chain, &token_in, &in_amount, &0);
        },
    );
}

#[test]
fn bench_testnet_4pool_strict_receive() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_testnet_4pool_chain(&setup);

    let user = Address::generate(&setup.env);
    get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);

    let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
    let token_in = tokens[0].address.clone();
    let out_amount: u128 = 5_0000000;
    let in_max: u128 = 100_0000000;

    // 3 std estimate + 3 std swap + 1 conc estimate + 1 conc swap = 6 std + 2 conc
    // total token calls: 3*2 balance (estimate sync) + 4*2 transfer (swap) + 5 router = 19
    // custom_token_calls: 0 (all SAC in test)
    measure_budget_with_vm_mixed(
        &setup.env,
        "testnet 4-pool strict_receive (3std+1conc)",
        &ROUTER_MASTER,
        &[
            ("standard", &POOL_MASTER, 6),
            ("concentrated", &CONC_POOL_MASTER, 2),
        ],
        &TOKEN_MASTER,
        &PLANE_MASTER,
        0,
        4,
        || {
            setup.router.swap_chained_strict_receive(
                &user,
                &chain,
                &token_in,
                &out_amount,
                &in_max,
            );
        },
    );
}

// --- Testnet-accurate estimate with yTIME1 custom token overhead ---
// yTIME1 (CC52...70BT) is a 60KB custom WASM token, ~6x standard token.
// It appears in pools 2 and 3, contributing ~3 cross-contract calls.

#[test]
fn bench_testnet_4pool_with_ytime1() {
    let setup = Setup::default();
    let (tokens, pool_hashes) = setup_testnet_4pool_chain(&setup);

    // --- strict_send ---
    {
        let user = Address::generate(&setup.env);
        get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &10_0000000);
        let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
        let token_in = tokens[0].address.clone();
        let in_amount: u128 = 10_0000000;

        // Run swap to get measured memory (SAC tokens in test)
        setup.env.cost_estimate().budget().reset_unlimited();
        setup.env.cost_estimate().budget().reset_tracker();
        setup
            .router
            .swap_chained(&user, &chain, &token_in, &in_amount, &0);
        let measured_mem = setup.env.cost_estimate().budget().memory_bytes_cost();

        // Compute VM overhead with yTIME1: 3 custom token calls (balance+transfer in pool2, transfer in pool3)
        let vm_mem = estimate_vm_overhead_mixed(
            "testnet 4-pool strict_send (3std+1conc, yTIME1)",
            &ROUTER_MASTER,
            &[
                ("standard", &POOL_MASTER, 3),
                ("concentrated", &CONC_POOL_MASTER, 1),
            ],
            &YTIME1_TOKEN,
            &PLANE_MASTER,
            3, // yTIME1 calls: balance(pool2 sync) + transfer(pool2→router) + transfer(router→pool3)
            4,
        );

        let total = measured_mem + vm_mem;
        std::println!("=== Testnet-accurate: strict_send (3std+1conc, yTIME1) ===");
        std::println!(
            "  Measured (SAC): {} ({:.2} MB)",
            measured_mem,
            measured_mem as f64 / 1_048_576.0
        );
        std::println!(
            "  VM overhead:    {} ({:.2} MB)",
            vm_mem,
            vm_mem as f64 / 1_048_576.0
        );
        std::println!(
            "  TOTAL:          {} ({:.2} MB)",
            total,
            total as f64 / 1_048_576.0
        );
        std::println!("  Testnet actual: 30321879 (28.92 MB)");
        std::println!(
            "  Delta:          {:.2} MB",
            total as f64 / 1_048_576.0 - 28.92
        );
        std::println!();
    }

    // --- strict_receive ---
    {
        let user = Address::generate(&setup.env);
        get_token_admin_client(&setup.env, &tokens[0].address).mint(&user, &100_0000000);
        let chain = build_swap_chain(&setup.env, &tokens, &pool_hashes);
        let token_in = tokens[0].address.clone();

        setup.env.cost_estimate().budget().reset_unlimited();
        setup.env.cost_estimate().budget().reset_tracker();
        setup.router.swap_chained_strict_receive(
            &user,
            &chain,
            &token_in,
            &5_0000000u128,
            &100_0000000u128,
        );
        let measured_mem = setup.env.cost_estimate().budget().memory_bytes_cost();

        // yTIME1 calls in strict_receive: balance(pool2 estimate sync) + transfer(pool2 swap) + transfer(pool3 swap) = 3
        let vm_mem = estimate_vm_overhead_mixed(
            "testnet 4-pool strict_receive (3std+1conc, yTIME1)",
            &ROUTER_MASTER,
            &[
                ("standard", &POOL_MASTER, 6),
                ("concentrated", &CONC_POOL_MASTER, 2),
            ],
            &YTIME1_TOKEN,
            &PLANE_MASTER,
            3, // yTIME1 calls
            4,
        );

        let total = measured_mem + vm_mem;
        std::println!("=== Testnet-accurate: strict_receive (3std+1conc, yTIME1) ===");
        std::println!(
            "  Measured (SAC): {} ({:.2} MB)",
            measured_mem,
            measured_mem as f64 / 1_048_576.0
        );
        std::println!(
            "  VM overhead:    {} ({:.2} MB)",
            vm_mem,
            vm_mem as f64 / 1_048_576.0
        );
        std::println!(
            "  TOTAL:          {} ({:.2} MB)",
            total,
            total as f64 / 1_048_576.0
        );
        std::println!("  Mainnet limit:  41943040 (40.00 MB)");
        let headroom = 41943040i64 - total as i64;
        std::println!("  Headroom:       {:+.2} MB", headroom as f64 / 1_048_576.0);
        std::println!();
    }
}
