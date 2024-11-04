#![cfg(test)]

use crate::pool_constants::MIN_RAMP_TIME;
use crate::testutils::Setup;
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Symbol, Vec};
use token_share::Client as ShareTokenClient;
use utils::test_utils::{install_dummy_wasm, jump};

// test admin transfer ownership
#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_admin_transfer_ownership_too_early() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    // check admin not changed yet by calling protected method
    assert!(pool
        .try_revert_transfer_ownership(&admin_new, &symbol_short!("Admin"))
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY - 1);
    pool.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2906)")]
fn test_admin_transfer_ownership_twice() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    pool.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_admin_transfer_ownership_not_committed() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let admin_original = setup.admin;

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    pool.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_admin_transfer_ownership_reverted() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    // check admin not changed yet by calling protected method
    assert!(pool
        .try_revert_transfer_ownership(&admin_new, &symbol_short!("Admin"))
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    pool.revert_transfer_ownership(&admin_original, &symbol_short!("Admin"));
    pool.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));
}

#[test]
fn test_admin_transfer_ownership() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    // check admin not changed yet by calling protected method
    assert!(pool
        .try_revert_transfer_ownership(&admin_new, &symbol_short!("Admin"))
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    pool.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));

    pool.commit_transfer_ownership(&admin_new, &symbol_short!("Admin"), &admin_new);
}

// test emergency admin transfer ownership
#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_emergency_admin_transfer_ownership_too_early() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let emergency_admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );

    // check emergency admin not changed yet by calling protected method
    assert!(pool
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_err());
    assert!(pool
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_ok());

    jump(&setup.env, ADMIN_ACTIONS_DELAY - 1);
    pool.apply_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2906)")]
fn test_emergency_admin_transfer_ownership_twice() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let emergency_admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );
    pool.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_emergency_admin_transfer_ownership_not_committed() {
    let setup = Setup::default();
    let pool = setup.liq_pool;

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    pool.apply_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_emergency_admin_transfer_ownership_reverted() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let emergency_admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );

    // check emergency admin not changed yet by calling protected method
    assert!(pool
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_err());
    assert!(pool
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_ok());

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    pool.revert_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));
    pool.apply_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));
}

#[test]
fn test_emergency_admin_transfer_ownership() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let emergency_admin_new = Address::generate(&setup.env);

    pool.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );

    // check emergency admin not changed yet by calling protected method
    assert!(pool
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_err());
    assert!(pool
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_ok());

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    pool.apply_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));

    // check emergency admin has changed
    assert!(pool
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_ok());
    assert!(pool
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_err());
}

#[test]
fn test_transfer_ownership_separate_deadlines() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let admin_new = Address::generate(&setup.env);
    let emergency_admin_new = Address::generate(&setup.env);

    assert_eq!(
        pool.get_future_address(&Symbol::new(&setup.env, "EmergencyAdmin")),
        setup.emergency_admin
    );
    assert_eq!(
        pool.get_future_address(&symbol_short!("Admin")),
        setup.admin
    );

    assert!(pool
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_err());
    assert!(pool
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_ok());

    pool.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );
    jump(&setup.env, 10);
    pool.commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &admin_new);

    assert_eq!(
        pool.get_future_address(&Symbol::new(&setup.env, "EmergencyAdmin")),
        emergency_admin_new
    );
    assert_eq!(pool.get_future_address(&symbol_short!("Admin")), admin_new);

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1 - 10);
    pool.apply_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));
    assert!(pool
        .try_apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"))
        .is_err());

    assert_eq!(
        pool.get_future_address(&Symbol::new(&setup.env, "EmergencyAdmin")),
        emergency_admin_new
    );

    jump(&setup.env, 10);
    pool.apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"));

    assert_eq!(pool.get_future_address(&symbol_short!("Admin")), admin_new);

    // check ownership transfer is complete. new admin is capable to call protected methods
    //      and new emergency admin can change toggle emergency mode
    pool.commit_transfer_ownership(&admin_new, &Symbol::new(&setup.env, "Admin"), &setup.admin);
    assert!(pool
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_ok());
    assert!(pool
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_err());
}

