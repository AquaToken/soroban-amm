mod standard_pool_client {
    soroban_sdk::contractimport!(
        file = "../liquidity_pool/target/wasm32-unknown-unknown/release/soroban_liquidity_pool_contract.wasm"
    );
}
mod stableswap_pool_client {
    soroban_sdk::contractimport!(
        file = "../liquidity_pool_stableswap/target/wasm32-unknown-unknown/release/soroban_liquidity_pool_stableswap_contract.wasm"
    );
}

pub type StandardLiquidityPoolClient<'a> = standard_pool_client::Client<'a>;
pub type StableSwapLiquidityPoolClient<'a> = stableswap_pool_client::Client<'a>;
