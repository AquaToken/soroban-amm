use soroban_sdk::{contracttype, Address, Env};
pub use utils::bump::bump_instance;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    TokenA,
    TokenB,
    ReserveA,
    ReserveB,
    FeeFraction, // 1 = 0.01%
    Plane,
}

pub fn get_token_a(e: &Env) -> Address {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::TokenA)
        .expect("Trying to get Token A")
}

pub fn get_token_b(e: &Env) -> Address {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::TokenB)
        .expect("Trying to get Token B")
}

pub fn get_reserve_a(e: &Env) -> u128 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::ReserveA)
        .expect("Trying to get Reserve A")
}

pub fn get_reserve_b(e: &Env) -> u128 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::ReserveB)
        .expect("Trying to get Reserve B")
}

pub fn put_token_a(e: &Env, contract: Address) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::TokenA, &contract)
}

pub fn put_token_b(e: &Env, contract: Address) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::TokenB, &contract)
}

pub fn put_reserve_a(e: &Env, amount: u128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::ReserveA, &amount)
}

pub fn put_reserve_b(e: &Env, amount: u128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::ReserveB, &amount)
}

pub fn get_fee_fraction(e: &Env) -> u32 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::FeeFraction)
        .expect("Please initialize fee fraction")
}

pub fn put_fee_fraction(e: &Env, value: u32) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::FeeFraction, &value)
}

pub(crate) fn set_plane(e: &Env, plane: &Address) {
    let key = DataKey::Plane;
    bump_instance(e);
    e.storage().instance().set(&key, plane);
}

pub(crate) fn get_plane(e: &Env) -> Address {
    let key = DataKey::Plane;
    e.storage()
        .instance()
        .get(&key)
        .expect("unable to get plane")
}

pub(crate) fn has_plane(e: &Env) -> bool {
    let key = DataKey::Plane;
    e.storage().instance().has(&key)
}
