use soroban_sdk::{contracttype, Address, BytesN};

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const INSTANCE_BUMP_AMOUNT: u32 = 7 * DAY_IN_LEDGERS;
pub(crate) const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub(crate) const POOL_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const POOL_LIFETIME_THRESHOLD: u32 = POOL_BUMP_AMOUNT - DAY_IN_LEDGERS;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Pool(BytesN<32>),
    Admin,
    TokenHash,
    PoolHash,
    PoolsList, // temp key to handle list of pools to upgrade them
}
