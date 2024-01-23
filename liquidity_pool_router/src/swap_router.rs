mod swap_router_client {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_swap_router_contract.wasm"
    );
}

pub use crate::swap_router::swap_router_client::Client as SwapRouterClient;
