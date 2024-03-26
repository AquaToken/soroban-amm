#![cfg(test)]
extern crate std;

use crate::{contract::LiquidityPoolSwapRouter, LiquidityPoolSwapRouterClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Env, Vec};

fn create_contract<'a>(e: &Env) -> LiquidityPoolSwapRouterClient<'a> {
    let client = LiquidityPoolSwapRouterClient::new(
        e,
        &e.register_contract(None, LiquidityPoolSwapRouter {}),
    );
    client
}

mod pool_plane {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_plane_contract.wasm"
    );
}

fn create_plane_contract<'a>(e: &Env) -> pool_plane::Client<'a> {
    pool_plane::Client::new(e, &e.register_contract_wasm(None, pool_plane::WASM))
}

#[test]
#[should_panic(expected = "Error(Contract, #103)")]
fn test_init_admin_twice() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let router = create_contract(&e);
    router.init_admin(&admin);
    router.init_admin(&admin);
}

#[test]
fn test() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let address1 = Address::generate(&e);
    let address2 = Address::generate(&e);
    let address3 = Address::generate(&e);
    let address4 = Address::generate(&e);
    let address5 = Address::generate(&e);
    let address6 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    plane.update(
        &address1,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [30_u128]),
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
    );
    plane.update(
        &address2,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [10_u128]),
        &Vec::from_array(&e, [1500_0000000_u128, 1500_0000000_u128]),
    );
    plane.update(
        &address3,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [100_u128]),
        &Vec::from_array(&e, [150_0000000_u128, 15_0000000_u128]),
    );
    plane.update(
        &address4,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [20_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [150_0000000_u128, 150_0000000_u128]),
    );
    plane.update(
        &address5,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [150_0000000_u128, 150_0000000_u128]),
    );
    plane.update(
        &address6,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [14_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [150_0000000_u128, 150_0000000_u128]),
    );

    let router = create_contract(&e);
    router.init_admin(&admin);
    router.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let (best_pool, best_result) = router.estimate_swap(
        &Vec::from_array(
            &e,
            [
                address1.clone(),
                address2.clone(),
                address3.clone(),
                address4.clone(),
                address5.clone(),
                address6.clone(),
            ],
        ),
        &0,
        &1,
        &42_0000000,
    );
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(best_pool, address5);
    assert_eq!(best_result, 41_8273777);
}

#[test]
fn test_empty_pool() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let address1 = Address::generate(&e);
    let address2 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    plane.update(
        &address1,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [30_u128]),
        &Vec::from_array(&e, [0_u128, 0_u128]),
    );
    plane.update(
        &address2,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [0_u128, 0_u128]),
    );

    let router = create_contract(&e);
    router.init_admin(&admin);
    router.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let (best_pool, best_result) = router.estimate_swap(
        &Vec::from_array(&e, [address1.clone(), address2.clone()]),
        &0,
        &1,
        &42_0000000,
    );
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(best_pool, address1);
    assert_eq!(best_result, 0);
}

#[test]
fn test_bad_address() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let address1 = Address::generate(&e);
    let address2 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    plane.update(
        &address2,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [30_u128]),
        &Vec::from_array(&e, [0_u128, 0_u128]),
    );

    let router = create_contract(&e);
    router.init_admin(&admin);
    router.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let (best_pool, best_result) = router.estimate_swap(
        &Vec::from_array(&e, [address1.clone(), address2.clone()]),
        &0,
        &1,
        &42_0000000,
    );
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(best_pool, address1);
    assert_eq!(best_result, 0);
}
#[test]
fn test_large_numbers() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let reserves = u128::MAX / 1_000_000;
    let swap_in = reserves / 1_000;
    // swap out shouldn't differ for more than 0.1% since we're getting best swap through stableswap with fee 0.06%
    let expected_swap_result_delta = swap_in / 1000;

    let admin = Address::generate(&e);
    let address1 = Address::generate(&e);
    let address2 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    plane.update(
        &address1,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [30_u128]),
        &Vec::from_array(&e, [reserves, reserves]),
    );
    plane.update(
        &address2,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [reserves, reserves]),
    );

    let router = create_contract(&e);
    router.init_admin(&admin);
    router.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let (best_pool, best_result) = router.estimate_swap(
        &Vec::from_array(&e, [address1.clone(), address2.clone()]),
        &0,
        &1,
        &swap_in,
    );
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(best_pool, address2);
    assert!(swap_in - best_result < expected_swap_result_delta);
}
