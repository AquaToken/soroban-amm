#![cfg(test)]
extern crate std;

use crate::testutils::{create_contract, jump, Setup};
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{vec, Address, Env, IntoVal, Symbol};

#[test]
fn test() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let collector = create_contract(&e);
    collector.init_admin(&admin);
}

#[should_panic(expected = "Error(Contract, #103)")]
#[test]
fn test_init_admin_twice() {
    let setup = Setup::default();
    setup.collector.init_admin(&setup.admin);
}

#[test]
fn test_transfer_ownership_events() {
    let setup = Setup::default();
    let collector = setup.collector;
    let new_admin = Address::generate(&setup.env);

    collector.commit_transfer_ownership(&setup.admin, &new_admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                collector.address.clone(),
                (Symbol::new(&setup.env, "commit_transfer_ownership"),).into_val(&setup.env),
                (new_admin.clone(),).into_val(&setup.env),
            ),
        ]
    );

    collector.revert_transfer_ownership(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                collector.address.clone(),
                (Symbol::new(&setup.env, "revert_transfer_ownership"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );

    collector.commit_transfer_ownership(&setup.admin, &new_admin);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    collector.apply_transfer_ownership(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                collector.address.clone(),
                (Symbol::new(&setup.env, "apply_transfer_ownership"),).into_val(&setup.env),
                (new_admin.clone(),).into_val(&setup.env),
            ),
        ]
    );
}
