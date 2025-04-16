#![cfg(test)]

use crate::testutils::Setup;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::Address;

#[test]
fn test_operator_set_fee() {
    let setup = Setup::default();
    setup.contract.set_swap_fee_fraction(&setup.operator, &200);
}

#[test]
#[should_panic(expected = "Error(Contract, #102)")]
fn test_third_party_user_set_fee() {
    let setup = Setup::default();
    setup
        .contract
        .set_swap_fee_fraction(&Address::generate(&setup.env), &200);
}
