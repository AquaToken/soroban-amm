use soroban_sdk::{contracttype, Address, Env};

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Plane,
}

pub(crate) fn set_plane(e: &Env, plane: &Address) {
    let key = DataKey::Plane;
    e.storage().instance().set(&key, plane);
}

pub(crate) fn get_plane(e: &Env) -> Address {
    let key = DataKey::Plane;
    e.storage()
        .instance()
        .get(&key)
        .expect("unable to get plane")
}
