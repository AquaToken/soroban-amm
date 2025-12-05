use rewards::{Rewards, RewardsContext};
use soroban_sdk::{Address, Env};
use token_share::{get_total_shares, get_user_balance_shares};

// page size of 100 is optimal since 8 bytes key + 16 bytes value * 100 = 2400 bytes per page
// it gives us up to 26 aggregation layers
#[cfg(not(test))]
pub(crate) const PAGE_SIZE: u64 = 100;

#[cfg(test)]
pub(crate) const PAGE_SIZE: u64 = 5;

#[derive(Clone)]
struct StaticPoolContext {
    total_shares: u128,
    user: Option<Address>,
    user_shares: u128,
}

impl RewardsContext for StaticPoolContext {
    fn get_total_shares(&self) -> u128 {
        self.total_shares
    }

    fn get_user_shares(&self, user: &Address) -> u128 {
        match &self.user {
            Some(stored_user) => {
                if user != stored_user {
                    panic!("User address mismatch");
                } else {
                    self.user_shares
                }
            }
            None => panic!("No user address stored"),
        }
    }
}

// Pool context can be static to use pre-fetched values or dynamic to fetch live values
#[derive(Clone)]
pub struct PoolContext {
    env: Env,
    static_context: Option<StaticPoolContext>,
}

impl RewardsContext for PoolContext {
    fn get_total_shares(&self) -> u128 {
        match &self.static_context {
            Some(static_ctx) => static_ctx.get_total_shares(),
            None => get_total_shares(&self.env),
        }
    }

    fn get_user_shares(&self, user: &Address) -> u128 {
        match &self.static_context {
            Some(static_ctx) => static_ctx.get_user_shares(user),
            None => get_user_balance_shares(&self.env, &user),
        }
    }
}

pub(crate) fn get_rewards_manager(e: &Env) -> Rewards<PoolContext> {
    Rewards::new(
        e,
        PAGE_SIZE,
        PoolContext {
            env: e.clone(),
            static_context: None,
        },
    )
}

pub(crate) fn get_static_rewards_manager(
    e: &Env,
    total_shares: u128,
    user: Option<Address>,
    user_shares: u128,
) -> Rewards<PoolContext> {
    Rewards::new(
        e,
        PAGE_SIZE,
        PoolContext {
            env: e.clone(),
            static_context: Some(StaticPoolContext {
                total_shares,
                user,
                user_shares,
            }),
        },
    )
}
