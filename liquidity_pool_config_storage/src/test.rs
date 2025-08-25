#![cfg(test)]
extern crate std;

use crate::testutils::Setup;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contracttype, Address, BytesN, FromVal, IntoVal, U256};
use utils::test_utils::jump;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Admin,
    Operator,
    EmergencyMode,
    Router,
    TokenFutureWASM,
    ProtocolFeeFraction,
    LargeNumber,
}

#[test]
fn test_store_bytes() {
    let setup = Setup::default();
    let gauge_future_wasm = BytesN::from_array(&setup.env, &[1u8; 32]);
    let key = DataKey::TokenFutureWASM.into_val(&setup.env);
    setup
        .contract
        .set_value(&setup.admin, &key, &gauge_future_wasm.to_val());
    assert_eq!(
        BytesN::from_val(&setup.env, &setup.contract.get_value(&key).unwrap()),
        gauge_future_wasm,
    )
}

#[test]
fn test_store_address() {
    let setup = Setup::default();
    let router = Address::generate(&setup.env);
    let key = DataKey::Router.into_val(&setup.env);
    setup
        .contract
        .set_value(&setup.admin, &key, &router.to_val());
    assert_eq!(
        Address::from_val(&setup.env, &setup.contract.get_value(&key).unwrap()),
        router,
    )
}

#[test]
fn test_store_u32() {
    let setup = Setup::default();
    let protocol_fee_fraction = 5000_u32;
    let key = DataKey::ProtocolFeeFraction.into_val(&setup.env);
    setup.contract.set_value(
        &setup.admin,
        &key,
        &protocol_fee_fraction.into_val(&setup.env),
    );
    assert_eq!(
        u32::from_val(&setup.env, &setup.contract.get_value(&key).unwrap()),
        protocol_fee_fraction,
    )
}

#[test]
fn test_store_u256() {
    let setup = Setup::default();
    let two = U256::from_u128(&setup.env, 2);
    let large_number = U256::from_u128(&setup.env, u128::MAX).mul(&two);
    let key = DataKey::LargeNumber.into_val(&setup.env);
    setup
        .contract
        .set_value(&setup.admin, &key, &large_number.to_val());
    assert_eq!(
        U256::from_val(&setup.env, &setup.contract.get_value(&key).unwrap()).div(&two),
        U256::from_u128(&setup.env, u128::MAX),
    )
}

#[test]
#[should_panic(expected = "#102")]
fn test_override_admin() {
    let setup = Setup::default();
    let key = DataKey::Admin.into_val(&setup.env);
    setup
        .contract
        .set_value(&setup.admin, &key, &Address::generate(&setup.env).to_val());
}

#[test]
fn test_set_emergency_mode() {
    let setup = Setup::default();
    assert_eq!(setup.contract.get_emergency_mode(), false);
    setup
        .contract
        .set_emergency_mode(&setup.emergency_admin, &true);
    assert_eq!(setup.contract.get_emergency_mode(), true);
}

#[test]
#[should_panic(expected = "#102")]
fn test_override_emergency_mode() {
    let setup = Setup::default();
    assert_eq!(setup.contract.get_emergency_mode(), false,);
    setup.contract.set_value(
        &setup.admin,
        &DataKey::EmergencyMode.into_val(&setup.env),
        &true.into_val(&setup.env),
    );
}

#[test]
fn test_value_updated_timestamp() {
    let setup = Setup::default();
    let gauge_future_wasm = BytesN::from_array(&setup.env, &[1u8; 32]);
    let key = DataKey::TokenFutureWASM.into_val(&setup.env);
    jump(&setup.env, 42);
    setup
        .contract
        .set_value(&setup.admin, &key, &gauge_future_wasm.to_val());
    assert_eq!(
        BytesN::from_val(&setup.env, &setup.contract.get_value(&key).unwrap()),
        gauge_future_wasm,
    );
    jump(&setup.env, 100);
    assert_eq!(setup.contract.get_value_updated_at(&key), 42,);
}
