mod pool_plane_client {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_plane_contract.wasm"
    );
}

pub use crate::plane::pool_plane_client::Client as PoolPlaneClient;

use soroban_sdk::Vec;

pub(crate) fn parse_standard_data(init_args: Vec<u128>, reserves: Vec<u128>) -> (u128, Vec<u128>) {
    (init_args.get(0).unwrap(), reserves)
}

pub struct StableSwapPoolData {
    pub(crate) fee: u128,
    pub(crate) initial_a: u128,
    pub(crate) initial_a_time: u128,
    pub(crate) future_a: u128,
    pub(crate) future_a_time: u128,
    pub(crate) xp: Vec<u128>,
}

// * `init_args`: [fee, initial_a, initial_a_time, future_a, future_a_time]
// * `xp`: pool balances list in normalized form
pub(crate) fn parse_stableswap_data(init_args: Vec<u128>, xp: Vec<u128>) -> StableSwapPoolData {
    StableSwapPoolData {
        fee: init_args.get(0).unwrap(),
        initial_a: init_args.get(1).unwrap(),
        initial_a_time: init_args.get(2).unwrap(),
        future_a: init_args.get(3).unwrap(),
        future_a_time: init_args.get(4).unwrap(),
        xp,
    }
}
