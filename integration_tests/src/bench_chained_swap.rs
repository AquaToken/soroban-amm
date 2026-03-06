#![cfg(test)]
extern crate std;

use crate::testutils::{
    create_token_contract, estimate_vm_overhead, get_token_admin_client, measure_budget_with_vm,
    Setup, SetupV170, PLANE_MASTER, PLANE_V170, POOL_MASTER, POOL_V170, ROUTER_MASTER, ROUTER_V170,
    TOKEN_MASTER, TOKEN_V170,
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
