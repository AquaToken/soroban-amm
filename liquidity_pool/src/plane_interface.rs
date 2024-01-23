use soroban_sdk::{Address, Env};

pub trait Plane {
    // configure pools plane address to be used as lightweight proxy to optimize instructions & batch operations
    fn set_pools_plane(e: Env, plane: Address);

    // get pools plane address
    fn get_pools_plane(e: Env) -> Address;
}
