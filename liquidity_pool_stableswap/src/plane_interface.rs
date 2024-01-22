use soroban_sdk::{Address, Env};

pub trait Plane {
    fn initialize_plane(e: Env, plane: Address);
}
