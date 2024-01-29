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
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
    );
    plane.update(
        &address5,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
    );
    plane.update(
        &address6,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [30_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [1000_0000000_u128, 1000_0000000_u128]),
    );

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    // e.budget().reset_default();
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
            365453042,
            547105750,
            55200700,
            2802293296,
            2802263728,
            2802267760,
        ])
    );
}

#[test]
fn test_liquidity_overflow() {
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
        &Vec::from_array(&e, [u64::MAX as u128, u64::MAX as u128]),
    );
    plane.update(
        &address2,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [u64::MAX as u128, u64::MAX as u128]),
    );

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    // e.budget().reset_default();
    let results= calculator.get_liquidity(
        &Vec::from_array(
            &e,
            [
                address1.clone(),
                address2.clone(),
            ],
        ),
    );
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(
        results,
        Vec::from_array(&e, [
            674141876399876634,
            5169264198394322188,
        ])
    );
}

#[test]
fn test_multiple_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let address1 = Address::generate(&e);
    let address2 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    plane.update(
        &address1,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [u64::MAX as u128, u64::MAX as u128, u64::MAX as u128]),
    );
    plane.update(
        &address2,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [u64::MAX as u128, u64::MAX as u128, u64::MAX as u128, u64::MAX as u128]),
    );

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    // e.budget().reset_default();
    let results= calculator.get_liquidity(
        &Vec::from_array(
            &e,
            [
                address1.clone(),
                address2.clone(),
            ],
        ),
    );
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(
        results,
        Vec::from_array(&e, [
            15507792595182966564,
            31015585190365933128,
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
