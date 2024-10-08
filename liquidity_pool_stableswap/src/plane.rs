pub mod pool_plane {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_plane_contract.wasm"
    );
}

pub use crate::plane::pool_plane::Client as PoolPlaneClient;

use crate::normalize::xp;
use crate::storage::{
    get_fee, get_future_a, get_future_a_time, get_initial_a, get_initial_a_time, get_plane,
    get_reserves,
};
use soroban_sdk::{symbol_short, Env, Vec};

fn get_pool_data(e: &Env) -> (Vec<u128>, Vec<u128>) {
    (
        Vec::from_array(
            e,
            [
                get_fee(e) as u128,
                get_initial_a(e),
                get_initial_a_time(e) as u128,
                get_future_a(e),
                get_future_a_time(e) as u128,
            ],
        ),
        xp(e, &get_reserves(e)), // save reserves in normalized form
    )
}

pub fn update_plane(e: &Env) {
    let (init_args, reserves) = get_pool_data(e);
    PoolPlaneClient::new(e, &get_plane(e)).update(
        &e.current_contract_address(),
        &symbol_short!("stable"),
        &init_args,
        &reserves,
    );
}
