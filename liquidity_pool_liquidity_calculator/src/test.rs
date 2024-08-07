#![cfg(test)]
extern crate std;

use crate::{contract::LiquidityPoolLiquidityCalculator, LiquidityPoolLiquidityCalculatorClient};
use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
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

fn jump(e: &Env, time: u64) {
    e.ledger().set(LedgerInfo {
        timestamp: e.ledger().timestamp().saturating_add(time),
        protocol_version: e.ledger().protocol_version(),
        sequence_number: e.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 999999,
        min_persistent_entry_ttl: 999999,
        max_entry_ttl: u32::MAX,
    });
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
                U256::from_u128(&e, 364591028),
                U256::from_u128(&e, 546886550),
                U256::from_u128(&e, 54688608),
                U256::from_u128(&e, 2802265656),
                U256::from_u128(&e, 2802262932),
                U256::from_u128(&e, 2802267364),
            ]
        )
    );
}

#[test]
fn test_bad_math() {
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

    jump(&e, 1813808460);

    let plane = create_plane_contract(&e);
    plane.update(
        &address1,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [100]),
        &Vec::from_array(&e, [0, 0]),
    );
    plane.update(
        &address2,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [100, 2500, 1713807096, 2500, 1713807096]),
        &Vec::from_array(&e, [101804393, 160000000000000]),
    );
    plane.update(
        &address3,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [5, 5000, 1713808459, 5000, 1713808459]),
        &Vec::from_array(&e, [219219286, 16538316775072]),
    );
    plane.update(
        &address4,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [30]),
        &Vec::from_array(&e, [0, 0]),
    );
    plane.update(
        &address5,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [10]),
        &Vec::from_array(&e, [485714287, 210000000000000]),
    );
    plane.update(
        &address6,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [100, 5000, 1713806916, 5000, 1713806916]),
        &Vec::from_array(&e, [102098314, 110000000000000]),
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
                U256::from_u128(&e, 0),
                U256::from_u128(&e, 12208000),
                U256::from_u128(&e, 131221069311),
                U256::from_u128(&e, 0),
                U256::from_u128(&e, 3075607613295000),
                U256::from_u128(&e, 630244655854000),
            ]
        )
    );
}

#[test]
fn test_bad_math_2() {
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
        &Vec::from_array(&e, [6, 85, 1718465537, 85, 1718465537]),
        &Vec::from_array(&e, [96474122, 7654213018638340]),
    );
    plane.update(
        &address2,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [30]),
        &Vec::from_array(&e, [107478636, 222947860089606]),
    );

    jump(&e, 1918465537);

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
        Vec::from_array(&e, [U256::from_u128(&e, 0), U256::from_u128(&e, 0),])
    );
}

#[test]
fn test_out_of_fuel() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let address1 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    // it gives us 102M operations for real contract
    plane.update(
        &address1,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [10, 5000, 1718230749, 5000, 1718230749]),
        &Vec::from_array(&e, [5157363573, 61817519211, 11875744570237, 5866985200]),
    );

    jump(&e, 1918465537);

    let calculator = create_contract(&e);
    calculator.init_admin(&admin);
    calculator.set_pools_plane(&admin, &plane.address);

    e.budget().reset_default();
    let results = calculator.get_liquidity(&Vec::from_array(&e, [address1.clone()]));
    e.budget().print();
    e.budget().reset_unlimited();
    assert_eq!(
        results,
        Vec::from_array(&e, [U256::from_u128(&e, 345242385178),])
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
                U256::from_u128(&e, 54805764500),
                U256::from_u128(&e, 5471429400),
                U256::from_u128(&e, 547144562),
                U256::from_u128(&e, 345062872750),
                U256::from_u128(&e, 34479140125),
                U256::from_u128(&e, 3447914006),
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
                U256::from_u128(&e, 12406389796597814911885218847200013804),
                U256::from_u128(&e, 95356066323576883081645100203114078476),
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
    // assert_eq!(
    //     results.get(0).unwrap().to_be_bytes(),
    //     Bytes::from_array(&e, &[0])
    // );
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
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 215, 54, 187, 38, 81,
                            230, 97, 36, 91, 116, 178, 233, 122, 252, 227, 36
                        ]
                    )
                ),
                U256::from_be_bytes(
                    &e,
                    &Bytes::from_array(
                        &e,
                        &[
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 174, 109, 118, 76, 163,
                            204, 194, 72, 182, 233, 101, 210, 245, 249, 198, 72
                        ]
                    )
                ),
                U256::from_be_bytes(
                    &e,
                    &Bytes::from_array(
                        &e,
                        &[
                            0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 2, 205, 97, 26, 127, 187,
                            170, 153, 35, 219, 132, 255, 10, 68, 160, 74, 120
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
