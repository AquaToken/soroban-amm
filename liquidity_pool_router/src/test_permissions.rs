#![cfg(test)]

use crate::testutils::{
    install_liq_pool_hash, install_stableswap_liq_pool_hash, install_token_wasm, Setup,
};
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Vec};
use utils::test_utils::jump;

// test transfer ownership
#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
fn test_transfer_ownership_too_early() {
    let setup = Setup::default();
    let router = setup.router;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(&admin_original, &admin_new);
    // check admin by calling protected method
    router.set_pools_plane(&admin_original, &router.get_plane());
    jump(&setup.env, ADMIN_ACTIONS_DELAY - 1);
    router.apply_transfer_ownership(&admin_original);
}

#[test]
#[should_panic(expected = "Error(Contract, #2906)")]
fn test_transfer_ownership_twice() {
    let setup = Setup::default();
    let router = setup.router;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(&admin_original, &admin_new);
    router.commit_transfer_ownership(&admin_original, &admin_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_transfer_ownership_not_committed() {
    let setup = Setup::default();
    let router = setup.router;
    let admin_original = setup.admin;

    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    router.apply_transfer_ownership(&admin_original);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
fn test_transfer_ownership_reverted() {
    let setup = Setup::default();
    let router = setup.router;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(&admin_original, &admin_new);
    // check admin by calling protected method
    router.set_pools_plane(&admin_original, &router.get_plane());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    router.revert_transfer_ownership(&admin_original);
    router.apply_transfer_ownership(&admin_original);
}

#[test]
fn test_transfer_ownership() {
    let setup = Setup::default();
    let router = setup.router;
    let admin_original = setup.admin;
    let admin_new = Address::generate(&setup.env);

    router.commit_transfer_ownership(&admin_original, &admin_new);
    // check admin by calling protected method
    router.set_pools_plane(&admin_original, &router.get_plane());
    jump(&setup.env, ADMIN_ACTIONS_DELAY + 1);
    router.apply_transfer_ownership(&admin_original);
    router.set_pools_plane(&admin_new, &router.get_plane());
}

// test all the authorized methods
#[test]
fn test_set_pools_plane() {
    let setup = Setup::default();
    let router = setup.router;
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
fn test_upgrade_third_party_user() {
    let setup = Setup::default();
    let router = setup.router;
    let user = Address::generate(&setup.env);
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    let token_hash = install_token_wasm(&setup.env);

    assert!(router.try_upgrade(&user, &token_hash).is_err());
}

#[test]
fn test_upgrade_admin() {
    let setup = Setup::default();
    let router = setup.router;
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    let token_hash = install_token_wasm(&setup.env);

    assert!(router.try_upgrade(&setup.admin, &token_hash).is_ok());
}

#[test]
fn test_upgrade_rewards_admin() {
    let setup = Setup::default();
    let router = setup.router;
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    let token_hash = install_token_wasm(&setup.env);

    assert!(router
        .try_upgrade(&setup.rewards_admin, &token_hash)
        .is_err());
}

#[test]
fn test_upgrade_operations_admin() {
    let setup = Setup::default();
    let router = setup.router;
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    let token_hash = install_token_wasm(&setup.env);

    assert!(router
        .try_upgrade(&setup.operations_admin, &token_hash)
        .is_err());
}

#[test]
fn test_upgrade_pause_admin() {
    let setup = Setup::default();
    let router = setup.router;
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    let token_hash = install_token_wasm(&setup.env);

    assert!(router.try_upgrade(&setup.pause_admin, &token_hash).is_err());
}

#[test]
fn test_upgrade_emergency_pause_admin() {
    let setup = Setup::default();
    let router = setup.router;
    // Upgrade router code with token wasm as it has no dependencies
    // after upgrade router cannot be reused
    let token_hash = install_token_wasm(&setup.env);

    assert!(router
        .try_upgrade(&setup.emergency_pause_admin, &token_hash)
        .is_err());
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
