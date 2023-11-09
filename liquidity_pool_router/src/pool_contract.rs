mod standard_pool_client {
    soroban_sdk::contractimport!(
        file = "../liquidity_pool/target/wasm32-unknown-unknown/release/soroban_liquidity_pool_contract.wasm"
    );
}

pub type StandardLiquidityPoolClient<'a> = standard_pool_client::Client<'a>;
