#![cfg(test)]
extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

use crate::{FeesCollector, FeesCollectorClient};

fn create_contract<'a>(e: &Env) -> FeesCollectorClient<'a> {
    let client = FeesCollectorClient::new(e, &e.register_contract(None, FeesCollector {}));
    client
}

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
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let collector = create_contract(&e);
    collector.init_admin(&admin);
    collector.init_admin(&admin);
}
