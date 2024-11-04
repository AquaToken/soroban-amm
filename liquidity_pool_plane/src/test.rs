#![cfg(test)]
extern crate std;

use crate::testutils::{jump, Setup};
use crate::{contract::LiquidityPoolPlane, LiquidityPoolPlaneClient};
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{symbol_short, vec, Address, Env, IntoVal, Symbol, Vec};

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
    plane.init_admin(&Address::generate(&e));
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

#[should_panic(expected = "Error(Contract, #103)")]
#[test]
fn test_init_admin_twice() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let plane = create_plane_contract(&e);
    plane.init_admin(&admin);
    plane.init_admin(&admin);
}

#[test]
fn test_transfer_ownership_events() {
    let setup = Setup::default();
    let plane = setup.plane;
    let new_admin = Address::generate(&setup.env);

    plane.commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &new_admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                plane.address.clone(),
                (Symbol::new(&setup.env, "commit_transfer_ownership"),).into_val(&setup.env),
                (symbol_short!("Admin"), new_admin.clone(),).into_val(&setup.env),
            ),
        ]
    );

    plane.revert_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                plane.address.clone(),
                (Symbol::new(&setup.env, "revert_transfer_ownership"),).into_val(&setup.env),
                (symbol_short!("Admin"),).into_val(&setup.env),
            ),
        ]
    );

    plane.commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &new_admin);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    plane.apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                plane.address.clone(),
                (Symbol::new(&setup.env, "apply_transfer_ownership"),).into_val(&setup.env),
                (symbol_short!("Admin"), new_admin.clone(),).into_val(&setup.env),
            ),
        ]
    );
}
