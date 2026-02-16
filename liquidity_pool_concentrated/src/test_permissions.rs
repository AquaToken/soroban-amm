#![cfg(test)]

use crate::testutils::Setup;
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Symbol, Vec};
use utils::test_utils::{install_dummy_wasm, jump};

// ═══════════════════════════════════════════════════════════════════════════
// Admin transfer ownership (3-day delay)
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_admin_transfer_ownership_too_early() {
    let setup = Setup::default();
    let admin_new = Address::generate(&setup.env);
    setup
        .pool
        .commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &admin_new);
    jump(&setup.env, ADMIN_ACTIONS_DELAY - 1);
    setup
        .pool
        .apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2906)")]
fn test_admin_transfer_ownership_twice() {
    let setup = Setup::default();
    let admin_new = Address::generate(&setup.env);
    setup
        .pool
        .commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &admin_new);
    setup
        .pool
        .commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &admin_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_admin_transfer_ownership_not_committed() {
    let setup = Setup::default();
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    setup
        .pool
        .apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_admin_transfer_ownership_reverted() {
    let setup = Setup::default();
    let admin_new = Address::generate(&setup.env);
    setup
        .pool
        .commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &admin_new);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    setup
        .pool
        .revert_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
    setup
        .pool
        .apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"));
}

#[test]
fn test_admin_transfer_ownership() {
    let setup = Setup::default();
    let admin_new = Address::generate(&setup.env);
    setup
        .pool
        .commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &admin_new);
    assert!(setup
        .pool
        .try_revert_transfer_ownership(&admin_new, &symbol_short!("Admin"))
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    setup
        .pool
        .apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"));

    // New admin can call protected methods
    setup
        .pool
        .commit_transfer_ownership(&admin_new, &symbol_short!("Admin"), &admin_new);
}

// ═══════════════════════════════════════════════════════════════════════════
// Emergency admin transfer ownership
// ═══════════════════════════════════════════════════════════════════════════

#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_emergency_admin_transfer_ownership_too_early() {
    let setup = Setup::default();
    let new_addr = Address::generate(&setup.env);
    setup.pool.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &new_addr,
    );
    jump(&setup.env, ADMIN_ACTIONS_DELAY - 1);
    setup.pool.apply_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
    );
}

