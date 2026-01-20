use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env};
use utils::storage_errors::StorageError;
use utils::{
    generate_instance_storage_getter, generate_instance_storage_getter_and_setter,
    generate_instance_storage_setter,
};

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Router,
    FeeContractWASM,
    ContractSequence(Address),
}

generate_instance_storage_getter_and_setter!(router, DataKey::Router, Address);
generate_instance_storage_getter_and_setter!(
    fee_contract_wasm,
    DataKey::FeeContractWASM,
    BytesN<32>
);

pub(crate) fn get_contract_sequence(env: &Env, operator: Address) -> u32 {
    let key = DataKey::ContractSequence(operator);
    env.storage().persistent().get(&key).unwrap_or(0)
}

pub(crate) fn set_contract_sequence(env: &Env, operator: Address, sequence: u32) {
    let key = DataKey::ContractSequence(operator);
    env.storage().persistent().set(&key, &sequence);
}
