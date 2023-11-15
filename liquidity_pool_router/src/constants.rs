pub(crate) const MAX_POOLS_FOR_PAIR: u32 = 10;
pub(crate) const CONSTANT_PRODUCT_FEE_AVAILABLE: [u32; 3] = [10, 30, 100];
pub(crate) const STABLE_SWAP_MAX_POOLS: u32 = 3;

pub(crate) const DAY_IN_LEDGERS: u32 = 17280;
pub(crate) const INSTANCE_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const INSTANCE_LIFETIME_THRESHOLD: u32 = INSTANCE_BUMP_AMOUNT - DAY_IN_LEDGERS;

pub(crate) const POOL_BUMP_AMOUNT: u32 = 30 * DAY_IN_LEDGERS;
pub(crate) const POOL_LIFETIME_THRESHOLD: u32 = POOL_BUMP_AMOUNT - DAY_IN_LEDGERS;

// todo: make configurable
pub(crate) const POOL_CREATION_FEE: i128 = 1000_0000000;
