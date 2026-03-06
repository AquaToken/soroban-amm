use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env, Vec};
pub use utils::bump::bump_instance;
use utils::generate_instance_storage_getter;
use utils::storage_errors::StorageError;
use utils::{
    generate_instance_storage_getter_and_setter,
    generate_instance_storage_getter_and_setter_with_default,
    generate_instance_storage_getter_with_default, generate_instance_storage_setter,
};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    TokenA,
    TokenB,
    ReserveA,
    ReserveB,
    FeeFraction, // 1 = 0.01%
    Plane,
    Router,
    IsKilledSwap,
    IsKilledDeposit,
    IsKilledClaim,

    TokenFutureWASM,
    GaugeFutureWASM,

    ProtocolFeeFraction, // part of the fee that goes to the protocol, 5000 = 50% of the fee goes to the protocol
    ProtocolFeeA,
    ProtocolFeeB,

    ReservesSyncLedger,
}

generate_instance_storage_getter_and_setter_with_default!(
    is_killed_swap,
    DataKey::IsKilledSwap,
    bool,
    false
);
generate_instance_storage_getter_and_setter_with_default!(
    is_killed_deposit,
    DataKey::IsKilledDeposit,
    bool,
    false
);
generate_instance_storage_getter_and_setter_with_default!(
    is_killed_claim,
    DataKey::IsKilledClaim,
    bool,
    false
);
generate_instance_storage_getter_and_setter_with_default!(
    protocol_fee_fraction,
    DataKey::ProtocolFeeFraction,
    u32,
    0
);
generate_instance_storage_getter_and_setter_with_default!(
    protocol_fee_a,
    DataKey::ProtocolFeeA,
    u128,
    0
);
generate_instance_storage_getter_and_setter_with_default!(
    protocol_fee_b,
    DataKey::ProtocolFeeB,
    u128,
    0
);
generate_instance_storage_getter_and_setter!(token_a, DataKey::TokenA, Address);
generate_instance_storage_getter_and_setter!(token_b, DataKey::TokenB, Address);
generate_instance_storage_getter_and_setter!(reserve_a, DataKey::ReserveA, u128);
generate_instance_storage_getter_and_setter!(reserve_b, DataKey::ReserveB, u128);
generate_instance_storage_getter_and_setter!(fee_fraction, DataKey::FeeFraction, u32);
generate_instance_storage_getter_and_setter!(router, DataKey::Router, Address);
generate_instance_storage_getter_and_setter!(plane, DataKey::Plane, Address);
generate_instance_storage_getter_and_setter!(
    token_future_wasm,
    DataKey::TokenFutureWASM,
    BytesN<32>
);
generate_instance_storage_getter_and_setter!(
    gauge_future_wasm,
    DataKey::GaugeFutureWASM,
    BytesN<32>
);

generate_instance_storage_getter_and_setter_with_default!(
    reserves_sync_ledger,
    DataKey::ReservesSyncLedger,
    u32,
    0
);

pub(crate) fn has_plane(e: &Env) -> bool {
    bump_instance(e);
    let key = DataKey::Plane;
    e.storage().instance().has(&key)
}

// utility helpers to unify logic across different pool types
pub fn get_tokens(e: &Env) -> Vec<Address> {
    Vec::from_array(e, [get_token_a(e), get_token_b(e)])
}

pub fn get_reserves(e: &Env) -> Vec<u128> {
    Vec::from_array(e, [get_reserve_a(e), get_reserve_b(e)])
}

pub fn put_reserves(e: &Env, amounts: &Vec<u128>) {
    set_reserve_a(e, &amounts.get(0).unwrap());
    set_reserve_b(e, &amounts.get(1).unwrap());
}

pub fn get_protocol_fees(e: &Env) -> Vec<u128> {
    Vec::from_array(e, [get_protocol_fee_a(e), get_protocol_fee_b(e)])
}
