#![cfg(test)]
extern crate std;

use crate::{contract::LiquidityPoolLiquidityCalculator, LiquidityPoolLiquidityCalculatorClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Env, Vec};

fn create_contract<'a>(e: &Env) -> LiquidityPoolLiquidityCalculatorClient<'a> {
    let client = LiquidityPoolLiquidityCalculatorClient::new(
        e,
        &e.register_contract(None, LiquidityPoolLiquidityCalculator {}),
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
        &Vec::from_array(&e, [150_0000000_u128, 150_0000000_u128]),
    );
    plane.update(
        &address4,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [0_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        // &Vec::from_array(&e, [30000_u128, 30000_u128]),
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
    );
    plane.update(
        &address5,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        // &Vec::from_array(&e, [30000_u128, 30000_u128]),
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
    );
    plane.update(
        &address6,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [30_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        // &Vec::from_array(&e, [30000_u128, 30000_u128]),
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
    );

    // why stable pool with fee 1% has more liquidity than pool with 0.1%
    // 3576525504 - 0.1%
    // 3576533246 - 0.3%
    // 3576530062 - 1%

    // 3570071208
    // 3573924372
    // 3576533246

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let results= calculator.get_liquidity(
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
    );
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(
        results,
        Vec::from_array(&e, [
            372345266,
            557484114,
            56218710,
            380340640,
            380338812,
            2535592254,
        ])
    );
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
        &Vec::from_array(&e, [30_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [0_u128, 0_u128]),
    );

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let results = calculator.get_liquidity(
        &Vec::from_array(&e, [address1.clone(), address2.clone()]),
    );
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(
        results,
        Vec::from_array(&e, [0, 0])
    );
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

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let results = calculator.get_liquidity(
        &Vec::from_array(&e, [address1.clone(), address2.clone()]),
    );
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(
        results,
        Vec::from_array(&e, [0, 0])
    );
}
