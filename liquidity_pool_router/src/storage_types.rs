use soroban_sdk::{contracttype, BytesN};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    TokensPairPools(BytesN<32>),
    Admin,
    TokenHash,
    InitPoolPaymentToken,
    InitPoolPaymentAmount,
    RewardToken,
    ConstantPoolHash,
    StableSwapPoolHash(u32),
    StableSwapCounter,
}
