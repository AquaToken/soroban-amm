#![cfg(test)]
extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{vec, Address, Env, IntoVal, Symbol, TryFromVal, Vec};

use crate::{ContractGuard, ContractGuardClient};

mod pool_plane {
    soroban_sdk::contractimport!(file = "../contracts/soroban_liquidity_pool_plane_contract.wasm");
}

fn create_plane_contract<'a>(e: &Env) -> pool_plane::Client<'a> {
    pool_plane::Client::new(e, &e.register(pool_plane::WASM, ()))
}

fn create_contract<'a>(e: &Env) -> ContractGuardClient<'a> {
    let client = ContractGuardClient::new(e, &e.register(ContractGuard, ()));
    client
}

#[test]
fn test() {
    let e = Env::default();
    e.mock_all_auths();
    e.cost_estimate().budget().reset_unlimited();

    let plane_client = create_plane_contract(&e);
    let pool = Address::generate(&e);

    let guard = create_contract(&e);
    plane_client.update(
        &pool,
        &Symbol::new(&e, "constant_product"),
        &Vec::from_array(&e, [1_u128]),
        &Vec::from_array(&e, [2_u128, 3_u128]),
    );

    assert_eq!(
        u32::try_from_val(
            &e,
            &guard.assert_result(
                &vec![&e],
                &plane_client.address,
                &Symbol::new(&e, "version"),
                &vec![&e],
                &105_u32.into_val(&e),
            )
        )
        .unwrap(),
        105
    );
    assert!(guard
        .try_assert_result(
            &vec![&e],
            &plane_client.address,
            &Symbol::new(&e, "version"),
            &vec![&e],
            &105_i32.into_val(&e),
        )
        .is_err());

    let expected_result = plane_client.get(&Vec::from_array(&e, [pool.clone()]));
    assert_eq!(
        Vec::try_from_val(
            &e,
            &guard.assert_result(
                &vec![&e],
                &plane_client.address,
                &Symbol::new(&e, "get"),
                &vec![&e, vec![&e, pool.to_val()].to_val()],
                &expected_result.to_val(),
            )
        )
        .unwrap(),
        expected_result
    );
}
