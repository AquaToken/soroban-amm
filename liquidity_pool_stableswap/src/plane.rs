pub mod pool_plane {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_plane_contract.wasm"
    );
}

pub use crate::plane::pool_plane::Client as PoolPlaneClient;

use crate::storage::{get_fee, get_plane, get_reserves};
use soroban_sdk::{symbol_short, Env, Vec};

fn get_pool_data(e: &Env, a: u128) -> (Vec<u128>, Vec<u128>) {
    (Vec::from_array(e, [get_fee(e) as u128, a]), get_reserves(e))
}

pub fn update_plane(e: &Env, a: u128) {
    let (init_args, reserves) = get_pool_data(e, a);
    PoolPlaneClient::new(e, &get_plane(e)).update(
        &e.current_contract_address(),
        &symbol_short!("stable"),
        &init_args,
        &reserves,
    );
}
