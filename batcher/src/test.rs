#![cfg(test)]
extern crate std;

use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env, Symbol, TryFromVal, Vec};

use crate::{ContractBatcher, ContractBatcherClient};

mod pool_plane {
    soroban_sdk::contractimport!(file = "../contracts/soroban_liquidity_pool_plane_contract.wasm");
}

fn create_plane_contract<'a>(e: &Env) -> pool_plane::Client<'a> {
    pool_plane::Client::new(e, &e.register(pool_plane::WASM, ()))
}

fn create_contract<'a>(e: &Env) -> ContractBatcherClient<'a> {
    let client = ContractBatcherClient::new(e, &e.register(ContractBatcher {}, ()));
    client
}

#[test]
fn test() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let plane_client = create_plane_contract(&e);
    let plane = plane_client.address.clone();

    let pool1 = Address::generate(&e);
    let pool2 = Address::generate(&e);

    let batcher = create_contract(&e);
    batcher.batch(
        &Vec::from_array(&e, [pool1.clone(), pool2.clone()]),
        &Vec::from_array(
            &e,
            [
                (
                    plane.clone(),
                    Symbol::new(&e, "update"),
                    Vec::from_array(
                        &e,
                        [
                            pool1.clone().to_val(),
                            Symbol::new(&e, "constant_product").to_val(),
                            Vec::from_array(&e, [1_u128]).to_val(),
                            Vec::from_array(&e, [2_u128, 3_u128]).to_val(),
                        ],
                    ),
                ),
                (
                    plane.clone(),
                    Symbol::new(&e, "update"),
                    Vec::from_array(
                        &e,
                        [
                            pool2.clone().to_val(),
                            Symbol::new(&e, "stable").to_val(),
                            Vec::from_array(&e, [4_u128]).to_val(),
                            Vec::from_array(&e, [5_u128, 6_u128]).to_val(),
                        ],
                    ),
                ),
            ],
        ),
        &true,
    );

    let results = plane_client.get(&Vec::from_array(&e, [pool1.clone(), pool2.clone()]));
    assert_eq!(
        results,
        Vec::from_array(
            &e,
            [
                (
                    Symbol::new(&e, "constant_product"),
                    Vec::from_array(&e, [1_u128]),
                    Vec::from_array(&e, [2_u128, 3_u128]),
                ),
                (
                    Symbol::new(&e, "stable"),
                    Vec::from_array(&e, [4_u128]),
                    Vec::from_array(&e, [5_u128, 6_u128]),
                ),
            ],
        )
    );

    let batch_results = batcher.batch(
        &Vec::from_array(&e, []),
        &Vec::from_array(
            &e,
            [
                (
                    plane.clone(),
                    Symbol::new(&e, "get"),
                    Vec::from_array(&e, [Vec::from_array(&e, [pool1.clone()]).to_val()]),
                ),
                (
                    plane.clone(),
                    Symbol::new(&e, "get"),
                    Vec::from_array(&e, [Vec::from_array(&e, [pool2.clone()]).to_val()]),
                ),
            ],
        ),
        &true,
    );
    let result1 = batch_results.get(0).unwrap();
    let result1_vec: Vec<(Symbol, Vec<u128>, Vec<u128>)> = Vec::try_from_val(&e, &result1).unwrap();
    let pool1_result = result1_vec.get(0).unwrap();
    assert_eq!(
        pool1_result,
        (
            Symbol::new(&e, "constant_product"),
            Vec::from_array(&e, [1_u128]),
            Vec::from_array(&e, [2_u128, 3_u128]),
        )
    );
    let result2 = batch_results.get(1).unwrap();
    let result2_vec: Vec<(Symbol, Vec<u128>, Vec<u128>)> = Vec::try_from_val(&e, &result2).unwrap();
    let pool2_result = result2_vec.get(0).unwrap();
    assert_eq!(
        pool2_result,
        (
            Symbol::new(&e, "stable"),
            Vec::from_array(&e, [4_u128]),
            Vec::from_array(&e, [5_u128, 6_u128]),
        )
    );
}
