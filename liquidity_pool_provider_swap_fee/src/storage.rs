use paste::paste;
use soroban_sdk::{contracttype, panic_with_error, Address, Env};
use utils::bump::bump_instance;
use utils::storage_errors::StorageError;
use utils::{
    generate_instance_storage_getter, generate_instance_storage_getter_and_setter,
    generate_instance_storage_setter,
};

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Router,             // Address of the AMM router.
    Operator, // Address of the operator. Operator is capable to configure fees and claim them.
    FeeDestination, // Fee destination address
    MaxSwapFeeFraction, // Max swap fee in basis points (100 = 1%)
}

generate_instance_storage_getter_and_setter!(router, DataKey::Router, Address);
generate_instance_storage_getter_and_setter!(operator, DataKey::Operator, Address);
generate_instance_storage_getter_and_setter!(fee_destination, DataKey::FeeDestination, Address);
generate_instance_storage_getter_and_setter!(
    max_swap_fee_fraction,
    DataKey::MaxSwapFeeFraction,
    u32
);
