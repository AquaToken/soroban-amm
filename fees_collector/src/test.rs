#![cfg(test)]
extern crate std;

use crate::testutils::{create_contract, install_dummy_wasm, jump, Setup};
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::{Address as _, Events};
use soroban_sdk::{symbol_short, vec, Address, Env, IntoVal, Symbol};

#[test]
fn test() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

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

    collector.commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &new_admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                collector.address.clone(),
                (
                    Symbol::new(&setup.env, "commit_transfer_ownership"),
                    symbol_short!("Admin")
                )
                    .into_val(&setup.env),
                (new_admin.clone(),).into_val(&setup.env),
            ),
        ]
    );

    collector.revert_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                collector.address.clone(),
                (
                    Symbol::new(&setup.env, "revert_transfer_ownership"),
                    symbol_short!("Admin")
                )
                    .into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );

    collector.commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &new_admin);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    collector.apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                collector.address.clone(),
                (
                    Symbol::new(&setup.env, "apply_transfer_ownership"),
                    symbol_short!("Admin")
                )
                    .into_val(&setup.env),
                (new_admin.clone(),).into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_upgrade_events() {
    let setup = Setup::default();
    let contract = setup.collector;
    let new_wasm_hash = install_dummy_wasm(&setup.env);

    contract.commit_upgrade(&setup.admin, &new_wasm_hash);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "commit_upgrade"),).into_val(&setup.env),
                (new_wasm_hash.clone(),).into_val(&setup.env),
            ),
        ]
    );

    contract.revert_upgrade(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "revert_upgrade"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );

    contract.commit_upgrade(&setup.admin, &new_wasm_hash);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    contract.apply_upgrade(&setup.admin);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "apply_upgrade"),).into_val(&setup.env),
                (new_wasm_hash.clone(),).into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_emergency_mode_events() {
    let setup = Setup::default();
    let contract = setup.collector;

    contract.set_emergency_mode(&setup.emergency_admin, &true);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "enable_emergency_mode"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );
    contract.set_emergency_mode(&setup.emergency_admin, &false);
    assert_eq!(
        vec![&setup.env, setup.env.events().all().last().unwrap()],
        vec![
            &setup.env,
            (
                contract.address.clone(),
                (Symbol::new(&setup.env, "disable_emergency_mode"),).into_val(&setup.env),
                ().into_val(&setup.env),
            ),
        ]
    );
}

#[test]
fn test_emergency_upgrade() {
    let setup = Setup::default();
    let contract = setup.collector;
    let new_wasm = install_dummy_wasm(&setup.env);

    assert_eq!(contract.get_emergency_mode(), false);
    assert_ne!(contract.version(), 130);
    contract.set_emergency_mode(&setup.emergency_admin, &true);

    contract.commit_upgrade(&setup.admin, &new_wasm);
    contract.apply_upgrade(&setup.admin);

    assert_eq!(contract.version(), 130)
}

#[test]
fn test_regular_upgrade() {
    let setup = Setup::default();
    let contract = setup.collector;
    let new_wasm = install_dummy_wasm(&setup.env);

    assert_eq!(contract.get_emergency_mode(), false);
    assert_ne!(contract.version(), 130);

    contract.commit_upgrade(&setup.admin, &new_wasm);
    assert!(contract.try_apply_upgrade(&setup.admin).is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    contract.apply_upgrade(&setup.admin);

    assert_eq!(contract.version(), 130)
}
