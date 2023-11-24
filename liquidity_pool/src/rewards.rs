use rewards::Rewards;
use soroban_sdk::Env;

#[cfg(not(test))]
pub(crate) const PAGE_SIZE: u64 = 1000;

#[cfg(test)]
pub(crate) const PAGE_SIZE: u64 = 2;

pub(crate) fn get_rewards_manager(e: &Env) -> Rewards {
    Rewards::new(&e, PAGE_SIZE)
}
