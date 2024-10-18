#![cfg(test)]

use crate::testutils::Setup;
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Vec};
use utils::test_utils::{install_dummy_wasm, jump};

// test transfer ownership
#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_transfer_ownership_too_early() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let admin_original = setup.users[0].clone();
    let admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(&admin_original, &admin_new);
    // check admin not changed yet by calling protected method
    assert!(pool.try_revert_transfer_ownership(&admin_new).is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY - 1);
    pool.apply_transfer_ownership(&admin_original);
}

#[test]
#[should_panic(expected = "Error(Contract, #2906)")]
fn test_transfer_ownership_twice() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let admin_original = setup.users[0].clone();
    let admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(&admin_original, &admin_new);
    pool.commit_transfer_ownership(&admin_original, &admin_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_transfer_ownership_not_committed() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let admin_original = setup.users[0].clone();

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    pool.apply_transfer_ownership(&admin_original);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_transfer_ownership_reverted() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let admin_original = setup.users[0].clone();
    let admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(&admin_original, &admin_new);
    // check admin not changed yet by calling protected method
    assert!(pool.try_revert_transfer_ownership(&admin_new).is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    pool.revert_transfer_ownership(&admin_original);
    pool.apply_transfer_ownership(&admin_original);
}

#[test]
fn test_transfer_ownership() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let admin_original = setup.users[0].clone();
    let admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(&admin_original, &admin_new);
    // check admin not changed yet by calling protected method
    assert!(pool.try_revert_transfer_ownership(&admin_new).is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    pool.apply_transfer_ownership(&admin_original);

    pool.commit_transfer_ownership(&admin_new, &admin_new);
}

// upgrade
#[test]
fn test_upgrade_third_party_user() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let user = Address::generate(&setup.env);
    assert!(pool
        .try_upgrade(&user, &install_dummy_wasm(&setup.env))
        .is_err());
}

#[test]
fn test_upgrade_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool
        .try_upgrade(&setup.admin, &install_dummy_wasm(&setup.env))
        .is_ok());
}

#[test]
fn test_upgrade_rewards_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool
        .try_upgrade(&setup.rewards_admin, &install_dummy_wasm(&setup.env))
        .is_err());
}

#[test]
fn test_upgrade_operations_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool
        .try_upgrade(&setup.operations_admin, &install_dummy_wasm(&setup.env))
        .is_err());
}

#[test]
fn test_upgrade_pause_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool
        .try_upgrade(&setup.pause_admin, &install_dummy_wasm(&setup.env))
        .is_err());
}

#[test]
fn test_upgrade_emergency_pause_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool
        .try_upgrade(
            &setup.emergency_pause_admin,
            &install_dummy_wasm(&setup.env)
        )
        .is_err());
}

// upgrade token
#[test]
fn test_upgrade_token_third_party_user() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let user = Address::generate(&setup.env);
    assert!(pool
        .try_upgrade_token(&user, &install_dummy_wasm(&setup.env))
        .is_err());
}

#[test]
fn test_upgrade_token_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool
        .try_upgrade_token(&setup.admin, &install_dummy_wasm(&setup.env))
        .is_ok());
}

#[test]
fn test_upgrade_token_rewards_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool
        .try_upgrade_token(&setup.rewards_admin, &install_dummy_wasm(&setup.env))
        .is_err());
}

#[test]
fn test_upgrade_token_operations_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool
        .try_upgrade_token(&setup.operations_admin, &install_dummy_wasm(&setup.env))
        .is_err());
}

#[test]
fn test_upgrade_token_pause_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool
        .try_upgrade_token(&setup.pause_admin, &install_dummy_wasm(&setup.env))
        .is_err());
}

#[test]
fn test_upgrade_token_emergency_pause_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool
        .try_upgrade_token(
            &setup.emergency_pause_admin,
            &install_dummy_wasm(&setup.env)
        )
        .is_err());
}

