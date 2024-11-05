#![cfg(test)]

use crate::testutils::{
    install_liq_pool_hash, install_stableswap_liq_pool_hash, install_token_wasm, Setup,
};
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{symbol_short, Address, Symbol, Vec};
use utils::test_utils::{install_dummy_wasm, jump};

// test admin transfer ownership
#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_admin_transfer_ownership_too_early() {
    let setup = Setup::default();
    let router = setup.router;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    // check admin not changed yet by calling protected method
    assert!(router
        .try_revert_transfer_ownership(&admin_new, &symbol_short!("Admin"))
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY - 1);
    router.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2906)")]
fn test_admin_transfer_ownership_twice() {
    let setup = Setup::default();
    let router = setup.router;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    router.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_admin_transfer_ownership_not_committed() {
    let setup = Setup::default();
    let router = setup.router;
    let admin_original = setup.admin;

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    router.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_admin_transfer_ownership_reverted() {
    let setup = Setup::default();
    let router = setup.router;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    // check admin not changed yet by calling protected method
    assert!(router
        .try_revert_transfer_ownership(&admin_new, &symbol_short!("Admin"))
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    router.revert_transfer_ownership(&admin_original, &symbol_short!("Admin"));
    router.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));
}

#[test]
fn test_admin_transfer_ownership() {
    let setup = Setup::default();
    let router = setup.router;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(&admin_original, &symbol_short!("Admin"), &admin_new);
    // check admin not changed yet by calling protected method
    assert!(router
        .try_revert_transfer_ownership(&admin_new, &symbol_short!("Admin"))
        .is_err());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    router.apply_transfer_ownership(&admin_original, &symbol_short!("Admin"));

    router.commit_transfer_ownership(&admin_new, &symbol_short!("Admin"), &admin_new);
}

// test emergency admin transfer ownership
#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_emergency_admin_transfer_ownership_too_early() {
    let setup = Setup::default();
    let router = setup.router;
    let emergency_admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );

    // check emergency admin not changed yet by calling protected method
    assert!(router
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_err());
    assert!(router
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_ok());

    jump(&setup.env, ADMIN_ACTIONS_DELAY - 1);
    router.apply_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2906)")]
fn test_emergency_admin_transfer_ownership_twice() {
    let setup = Setup::default();
    let router = setup.router;
    let emergency_admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );
    router.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_emergency_admin_transfer_ownership_not_committed() {
    let setup = Setup::default();
    let router = setup.router;

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    router.apply_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_emergency_admin_transfer_ownership_reverted() {
    let setup = Setup::default();
    let router = setup.router;
    let emergency_admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );

    // check emergency admin not changed yet by calling protected method
    assert!(router
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_err());
    assert!(router
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_ok());

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    router.revert_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));
    router.apply_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));
}

#[test]
fn test_emergency_admin_transfer_ownership() {
    let setup = Setup::default();
    let router = setup.router;
    let emergency_admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );

    // check emergency admin not changed yet by calling protected method
    assert!(router
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_err());
    assert!(router
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_ok());

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    router.apply_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));

    // check emergency admin has changed
    assert!(router
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_ok());
    assert!(router
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_err());
}

#[test]
fn test_transfer_ownership_separate_deadlines() {
    let setup = Setup::default();
    let router = setup.router;
    let admin_new = Address::generate(&setup.env);
    let emergency_admin_new = Address::generate(&setup.env);

    assert_eq!(
        router.get_future_address(&Symbol::new(&setup.env, "EmergencyAdmin")),
        setup.emergency_admin
    );
    assert_eq!(
        router.get_future_address(&symbol_short!("Admin")),
        setup.admin
    );

    assert!(router
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_err());
    assert!(router
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_ok());

    router.commit_transfer_ownership(
        &setup.admin,
        &Symbol::new(&setup.env, "EmergencyAdmin"),
        &emergency_admin_new,
    );
    jump(&setup.env, 10);
    router.commit_transfer_ownership(&setup.admin, &symbol_short!("Admin"), &admin_new);

    assert_eq!(
        router.get_future_address(&Symbol::new(&setup.env, "EmergencyAdmin")),
        emergency_admin_new
    );
    assert_eq!(
        router.get_future_address(&symbol_short!("Admin")),
        admin_new
    );

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1 - 10);
    router.apply_transfer_ownership(&setup.admin, &Symbol::new(&setup.env, "EmergencyAdmin"));
    assert!(router
        .try_apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"))
        .is_err());

    assert_eq!(
        router.get_future_address(&Symbol::new(&setup.env, "EmergencyAdmin")),
        emergency_admin_new
    );

    jump(&setup.env, 10);
    router.apply_transfer_ownership(&setup.admin, &symbol_short!("Admin"));

    assert_eq!(
        router.get_future_address(&symbol_short!("Admin")),
        admin_new
    );

    // check ownership transfer is complete. new admin is capable to call protected methods
    //      and new emergency admin can change toggle emergency mode
    router.commit_transfer_ownership(&admin_new, &Symbol::new(&setup.env, "Admin"), &setup.admin);
    assert!(router
        .try_set_emergency_mode(&emergency_admin_new, &false)
        .is_ok());
    assert!(router
        .try_set_emergency_mode(&setup.emergency_admin, &false)
        .is_err());
}

