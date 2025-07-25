use soroban_sdk::contracttype;

#[contracttype]
pub enum WASMDataKey {
    TokenHash,
    TokenFutureWASM,
    GaugeWASM,
    FutureGaugeWASM,
    ConstantPoolHash,
    StableSwapPoolHash,
}
