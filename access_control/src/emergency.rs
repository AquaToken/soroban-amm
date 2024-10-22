use soroban_sdk::Env;
use utils::bump::bump_instance;
use crate::storage::DataKey;

pub fn get_emergency_mode(e: &Env) -> bool {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::EmergencyMode)
        .unwrap_or(false)
}

pub fn set_emergency_mode(e: &Env, value: &bool) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::EmergencyMode, value);
}

