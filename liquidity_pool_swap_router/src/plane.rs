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

pub(crate) fn parse_stableswap_data(
    init_args: Vec<u128>,
    reserves: Vec<u128>,
) -> (u128, u128, Vec<u128>) {
    (
        init_args.get(0).unwrap(),
        init_args.get(1).unwrap(),
        reserves,
    )
}
