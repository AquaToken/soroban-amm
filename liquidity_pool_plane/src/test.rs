#![cfg(test)]
extern crate std;

use crate::{contract::LiquidityPoolPlane, LiquidityPoolPlaneClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Env, Vec};

fn create_plane_contract<'a>(e: &Env) -> LiquidityPoolPlaneClient<'a> {
    let client =
        LiquidityPoolPlaneClient::new(e, &e.register_contract(None, LiquidityPoolPlane {}));
    client
}

#[test]
fn test() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let address1 = Address::generate(&e);
    let address2 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    plane.update(
        &address1,
        &symbol_short!("standard"),
        &Vec::from_array(&e, [30_u128]),
        &Vec::from_array(&e, [1000_u128, 1000_u128]),
    );
    plane.update(
        &address2,
        &symbol_short!("stable"),
        &Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128]),
        &Vec::from_array(&e, [800_u128, 900_u128]),
    );
    let data = plane.get(&Vec::from_array(&e, [address1, address2]));

    let data1 = data.get(0).unwrap();
    assert_eq!(data1.0, symbol_short!("standard"));
    assert_eq!(data1.1, Vec::from_array(&e, [30_u128]));
    assert_eq!(data1.2, Vec::from_array(&e, [1000_u128, 1000_u128]));

    let data2 = data.get(1).unwrap();
    assert_eq!(data2.0, symbol_short!("stable"));
    assert_eq!(
        data2.1,
        Vec::from_array(&e, [6_u128, 85_u128, 0_u128, 85_u128, 0_u128])
    );
    assert_eq!(data2.2, Vec::from_array(&e, [800_u128, 900_u128]));
}
