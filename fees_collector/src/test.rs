#![cfg(test)]
extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

use crate::testutils::{create_contract, Setup};

#[test]
fn test() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let collector = create_contract(&e);
    collector.init_admin(&admin);
}

#[should_panic(expected = "Error(Contract, #103)")]
#[test]
fn test_init_admin_twice() {
    let setup = Setup::default();
    setup.collector.init_admin(&setup.admin);
}
