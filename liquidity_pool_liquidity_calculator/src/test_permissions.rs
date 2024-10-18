#![cfg(test)]

use crate::testutils::{install_dummy_wasm, jump, Setup};
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

// test transfer ownership
#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_transfer_ownership_too_early() {
    let setup = Setup::default();
    let calculator = setup.calculator;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    calculator.commit_transfer_ownership(&admin_original, &admin_new);
    // check admin not changed yet by calling protected method
    assert!(calculator
        .try_revert_transfer_ownership(&admin_new)
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY - 1);
    calculator.apply_transfer_ownership(&admin_original);
}

#[test]
#[should_panic(expected = "Error(Contract, #2906)")]
fn test_transfer_ownership_twice() {
    let setup = Setup::default();
    let calculator = setup.calculator;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    calculator.commit_transfer_ownership(&admin_original, &admin_new);
    calculator.commit_transfer_ownership(&admin_original, &admin_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_transfer_ownership_not_committed() {
    let setup = Setup::default();
    let calculator = setup.calculator;
    let admin_original = setup.admin;

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    calculator.apply_transfer_ownership(&admin_original);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_transfer_ownership_reverted() {
    let setup = Setup::default();
    let calculator = setup.calculator;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    calculator.commit_transfer_ownership(&admin_original, &admin_new);
    // check admin not changed yet by calling protected method
    assert!(calculator
        .try_revert_transfer_ownership(&admin_new)
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    calculator.revert_transfer_ownership(&admin_original);
    calculator.apply_transfer_ownership(&admin_original);
}

#[test]
fn test_transfer_ownership() {
    let setup = Setup::default();
    let calculator = setup.calculator;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    calculator.commit_transfer_ownership(&admin_original, &admin_new);
    // check admin not changed yet by calling protected method
    assert!(calculator
        .try_revert_transfer_ownership(&admin_new)
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    calculator.apply_transfer_ownership(&admin_original);

    calculator.commit_transfer_ownership(&admin_new, &admin_new);
}

// upgrade
#[test]
fn test_upgrade_third_party_user() {
    let setup = Setup::default();
    let calculator = setup.calculator;
    let user = Address::generate(&setup.env);
    assert!(calculator
        .try_upgrade(&user, &install_dummy_wasm(&setup.env))
        .is_err());
}

#[test]
fn test_upgrade_admin() {
    let setup = Setup::default();
    let calculator = setup.calculator;
    assert!(calculator
        .try_upgrade(&setup.admin, &install_dummy_wasm(&setup.env))
        .is_ok());
}
