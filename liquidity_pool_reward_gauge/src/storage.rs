use paste::paste;
use soroban_sdk::{contracttype, Address, Env, Map};
use utils::bump::bump_instance;
use utils::{
    generate_instance_storage_getter_and_setter_with_default,
    generate_instance_storage_getter_with_default, generate_instance_storage_setter,
};

#[contracttype]
pub struct RewardConfig {
    pub start_at: u64,
    pub tps: u128,
    pub expired_at: u64,
}

#[derive(Clone)]
#[contracttype]
enum DataKey {
    RewardGaugesMap,
    IsKilledGaugesClaim,
}

generate_instance_storage_getter_and_setter_with_default!(
    is_killed_gauges_claim,
    DataKey::IsKilledGaugesClaim,
    bool,
    false
);

pub(crate) fn get_reward_gauges(e: &Env) -> Map<Address, Address> {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::RewardGaugesMap)
        .unwrap_or(Map::new(e))
}

pub(crate) fn set_reward_gauges(e: &Env, value: &Map<Address, Address>) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::RewardGaugesMap, value)
}
