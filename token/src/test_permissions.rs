#![cfg(test)]

use crate::testutils::Setup;
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address};
use utils::test_utils::{install_dummy_wasm, jump};

// test transfer ownership
#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_transfer_ownership_too_early() {
    let setup = Setup::default();
    let collector = setup.token;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    collector.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    // check admin not changed yet by calling protected method
    assert!(collector
        .try_revert_transfer_ownership(&admin_new, &symbol_short!("Admin"))
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY - 1);
    collector.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2906)")]
fn test_transfer_ownership_twice() {
    let setup = Setup::default();
    let collector = setup.token;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    collector.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    collector.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_transfer_ownership_not_committed() {
    let setup = Setup::default();
    let collector = setup.token;
    let admin_original = setup.admin;

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    collector.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_transfer_ownership_reverted() {
    let setup = Setup::default();
    let collector = setup.token;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    collector.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    // check admin not changed yet by calling protected method
    assert!(collector
        .try_revert_transfer_ownership(&admin_new, &symbol_short!("Admin"))
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    collector.revert_transfer_ownership(&admin_original, &symbol_short!("Admin"));
    collector.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));
}

#[test]
fn test_transfer_ownership() {
    let setup = Setup::default();
    let collector = setup.token;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    collector.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    // check admin not changed yet by calling protected method
    assert!(collector
        .try_revert_transfer_ownership(&admin_new, &symbol_short!("Admin"))
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    collector.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));

    collector.commit_transfer_ownership(&admin_new, &symbol_short!("Admin"), &admin_new);
}

// upgrade
#[test]
fn test_upgrade_third_party_user() {
    let setup = Setup::default();
    let token = setup.token;
    let user = Address::generate(&setup.env);
    assert!(token
        .try_upgrade(&user, &install_dummy_wasm(&setup.env))
        .is_err());
}

#[test]
fn test_upgrade_admin() {
    let setup = Setup::default();
    let token = setup.token;
    assert!(token
        .try_upgrade(&setup.admin, &install_dummy_wasm(&setup.env))
        .is_ok());
}
