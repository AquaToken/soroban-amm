pub mod pool_plane {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_plane_contract.wasm"
    );
}

pub use crate::plane::pool_plane::Client as PoolPlaneClient;

use crate::storage::{get_fee_fraction, get_plane, get_reserve_a, get_reserve_b};
use soroban_sdk::{symbol_short, Env, Vec};

fn get_pool_data(e: &Env) -> (Vec<u128>, Vec<u128>) {
    (
        Vec::from_array(e, [get_fee_fraction(e) as u128]),
        Vec::from_array(e, [get_reserve_a(e), get_reserve_b(e)]),
    )
}

pub fn update_plane(e: &Env) {
    let (init_args, reserves) = get_pool_data(e);
    PoolPlaneClient::new(e, &get_plane(e)).update(
        &e.current_contract_address(),
        &symbol_short!("standard"),
        &init_args,
        &reserves,
    );
}