// upgrade pool & token
#[test]
fn test_commit_upgrade() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let new_wasm = install_dummy_wasm(&setup.env);
    let new_token_wasm = install_dummy_wasm(&setup.env);
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user, false),
        (setup.admin, true),
        (setup.emergency_admin, false),
        (setup.rewards_admin, false),
        (setup.operations_admin, false),
        (setup.pause_admin, false),
        (setup.emergency_pause_admin, false),
    ] {
        assert_eq!(
            pool.try_commit_upgrade(&addr, &new_wasm, &new_token_wasm)
                .is_ok(),
            is_ok
        );
    }
}

#[test]
fn test_apply_upgrade_third_party_user() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let user = Address::generate(&setup.env);
    pool.commit_upgrade(
        &setup.admin,
        &install_dummy_wasm(&setup.env),
        &install_dummy_wasm(&setup.env),
    );
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert!(pool.try_apply_upgrade(&user).is_err());
}

#[test]
fn test_apply_upgrade_emergency_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    pool.commit_upgrade(
        &setup.admin,
        &install_dummy_wasm(&setup.env),
        &install_dummy_wasm(&setup.env),
    );
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert!(pool.try_apply_upgrade(&setup.emergency_admin).is_err());
}

#[test]
fn test_apply_upgrade_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let token = ShareTokenClient::new(&setup.env, &pool.share_id());
    let new_wasm = install_dummy_wasm(&setup.env);
    let new_token_wasm = install_dummy_wasm(&setup.env);

    assert_ne!(pool.version(), 130);
    assert_ne!(token.version(), 130);

    pool.commit_upgrade(&setup.admin, &new_wasm, &new_token_wasm);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert_eq!(pool.apply_upgrade(&setup.admin), (new_wasm, new_token_wasm));

    // check contracts updated, dummy contract version is 130
    assert_eq!(pool.version(), 130);
    assert_eq!(token.version(), 130);
}

// emergency mode
#[test]
fn test_set_emergency_mode_third_party_user() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let user = Address::generate(&setup.env);
    assert!(pool.try_set_emergency_mode(&user, &false).is_err());
}

#[test]
fn test_set_emergency_mode_emergency_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool.try_set_emergency_mode(&setup.admin, &false).is_err());
}

#[test]
fn test_set_emergency_mode_admin() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    assert!(pool
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_ok());
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

#[test]
fn test_ramp_a() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user, false),
        (setup.admin, true),
        (setup.rewards_admin, false),
        (setup.operations_admin, true),
        (setup.pause_admin, false),
        (setup.emergency_pause_admin, false),
    ] {
        assert_eq!(
            pool.try_ramp_a(
                &addr,
                &(pool.a() * 2),
                &setup.env.ledger().timestamp().saturating_add(MIN_RAMP_TIME)
            )
            .is_ok(),
            is_ok
        );
        jump(&setup.env, MIN_RAMP_TIME);
        assert_eq!(pool.try_stop_ramp_a(&addr).is_ok(), is_ok);
    }
}

#[test]
fn test_update_fee() {
    let setup = Setup::default();
    let pool = setup.liq_pool;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user, false),
        (setup.admin, true),
        (setup.rewards_admin, false),
        (setup.operations_admin, true),
        (setup.pause_admin, false),
        (setup.emergency_pause_admin, false),
    ] {
        assert_eq!(pool.try_revert_new_parameters(&addr).is_ok(), is_ok);
        assert_eq!(pool.try_commit_new_fee(&addr, &1).is_ok(), is_ok);
        jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
        assert_eq!(pool.try_apply_new_fee(&addr).is_ok(), is_ok);
    }
}
