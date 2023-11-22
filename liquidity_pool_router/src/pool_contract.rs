mod standard_pool_client {
    soroban_sdk::contractimport!(
        file = "../liquidity_pool/target/wasm32-unknown-unknown/release/soroban_liquidity_pool_contract.wasm"
    );
}

pub use crate::pool_contract::standard_pool_client::Client as StandardLiquidityPoolClient;
