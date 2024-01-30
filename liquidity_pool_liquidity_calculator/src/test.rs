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

    e.budget().reset_default();
    let results = calculator.get_liquidity(&Vec::from_array(
        &e,
        [
            address1.clone(),
            address2.clone(),
            address3.clone(),
            address4.clone(),
            address5.clone(),
            address6.clone(),
        ],
    ));
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(
        results,
        Vec::from_array(
            &e,
            [365671400, 547424532, 55233846, 2802265656, 2802226894, 2802235260,]
        )
    );
}

#[test]
fn test_norm() {
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
        &Vec::from_array(&e, [250_500_000_0000_u128, 50_000_000_0000_u128]),
    );
    plane.update(
        &address2,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [30_u128]),
        &Vec::from_array(&e, [25_000_000_0000_u128, 5_000_000_0000_u128]),
    );
    plane.update(
        &address3,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [300_u128]),
        &Vec::from_array(&e, [2_500_000_0000_u128, 0_500_000_0000_u128]),
    );
    plane.update(
        &address4,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [30_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [250_500_000_0000_u128, 50_000_000_0000_u128]),
    );
    plane.update(
        &address5,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [30_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [25_000_000_0000_u128, 5_000_000_0000_u128]),
    );
    plane.update(
        &address6,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [30_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [2_500_000_0000_u128, 0_500_000_0000_u128]),
    );

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let results = calculator.get_liquidity(&Vec::from_array(
        &e,
        [
            address1.clone(),
            address2.clone(),
            address3.clone(),
            address4.clone(),
            address5.clone(),
            address6.clone(),
        ],
    ));
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(
        results,
        Vec::from_array(
            &e,
            [
                54969136250,
                5487768725,
                563878134,
                345061368750,
                34478855100,
                3447885296,
            ]
        )
    );
}

#[test]
fn test_small() {
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
        &Vec::from_array(&e, [10_u128, 10_u128]),
    );
    plane.update(
        &address2,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [30_u128]),
        &Vec::from_array(&e, [100_u128, 100_u128]),
    );
    plane.update(
        &address3,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [300_u128]),
        &Vec::from_array(&e, [1000_u128, 1000_u128]),
    );
    plane.update(
        &address4,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [30_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [10_u128, 10_u128]),
    );
    plane.update(
        &address5,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [30_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [100_u128, 100_u128]),
    );
    plane.update(
        &address6,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [30_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [1000_u128, 1000_u128]),
    );

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let results = calculator.get_liquidity(&Vec::from_array(
        &e,
        [
            address1.clone(),
            address2.clone(),
            address3.clone(),
            address4.clone(),
            address5.clone(),
            address6.clone(),
        ],
    ));
    e.budget().reset_unlimited();
    assert_eq!(results, Vec::from_array(&e, [0, 2, 36, 2, 28, 280,]));
}

#[test]
fn test_reversed() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let address1 = Address::generate(&e);
    let address2 = Address::generate(&e);
    let address3 = Address::generate(&e);
    let address4 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    plane.update(
        &address1,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [30_u128]),
        &Vec::from_array(&e, [1000_u128, 3000_u128]),
    );
    plane.update(
        &address2,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [30_u128]),
        &Vec::from_array(&e, [3000_u128, 1000_u128]),
    );
    plane.update(
        &address3,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [30_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [1000_u128, 3000_u128]),
    );
    plane.update(
        &address4,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [30_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [3000_u128, 1000_u128]),
    );

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let results = calculator.get_liquidity(&Vec::from_array(
        &e,
        [
            address1.clone(),
            address2.clone(),
            address3.clone(),
            address4.clone(),
        ],
    ));
    e.budget().reset_unlimited();
    assert_eq!(results, Vec::from_array(&e, [72, 72, 529, 529,]));
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
        &Vec::from_array(&e, [u128::MAX, u128::MAX]),
    );
    plane.update(
        &address2,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [u128::MAX, u128::MAX]),
    );

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let results =
        calculator.get_liquidity(&Vec::from_array(&e, [address1.clone(), address2.clone()]));
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(
        results,
        Vec::from_array(
            &e,
            [
                12443152950729325724850104142337850200,
                95354840013982973403610470893703822242,
            ]
        )
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
    let address3 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    plane.update(
        &address1,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [u128::MAX / 3, u128::MAX / 3, u128::MAX / 3]),
    );
    plane.update(
        &address2,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(
            &e,
            [u128::MAX / 3, u128::MAX / 3, u128::MAX / 3, u128::MAX / 3],
        ),
    );
    plane.update(
        &address3,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(
            &e,
            [
                u128::MAX / 3,
                u128::MAX / 3,
                u128::MAX / 3,
                u128::MAX / 3,
                u128::MAX / 3,
            ],
        ),
    );

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let results = calculator.get_liquidity(&Vec::from_array(
        &e,
        [address1.clone(), address2.clone(), address3.clone()],
    ));
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(
        results,
        Vec::from_array(
            &e,
            [
                95354840013982973403610470888099368454,
                190709680027965946807220941776198736908,
                317849466713276578012034902960331228180,
            ]
        )
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
    let results =
        calculator.get_liquidity(&Vec::from_array(&e, [address1.clone(), address2.clone()]));
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(results, Vec::from_array(&e, [0, 0]));
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
    let results =
        calculator.get_liquidity(&Vec::from_array(&e, [address1.clone(), address2.clone()]));
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(results, Vec::from_array(&e, [0, 0]));
}
