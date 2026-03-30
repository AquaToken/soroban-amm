#![allow(dead_code)]

use soroban_sdk::{Address, Env};

pub trait Plane {
    // Configure pools plane address to be used as lightweight proxy to optimize instructions.
    fn init_pools_plane(e: Env, plane: Address);

    // Update pools plane address.
    fn set_pools_plane(e: Env, admin: Address, plane: Address);

    // Get pools plane address.
    fn get_pools_plane(e: Env) -> Address;

    // Re-publish current pool data to plane.
    fn backfill_plane_data(e: Env);
}
