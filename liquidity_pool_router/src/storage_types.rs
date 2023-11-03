use soroban_sdk::{contracttype, BytesN};

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub(crate) const POOL_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const POOL_LIFETIME_THRESHOLD: u32 = POOL_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub(crate) const MAX_POOLS_FOR_PAIR: u32 = 10;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    TokensPairPools(BytesN<32>),
    Admin,
    TokenHash,
    RewardToken,
    ConstantPoolHash,
    StableSwapPoolHash,
}
