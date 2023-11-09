use soroban_sdk::{contracttype, BytesN};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    TokensPairPools(BytesN<32>),
    Admin,
    TokenHash,
    RewardToken,
    ConstantPoolHash,
    StableSwapPoolHash(u32),
}