// test all the authorized methods
#[test]
fn test_set_pools_router() {
    let setup = Setup::default();
    let router = setup.router;
    let plane = Address::generate(&setup.env);
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
        assert_eq!(router.try_set_pools_plane(&addr, &plane).is_ok(), is_ok);
    }
}

#[test]
fn test_set_liquidity_calculator() {
    let setup = Setup::default();
    let router = setup.router;
    let liq_calculator = Address::generate(&setup.env);
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
            router
                .try_set_liquidity_calculator(&addr, &liq_calculator)
                .is_ok(),
            is_ok
        );
    }
}

#[test]
fn test_set_privileged_addresses() {
    let setup = Setup::default();
    let router = setup.router;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user, false),
        (setup.admin.clone(), true),
        (setup.emergency_admin, false),
        (setup.rewards_admin.clone(), false),
        (setup.operations_admin.clone(), false),
        (setup.pause_admin.clone(), false),
        (setup.emergency_pause_admin.clone(), false),
    ] {
        assert_eq!(
            router
                .try_set_privileged_addrs(
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
fn test_set_hashes() {
    let setup = Setup::default();
    let router = setup.router;
    let user = Address::generate(&setup.env);
    let pool_hash = install_liq_pool_hash(&setup.env);
    let stable_pool_hash = install_stableswap_liq_pool_hash(&setup.env);
    let token_hash = install_token_wasm(&setup.env);

    for (addr, is_ok) in [
        (user, false),
        (setup.admin, true),
        (setup.emergency_admin, false),
        (setup.rewards_admin, false),
        (setup.operations_admin, false),
        (setup.pause_admin, false),
        (setup.emergency_pause_admin, false),
    ] {
        assert_eq!(router.try_set_token_hash(&addr, &token_hash).is_ok(), is_ok);
        assert_eq!(router.try_set_pool_hash(&addr, &pool_hash).is_ok(), is_ok);
        assert_eq!(
            router
                .try_set_stableswap_pool_hash(&addr, &stable_pool_hash)
                .is_ok(),
            is_ok
        );
    }
}

#[test]
fn test_set_reward_token() {
    let setup = Setup::default();
    let router = setup.router;
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
            router
                .try_set_reward_token(&addr, &setup.reward_token.address)
                .is_ok(),
            is_ok
        );
    }
}

#[test]
fn test_commit_upgrade() {
    let setup = Setup::default();
    let router = setup.router;
    let new_wasm = install_dummy_wasm(&setup.env);
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
        assert_eq!(router.try_commit_upgrade(&addr, &new_wasm).is_ok(), is_ok);
    }
}

// apply upgrade
#[test]
fn test_apply_upgrade_third_party_user() {
    let setup = Setup::default();
    let router = setup.router;
    let user = Address::generate(&setup.env);
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    router.commit_upgrade(&setup.admin, &install_dummy_wasm(&setup.env));
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert!(router.try_apply_upgrade(&user).is_err());
}

#[test]
fn test_apply_upgrade_emergency_admin() {
    let setup = Setup::default();
    let router = setup.router;
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    router.commit_upgrade(&setup.admin, &install_dummy_wasm(&setup.env));
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert!(router.try_apply_upgrade(&setup.emergency_admin).is_err());
}

#[test]
fn test_apply_upgrade_admin() {
    let setup = Setup::default();
    let router = setup.router;
    assert_ne!(router.version(), 130);
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    router.commit_upgrade(&setup.admin, &install_dummy_wasm(&setup.env));
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert!(router.try_apply_upgrade(&setup.admin).is_ok());
    assert_eq!(router.version(), 130);
}

#[test]
fn test_apply_upgrade_rewards_admin() {
    let setup = Setup::default();
    let router = setup.router;
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    router.commit_upgrade(&setup.admin, &install_dummy_wasm(&setup.env));
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert!(router.try_apply_upgrade(&setup.rewards_admin).is_err());
}

#[test]
fn test_apply_upgrade_operations_admin() {
    let setup = Setup::default();
    let router = setup.router;
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    router.commit_upgrade(&setup.admin, &install_dummy_wasm(&setup.env));
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert!(router.try_apply_upgrade(&setup.operations_admin).is_err());
}

