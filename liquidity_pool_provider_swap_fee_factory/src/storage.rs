use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env};
use utils::bump::bump_instance;
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
}

generate_instance_storage_getter_and_setter!(router, DataKey::Router, Address);
generate_instance_storage_getter_and_setter!(
    fee_contract_wasm,
    DataKey::FeeContractWASM,
    BytesN<32>
);
