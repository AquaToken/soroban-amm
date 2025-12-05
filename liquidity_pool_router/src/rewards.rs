use rewards::{Rewards, RewardsContext};
use soroban_sdk::{Address, Env};

// page size of 100 is optimal since 8 bytes key + 16 bytes value * 100 = 2400 bytes per page
// it gives us up to 26 aggregation layers
#[cfg(not(test))]
pub(crate) const PAGE_SIZE: u64 = 100;

#[cfg(test)]
pub(crate) const PAGE_SIZE: u64 = 5;

#[derive(Clone)]
pub struct NoContext;

impl RewardsContext for NoContext {
    fn get_total_shares(&self) -> u128 {
        panic!("not implemented");
    }

    fn get_user_shares(&self, _user: &Address) -> u128 {
        panic!("not implemented");
    }
}

pub(crate) fn get_rewards_manager(e: &Env) -> Rewards<NoContext> {
    Rewards::new(e, PAGE_SIZE, NoContext {})
}
