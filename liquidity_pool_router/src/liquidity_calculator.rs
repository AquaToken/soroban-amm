mod liquidity_calculator_client {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_liquidity_calculator_contract.wasm"
    );
}

pub use crate::liquidity_calculator::liquidity_calculator_client::Client as LiquidityCalculatorClient;