// kill switches
#[test]
fn test_kill_deposit() {
    let setup = Setup::default();
    let liq_pool = setup.liq_pool;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user.clone(), false),
        (setup.admin.clone(), true),
        (setup.rewards_admin, false),
        (setup.operations_admin, false),
        (setup.pause_admin, true),
        (setup.emergency_pause_admin, true),
    ] {
        assert_eq!(liq_pool.try_kill_deposit(&addr).is_ok(), is_ok);
    }
}

#[test]
fn test_kill_swap() {
    let setup = Setup::default();
    let liq_pool = setup.liq_pool;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user.clone(), false),
        (setup.admin.clone(), true),
        (setup.rewards_admin, false),
        (setup.operations_admin, false),
        (setup.pause_admin, true),
        (setup.emergency_pause_admin, true),
    ] {
        assert_eq!(liq_pool.try_kill_swap(&addr).is_ok(), is_ok);
    }
}

#[test]
fn test_kill_claim() {
    let setup = Setup::default();
    let liq_pool = setup.liq_pool;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user.clone(), false),
        (setup.admin.clone(), true),
        (setup.rewards_admin, false),
        (setup.operations_admin, false),
        (setup.pause_admin, true),
        (setup.emergency_pause_admin, true),
    ] {
        assert_eq!(liq_pool.try_kill_claim(&addr).is_ok(), is_ok);
    }
}
#[test]
fn test_unkill_deposit() {
    let setup = Setup::default();
    let liq_pool = setup.liq_pool;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user.clone(), false),
        (setup.admin.clone(), true),
        (setup.rewards_admin, false),
        (setup.operations_admin, false),
        (setup.pause_admin, true),
        (setup.emergency_pause_admin, false),
    ] {
        assert_eq!(liq_pool.try_unkill_deposit(&addr).is_ok(), is_ok);
    }
}

#[test]
fn test_unkill_swap() {
    let setup = Setup::default();
    let liq_pool = setup.liq_pool;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user.clone(), false),
        (setup.admin.clone(), true),
        (setup.rewards_admin, false),
        (setup.operations_admin, false),
        (setup.pause_admin, true),
        (setup.emergency_pause_admin, false),
    ] {
        assert_eq!(liq_pool.try_unkill_swap(&addr).is_ok(), is_ok);
    }
}

#[test]
fn test_unkill_claim() {
    let setup = Setup::default();
    let liq_pool = setup.liq_pool;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user.clone(), false),
        (setup.admin.clone(), true),
        (setup.rewards_admin, false),
        (setup.operations_admin, false),
        (setup.pause_admin, true),
        (setup.emergency_pause_admin, false),
    ] {
        assert_eq!(liq_pool.try_unkill_claim(&addr).is_ok(), is_ok);
    }
}

// manage privileged addresses
#[test]
fn test_set_privileged_addresses() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user, false),
        (setup.admin.clone(), true),
        (setup.rewards_admin.clone(), false),
        (setup.operations_admin.clone(), false),
        (setup.pause_admin.clone(), false),
        (setup.emergency_pause_admin.clone(), false),
    ] {
        assert_eq!(
            pool.try_set_privileged_addrs(
                &addr,
                &setup.rewards_admin.clone(),
                &setup.operations_admin.clone(),
                &setup.pause_admin.clone(),
                &Vec::from_array(&setup.env, [setup.emergency_pause_admin.clone()]),
            )
            .is_ok(),
            is_ok
        );
    }
}

#[test]
fn test_set_pools_plane() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let plane = Address::generate(&setup.env);
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user, false),
        (setup.admin, true),
        (setup.rewards_admin, false),
        (setup.operations_admin, false),
        (setup.pause_admin, false),
        (setup.emergency_pause_admin, false),
    ] {
        assert_eq!(pool.try_set_pools_plane(&addr, &plane).is_ok(), is_ok);
    }
}

#[test]
fn test_set_rewards_config() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user, false),
        (setup.admin, true),
        (setup.rewards_admin, true),
        (setup.operations_admin, false),
        (setup.pause_admin, false),
        (setup.emergency_pause_admin, false),
    ] {
        assert_eq!(
            pool.try_set_rewards_config(
                &addr,
                &setup.env.ledger().timestamp().saturating_add(10),
                &1
            )
            .is_ok(),
            is_ok
        );
        jump(&setup.env, 10);
    }
}
