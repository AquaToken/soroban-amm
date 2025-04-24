#![cfg(test)]
extern crate std;

use crate::testutils::Setup;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Vec};

#[test]
fn test_strict_send() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    let result = setup.contract.swap_chained(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &9870300,
        &100,
    );
    assert_eq!(result, 9870300); // (10000000 - .3%) - 1%
    assert_eq!(setup.token_b.balance(&user), 9870300);
}

#[test]
fn test_strict_receive() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_2000000);
    let result = setup.contract.swap_chained_strict_receive(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &1_2000000,
        &100,
    );
    assert_eq!(result, 10130392); // ~ (10000000 + .3%) + 1%
    assert_eq!(setup.token_b.balance(&user), 1_0000000);
}

#[test]
#[should_panic(expected = "Error(Contract, #2904)")]
fn test_strict_send_fee_over_max() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    setup.contract.swap_chained(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &9870300,
        &101,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2904)")]
fn test_strict_receive_fee_over_max() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_2000000);
    setup.contract.swap_chained_strict_receive(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &1_2000000,
        &101,
    );
}

#[test]
fn test_strict_send_bad_slippage() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    let swap_path = Vec::from_array(
        &setup.env,
        [(tokens, pool_index, setup.token_b.address.clone())],
    );
    assert!(setup
        .contract
        .try_swap_chained(
            &user,
            &swap_path,
            &setup.token_a.address,
            &1_0000000,
            &9870301, // value is not enough to cover provider fee
            &100,
        )
        .is_err());
    assert!(setup
        .contract
        .try_swap_chained(
            &user,
            &swap_path,
            &setup.token_a.address,
            &1_0000000,
            &9870300,
            &100,
        )
        .is_ok());
}

#[test]
fn test_strict_receive_bad_slippage() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_2000000);
    let swap_path = Vec::from_array(
        &setup.env,
        [(tokens, pool_index, setup.token_b.address.clone())],
    );
    assert!(setup
        .contract
        .try_swap_chained_strict_receive(
            &user,
            &swap_path,
            &setup.token_a.address,
            &1_0000000,
            &10130391,
            &100,
        )
        .is_err());
    assert!(setup
        .contract
        .try_swap_chained_strict_receive(
            &user,
            &swap_path,
            &setup.token_a.address,
            &1_0000000,
            &10130392,
            &100,
        )
        .is_ok());
}

#[test]
fn test_strict_send_no_fee() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    let result = setup.contract.swap_chained(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &0,
        &0,
    );
    assert_eq!(result, 9969999); // (10000000 - .3%)
}

#[test]
fn test_strict_receive_no_fee() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_2000000);
    let result = setup.contract.swap_chained_strict_receive(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &1_2000000,
        &0,
    );
    assert_eq!(result, 10030092); // ~ (10000000 + .3%)
}

#[test]
fn test_claim_fee() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    setup.contract.swap_chained(
        &user,
        &Vec::from_array(
            &setup.env,
            [(tokens, pool_index, setup.token_b.address.clone())],
        ),
        &setup.token_a.address,
        &1_0000000,
        &0,
        &100,
    );
    assert_eq!(
        setup
            .contract
            .claim_fees(&setup.operator, &setup.token_b.address),
        99699
    ); // ~ (10000000 - .3%) * 1%
    assert_eq!(
        setup
            .contract
            .claim_fees(&setup.operator, &setup.token_a.address),
        0
    );
    assert_eq!(setup.token_a.balance(&setup.fee_destination), 0);
    assert_eq!(setup.token_b.balance(&setup.fee_destination), 99699);
}

#[test]
fn test_claim_fee_and_swap() {
    let setup = Setup::default();

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    setup.contract.swap_chained(
        &user,
        &Vec::from_array(
            &setup.env,
            [(
                tokens.clone(),
                pool_index.clone(),
                setup.token_b.address.clone(),
            )],
        ),
        &setup.token_a.address,
        &1_0000000,
        &0,
        &100,
    );
    assert_eq!(
        setup.contract.claim_fees_and_swap(
            &setup.operator,
            &Vec::from_array(
                &setup.env,
                [(tokens, pool_index, setup.token_a.address.clone())],
            ),
            &setup.token_b.address,
            &0,
        ),
        99399
    ); // ~ (10000000 - .3%) * 1%
    assert_eq!(
        setup
            .contract
            .claim_fees(&setup.operator, &setup.token_a.address),
        0
    );
    assert_eq!(setup.token_a.balance(&setup.fee_destination), 99399);
    assert_eq!(setup.token_b.balance(&setup.fee_destination), 0);
}