#[test]
fn test_emergency_admin_transfer_ownership() {
    let setup = Setup::default();
    let new_addr = Address::generate(&setup.env);
    setup.pool.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &new_addr,
    );
    // Old emergency admin can still act
    assert!(setup
        .pool
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_ok());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    setup.pool.apply_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
    );
    // New emergency admin works, old one doesn't
    assert!(setup
        .pool
        .try_set_emergency_mode(&new_addr, &false)
        .is_ok());
    assert!(setup
        .pool
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// Upgrade management
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_commit_upgrade() {
    let setup = Setup::default();
    let wasm = install_dummy_wasm(&setup.env);
    let token_wasm = install_dummy_wasm(&setup.env);
    let gauge_wasm = install_dummy_wasm(&setup.env);
    let user = Address::generate(&setup.env);

    // Only admin can commit upgrades
    for (addr, is_ok) in [
        (user, false),
        (setup.emergency_admin.clone(), false),
        (setup.rewards_admin.clone(), false),
        (setup.operations_admin.clone(), false),
        (setup.pause_admin.clone(), false),
        (setup.admin.clone(), true),
    ] {
        assert_eq!(
            setup
                .pool
                .try_commit_upgrade(&addr, &wasm, &token_wasm, &gauge_wasm)
                .is_ok(),
            is_ok,
        );
    }
}

#[test]
fn test_apply_upgrade_admin() {
    let setup = Setup::default();
    let wasm = install_dummy_wasm(&setup.env);
    let token_wasm = install_dummy_wasm(&setup.env);
    let gauge_wasm = install_dummy_wasm(&setup.env);

    setup
        .pool
        .commit_upgrade(&setup.admin, &wasm, &token_wasm, &gauge_wasm);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    setup.pool.apply_upgrade(&setup.admin);
}

#[test]
fn test_apply_upgrade_third_party_fails() {
    let setup = Setup::default();
    let wasm = install_dummy_wasm(&setup.env);
    let token_wasm = install_dummy_wasm(&setup.env);
    let gauge_wasm = install_dummy_wasm(&setup.env);
    let user = Address::generate(&setup.env);

    setup
        .pool
        .commit_upgrade(&setup.admin, &wasm, &token_wasm, &gauge_wasm);
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert!(setup.pool.try_apply_upgrade(&user).is_err());
}

#[test]
fn test_emergency_upgrade() {
    let setup = Setup::default();
    let wasm = install_dummy_wasm(&setup.env);
    let token_wasm = install_dummy_wasm(&setup.env);
    let gauge_wasm = install_dummy_wasm(&setup.env);

    setup
        .pool
        .set_emergency_mode(&setup.emergency_admin, &true);
    setup
        .pool
        .commit_upgrade(&setup.admin, &wasm, &token_wasm, &gauge_wasm);
    // Can apply immediately — no delay in emergency mode
    setup.pool.apply_upgrade(&setup.admin);
}

// ═══════════════════════════════════════════════════════════════════════════
// Emergency mode
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_set_emergency_mode_emergency_admin() {
    let setup = Setup::default();
    assert!(setup
        .pool
        .try_set_emergency_mode(&setup.emergency_admin, &true)
        .is_ok());
}

#[test]
fn test_set_emergency_mode_admin_fails() {
    let setup = Setup::default();
    assert!(setup
        .pool
        .try_set_emergency_mode(&setup.admin, &true)
        .is_err());
}

#[test]
fn test_set_emergency_mode_third_party_fails() {
    let setup = Setup::default();
    let user = Address::generate(&setup.env);
    assert!(setup
        .pool
        .try_set_emergency_mode(&user, &true)
        .is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// Kill switch permissions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_kill_deposit_permissions() {
    let setup = Setup::default();
    let user = Address::generate(&setup.env);

    // Admin, pause_admin, and emergency_pause_admin can kill
    assert!(setup.pool.try_kill_deposit(&setup.admin).is_ok());
    setup.pool.unkill_deposit(&setup.admin);
    assert!(setup.pool.try_kill_deposit(&setup.pause_admin).is_ok());
    setup.pool.unkill_deposit(&setup.admin);
    assert!(setup
        .pool
        .try_kill_deposit(&setup.emergency_pause_admin)
        .is_ok());

    // Others cannot
    assert!(setup.pool.try_kill_deposit(&user).is_err());
    assert!(setup
        .pool
        .try_kill_deposit(&setup.rewards_admin)
        .is_err());
    assert!(setup
        .pool
        .try_kill_deposit(&setup.operations_admin)
        .is_err());
}

#[test]
fn test_unkill_deposit_permissions() {
    let setup = Setup::default();
    let user = Address::generate(&setup.env);

    setup.pool.kill_deposit(&setup.admin);

    // Only admin and pause_admin can unkill (NOT emergency_pause_admin)
    assert!(setup
        .pool
        .try_unkill_deposit(&setup.emergency_pause_admin)
        .is_err());
    assert!(setup.pool.try_unkill_deposit(&user).is_err());
    assert!(setup.pool.try_unkill_deposit(&setup.pause_admin).is_ok());
    setup.pool.kill_deposit(&setup.admin);
    assert!(setup.pool.try_unkill_deposit(&setup.admin).is_ok());
}

#[test]
fn test_kill_swap_permissions() {
    let setup = Setup::default();
    let user = Address::generate(&setup.env);

    assert!(setup.pool.try_kill_swap(&setup.admin).is_ok());
    setup.pool.unkill_swap(&setup.admin);
    assert!(setup.pool.try_kill_swap(&setup.pause_admin).is_ok());
    setup.pool.unkill_swap(&setup.admin);
    assert!(setup
        .pool
        .try_kill_swap(&setup.emergency_pause_admin)
        .is_ok());

    assert!(setup.pool.try_kill_swap(&user).is_err());
    assert!(setup.pool.try_kill_swap(&setup.rewards_admin).is_err());
}

#[test]
fn test_unkill_swap_permissions() {
    let setup = Setup::default();

    setup.pool.kill_swap(&setup.admin);
    assert!(setup
        .pool
        .try_unkill_swap(&setup.emergency_pause_admin)
        .is_err());
    assert!(setup.pool.try_unkill_swap(&setup.pause_admin).is_ok());
    setup.pool.kill_swap(&setup.admin);
    assert!(setup.pool.try_unkill_swap(&setup.admin).is_ok());
}

#[test]
fn test_kill_claim_permissions() {
    let setup = Setup::default();
    let user = Address::generate(&setup.env);

    assert!(setup
        .pool
        .try_set_claim_killed(&setup.admin, &true)
        .is_ok());
    setup.pool.set_claim_killed(&setup.admin, &false);
    assert!(setup
        .pool
        .try_set_claim_killed(&setup.pause_admin, &true)
        .is_ok());
    setup.pool.set_claim_killed(&setup.admin, &false);
    assert!(setup
        .pool
        .try_set_claim_killed(&setup.emergency_pause_admin, &true)
        .is_ok());

    assert!(setup
        .pool
        .try_set_claim_killed(&user, &true)
        .is_err());
    assert!(setup
        .pool
        .try_set_claim_killed(&setup.rewards_admin, &true)
        .is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// Privileged address management
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_set_privileged_addrs() {
    let setup = Setup::default();
    let user = Address::generate(&setup.env);
    let new_addr = Address::generate(&setup.env);

    // Only admin can set privileged addresses
    assert!(setup
        .pool
        .try_set_privileged_addrs(
            &user,
            &new_addr,
            &new_addr,
            &new_addr,
            &Vec::from_array(&setup.env, [new_addr.clone()]),
            &new_addr,
        )
        .is_err());

    assert!(setup
        .pool
        .try_set_privileged_addrs(
            &setup.admin,
            &new_addr,
            &new_addr,
            &new_addr,
            &Vec::from_array(&setup.env, [new_addr.clone()]),
            &new_addr,
        )
        .is_ok());
}

// ═══════════════════════════════════════════════════════════════════════════
// Protocol fee permissions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_set_protocol_fee_fraction_permissions() {
    let setup = Setup::default();
    let user = Address::generate(&setup.env);

    // Operations admin or admin can set protocol fee
    assert!(setup
        .pool
        .try_set_protocol_fee_fraction(&setup.operations_admin, &3_000)
        .is_ok());
    assert!(setup
        .pool
        .try_set_protocol_fee_fraction(&setup.admin, &4_000)
        .is_ok());

    // Others cannot
    assert!(setup
        .pool
        .try_set_protocol_fee_fraction(&user, &5_000)
        .is_err());
    assert!(setup
        .pool
        .try_set_protocol_fee_fraction(&setup.rewards_admin, &5_000)
        .is_err());
    assert!(setup
        .pool
        .try_set_protocol_fee_fraction(&setup.pause_admin, &5_000)
        .is_err());
}

#[test]
fn test_claim_protocol_fees_permissions() {
    let setup = Setup::default();
    let user = Address::generate(&setup.env);
    let dest = Address::generate(&setup.env);

    // System fee admin or admin can claim protocol fees
    assert!(setup
        .pool
        .try_claim_protocol_fees(&setup.system_fee_admin, &dest)
        .is_ok());
    assert!(setup
        .pool
        .try_claim_protocol_fees(&setup.admin, &dest)
        .is_ok());

    // Others cannot
    assert!(setup
        .pool
        .try_claim_protocol_fees(&user, &dest)
        .is_err());
    assert!(setup
        .pool
        .try_claim_protocol_fees(&setup.operations_admin, &dest)
        .is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// Distance weighting permissions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_set_distance_weighting_permissions() {
    let setup = Setup::default();
    let user = Address::generate(&setup.env);

    // Operations admin or admin can set
    assert!(setup
        .pool
        .try_set_distance_weighting(&setup.operations_admin, &10_000, &1_000)
        .is_ok());
    assert!(setup
        .pool
        .try_set_distance_weighting(&setup.admin, &10_000, &1_000)
        .is_ok());

    // Others cannot
    assert!(setup
        .pool
        .try_set_distance_weighting(&user, &10_000, &1_000)
        .is_err());
    assert!(setup
        .pool
        .try_set_distance_weighting(&setup.pause_admin, &10_000, &1_000)
        .is_err());
}

// ═══════════════════════════════════════════════════════════════════════════
// Initialize price permissions
// ═══════════════════════════════════════════════════════════════════════════

#[test]
fn test_initialize_price_permissions() {
    let setup = Setup::default();
    let user = Address::generate(&setup.env);
    let price = soroban_sdk::U256::from_u128(&setup.env, 1u128 << 96);

    // Operations admin or admin can set price
    assert!(setup
        .pool
        .try_initialize_price(&setup.operations_admin, &price)
        .is_ok());
    assert!(setup
        .pool
        .try_initialize_price(&setup.admin, &price)
        .is_ok());

    // Others cannot
    assert!(setup
        .pool
        .try_initialize_price(&user, &price)
        .is_err());
    assert!(setup
        .pool
        .try_initialize_price(&setup.rewards_admin, &price)
        .is_err());
}
