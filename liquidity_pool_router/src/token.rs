pub(crate) mod token {
    // soroban_sdk::contractimport!(file = "../soroban_token_spec.wasm");

    soroban_sdk::contractimport!(
        file = "../token/target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
}
