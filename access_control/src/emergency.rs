use crate::storage::DataKey;
use soroban_sdk::Env;

pub fn get_emergency_mode(e: &Env) -> bool {
    e.storage()
        .instance()
        .get(&DataKey::EmergencyMode)
        .unwrap_or(false)
}

pub fn set_emergency_mode(e: &Env, value: &bool) {
    e.storage().instance().set(&DataKey::EmergencyMode, value);
}