#[test]
fn test_apply_upgrade_pause_admin() {
    let setup = Setup::default();
    let router = setup.router;
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    router.commit_upgrade(&setup.admin, &install_dummy_wasm(&setup.env));
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert!(router.try_apply_upgrade(&setup.pause_admin).is_err());
}

#[test]
fn test_apply_upgrade_emergency_pause_admin() {
    let setup = Setup::default();
    let router = setup.router;
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    router.commit_upgrade(&setup.admin, &install_dummy_wasm(&setup.env));
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    assert!(router
        .try_apply_upgrade(&setup.emergency_pause_admin)
        .is_err());
}

#[test]
fn test_set_emergency_mode() {
    let setup = Setup::default();
    let router = setup.router;
    let user = Address::generate(&setup.env);

    for (addr, is_ok) in [
        (user, false),
        (setup.admin, false),
        (setup.emergency_admin, true),
        (setup.rewards_admin, false),
        (setup.operations_admin, false),
        (setup.pause_admin, false),
        (setup.emergency_pause_admin, false),
    ] {
        assert_eq!(router.try_set_emergency_mode(&addr, &false).is_ok(), is_ok);
    }
}

// reward admin
#[test]
fn test_config_rewards() {
    let setup = Setup::default();
    let router = setup.router;
    let user = Address::generate(&setup.env);
    let [token1, token2, _, _] = setup.tokens;
    let tokens = Vec::from_array(&setup.env, [token1.address.clone(), token2.address.clone()]);

    for (addr, is_ok) in [
        (user, false),
        (setup.admin, true),
        (setup.emergency_admin, false),
        (setup.rewards_admin, true),
        (setup.operations_admin, false),
        (setup.pause_admin, false),
        (setup.emergency_pause_admin, false),
    ] {
        assert_eq!(
            router
                .try_config_global_rewards(
                    &addr,
                    &1,
                    &setup.env.ledger().timestamp().saturating_add(60),
                    &Vec::from_array(&setup.env, [(tokens.clone(), 1_0000000)]),
                )
                .is_ok(),
            is_ok
        );
    }
}

#[test]
fn test_distribute_rewards() {
    let setup = Setup::default();
    let router = setup.router;
    let user = Address::generate(&setup.env);
    let [token1, token2, _, _] = setup.tokens;
    let tokens = Vec::from_array(&setup.env, [token1.address.clone(), token2.address.clone()]);
    setup.reward_token.mint(&user, &1_0000000);
    setup.reward_token.mint(&router.address, &1_0000000);
    let (pool_hash, _pool_address) = router.init_standard_pool(&user, &tokens, &10);

    for (addr, is_ok) in [
        (user, false),
        (setup.admin.clone(), true),
        (setup.emergency_admin, false),
        (setup.rewards_admin, true),
        (setup.operations_admin, false),
        (setup.pause_admin, false),
        (setup.emergency_pause_admin, false),
    ] {
        router.config_global_rewards(
            &setup.admin,
            &1_0000000,
            &setup.env.ledger().timestamp().saturating_add(60),
            &Vec::from_array(&setup.env, [(tokens.clone(), 1_0000000)]),
        );
        router.fill_liquidity(&tokens);
        router.config_pool_rewards(&tokens, &pool_hash);

        assert_eq!(
            router
                .try_distribute_outstanding_reward(&addr, &router.address, &tokens, &pool_hash)
                .is_ok(),
            is_ok
        );
    }
}

#[test]
fn test_configure_init_pool_payment() {
    let setup = Setup::default();
    let router = setup.router;
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
            router
                .try_configure_init_pool_payment(
                    &addr,
                    &setup.reward_token.address,
                    &1,
                    &1,
                    &router.address
                )
                .is_ok(),
            is_ok
        );
    }
}

#[test]
fn test_remove_pool() {
    let setup = Setup::default();
    let router = setup.router;
    let user = Address::generate(&setup.env);
    let [token1, token2, _, _] = setup.tokens;
    let tokens = Vec::from_array(&setup.env, [token1.address.clone(), token2.address.clone()]);
    setup.reward_token.mint(&user, &10_0000000);

    for (addr, is_ok) in [
        (user.clone(), false),
        (setup.admin.clone(), true),
        (setup.emergency_admin, false),
        (setup.rewards_admin, false),
        (setup.operations_admin, true),
        (setup.pause_admin, false),
        (setup.emergency_pause_admin, false),
    ] {
        let (pool_hash, _pool_address) = router.init_standard_pool(&user.clone(), &tokens, &10);
        assert_eq!(
            router.try_remove_pool(&addr, &tokens, &pool_hash).is_ok(),
            is_ok
        );
    }
}
