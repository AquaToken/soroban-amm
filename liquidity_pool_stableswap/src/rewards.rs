use rewards::Rewards;
use soroban_sdk::Env;

// page size of 100 is optimal since 8 bytes key + 16 bytes value * 100 = 2400 bytes per page
// it gives us up to 26 aggregation layers
#[cfg(not(test))]
pub(crate) const PAGE_SIZE: u64 = 100;

#[cfg(test)]
pub(crate) const PAGE_SIZE: u64 = 5;

pub(crate) fn get_rewards_manager(e: &Env) -> Rewards {
    Rewards::new(e, PAGE_SIZE)
}
