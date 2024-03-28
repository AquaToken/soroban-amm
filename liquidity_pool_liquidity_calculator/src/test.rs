#![cfg(test)]
extern crate std;

use crate::{contract::LiquidityPoolLiquidityCalculator, LiquidityPoolLiquidityCalculatorClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Bytes, Env, Vec, U256};

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
            [
                U256::from_u128(&e, 365671400),
                U256::from_u128(&e, 547424532),
                U256::from_u128(&e, 55233846),
                U256::from_u128(&e, 2802265656),
                U256::from_u128(&e, 2802226894),
                U256::from_u128(&e, 2802235260),
            ]
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
                U256::from_u128(&e, 54969136250),
                U256::from_u128(&e, 5487768725),
                U256::from_u128(&e, 563878134),
                U256::from_u128(&e, 345061368750),
                U256::from_u128(&e, 34478855100),
                U256::from_u128(&e, 3447885296),
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
    assert_eq!(
        results,
        Vec::from_array(
            &e,
            [
                U256::from_u128(&e, 0),
                U256::from_u128(&e, 2),
                U256::from_u128(&e, 36),
                U256::from_u128(&e, 2),
                U256::from_u128(&e, 28),
                U256::from_u128(&e, 280),
            ]
        )
    );
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
    assert_eq!(
        results,
        Vec::from_array(
            &e,
            [
                U256::from_u128(&e, 72),
                U256::from_u128(&e, 72),
                U256::from_u128(&e, 529),
                U256::from_u128(&e, 529),
            ]
        )
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
                U256::from_u128(&e, 12443152950729325724850104142337850200),
                U256::from_u128(&e, 95354840013982973403610470893703822242),
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
        &Vec::from_array(&e, [u128::MAX, u128::MAX, u128::MAX]),
    );
    plane.update(
        &address2,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [u128::MAX, u128::MAX, u128::MAX, u128::MAX]),
    );
    plane.update(
        &address3,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [u128::MAX, u128::MAX, u128::MAX, u128::MAX, u128::MAX]),
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

    for i in [1, 2] {
        assert!(results.get(i).unwrap() > U256::from_u128(&e, u128::MAX));
    }
    assert_eq!(
        results,
        Vec::from_array(
            &e,
            [
                U256::from_be_bytes(
                    &e,
                    &Bytes::from_array(
                        &e,
                        &[
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 215, 54, 5, 195, 181,
                            227, 43, 124, 233, 49, 71, 208, 106, 111, 114, 230
                        ]
                    )
                ),
                U256::from_be_bytes(
                    &e,
                    &Bytes::from_array(
                        &e,
                        &[
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 174, 108, 11, 135, 107,
                            198, 86, 249, 210, 98, 143, 160, 212, 222, 229, 204
                        ]
                    )
                ),
                U256::from_be_bytes(
                    &e,
                    &Bytes::from_array(
                        &e,
                        &[
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 205, 94, 189, 225, 179,
                            159, 230, 75, 9, 78, 239, 97, 98, 200, 212, 84
                        ]
                    )
                ),
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
    assert_eq!(
        results,
        Vec::from_array(&e, [U256::from_u128(&e, 0), U256::from_u128(&e, 0)])
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
    let results =
        calculator.get_liquidity(&Vec::from_array(&e, [address1.clone(), address2.clone()]));
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(
        results,
        Vec::from_array(&e, [U256::from_u128(&e, 0), U256::from_u128(&e, 0)])
    );
}
