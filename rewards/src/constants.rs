pub(crate) const REWARD_PRECISION: u128 = 1_000;

#[cfg(not(test))]
pub(crate) const PAGE_SIZE: u64 = 1000;

#[cfg(test)]
pub(crate) const PAGE_SIZE: u64 = 5;
