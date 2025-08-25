#![cfg(test)]
extern crate std;

use crate::testutils::Setup;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::Address;
use utils::test_utils::install_dummy_wasm;

#[test]
fn test_checkpoint_pool() {
    let setup = Setup::with_mocked_pool();
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &Address::generate(&setup.env), &0, &0);
}

#[test]
#[should_panic(expected = "#102")]
fn test_checkpoint_third_party_user() {
    let setup = Setup::with_mocked_pool();
    setup.contract.checkpoint_user(
        &Address::generate(&setup.env), // random address, not pool
        &Address::generate(&setup.env),
        &0,
        &0,
    );
}

#[test]
fn test_claim_pool() {
    let setup = Setup::with_mocked_pool();
    setup
        .contract
        .claim(&setup.pool_address, &Address::generate(&setup.env), &0, &0);
}

#[test]
#[should_panic(expected = "#102")]
fn test_claim_third_party_user() {
    let setup = Setup::with_mocked_pool();
    setup.contract.claim(
        &Address::generate(&setup.env), // random address, not pool
        &Address::generate(&setup.env),
        &0,
        &0,
    );
}

#[test]
fn test_get_user_reward_pool() {
    let setup = Setup::with_mocked_pool();
    setup
        .contract
        .get_user_reward(&setup.pool_address, &Address::generate(&setup.env), &0, &0);
}

#[test]
#[should_panic(expected = "#102")]
fn test_get_user_reward_third_party_user() {
    let setup = Setup::with_mocked_pool();
    setup.contract.get_user_reward(
        &Address::generate(&setup.env), // random address, not pool
        &Address::generate(&setup.env),
        &0,
        &0,
    );
}

#[test]
fn test_schedule_reward_pool() {
    let setup = Setup::with_mocked_pool();
    let distributor = Address::generate(&setup.env);
    StellarAssetClient::new(&setup.env, &setup.reward_token.address).mint(&distributor, &1000);
    setup
        .contract
        .schedule_rewards_config(&setup.pool_address, &distributor, &None, &1000, &1, &0);
}

#[test]
#[should_panic(expected = "#102")]
fn test_schedule_reward_not_pool() {
    let setup = Setup::with_mocked_pool();
    let distributor = Address::generate(&setup.env);
    StellarAssetClient::new(&setup.env, &setup.reward_token.address).mint(&distributor, &1000);
    setup.contract.schedule_rewards_config(
        &Address::generate(&setup.env),
        &distributor,
        &None,
        &1000,
        &1,
        &0,
    );
}

// upgrade
#[test]
fn test_upgrade_third_party_user() {
    let setup = Setup::default();
    let user = Address::generate(&setup.env);
    assert!(setup
        .contract
        .try_upgrade(&user, &install_dummy_wasm(&setup.env))
        .is_err());
}

#[test]
fn test_upgrade_pool() {
    let setup = Setup::default();
    assert!(setup
        .contract
        .try_upgrade(&setup.pool_address, &install_dummy_wasm(&setup.env))
        .is_ok());
}
