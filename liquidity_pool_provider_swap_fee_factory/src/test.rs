#![cfg(test)]
extern crate std;

use crate::testutils::{swap_fee_collector, Setup};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Vec};

#[test]
fn test() {
    let setup = Setup::default();

    let operator = Address::generate(&setup.env);
    let fee_destination = Address::generate(&setup.env);
    let swap_fee_collector =
        setup
            .contract
            .deploy_swap_fee_contract(&operator, &fee_destination, &100);
    let swap_fee_collector_client =
        swap_fee_collector::Client::new(&setup.env, &swap_fee_collector);

    let tokens = Vec::from_array(
        &setup.env,
        [setup.token_a.address.clone(), setup.token_b.address.clone()],
    );
    let (pool_index, _pool_address) = setup.router.get_pools(&tokens).iter().last().unwrap();

    let user = Address::generate(&setup.env);
    setup.token_a_admin_client.mint(&user, &1_0000000);
    let result = swap_fee_collector_client.swap_chained(
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
    assert_eq!(result, 9870300); // (10000000 - .3%) - 1%
}

#[test]
fn test_deploy_multiple_times() {
    let setup = Setup::default();

    let operator = Address::generate(&setup.env);
    let fee_destination = Address::generate(&setup.env);
    let swap_fee_collector_1 =
        setup
            .contract
            .deploy_swap_fee_contract(&operator, &fee_destination, &100);
    let swap_fee_collector_2 =
        setup
            .contract
            .deploy_swap_fee_contract(&operator, &fee_destination, &100);
    assert_ne!(swap_fee_collector_1, swap_fee_collector_2,);
}
