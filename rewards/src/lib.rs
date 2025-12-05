#![no_std]

use soroban_sdk::{Address, Env};

pub mod boost_feed;
mod constants;
pub mod errors;
pub mod events;
pub mod manager;
pub mod storage;

pub use manager::Manager;
pub use storage::Storage;
pub use utils;

#[derive(Clone)]
pub struct RewardsConfig {
    page_size: u64,
}

pub trait RewardsContext {
    fn get_total_shares(&self) -> u128;
    fn get_user_shares(&self, user: &Address) -> u128;
}

pub struct Rewards<Ctx>
where
    Ctx: RewardsContext + Clone,
{
    env: Env,
    config: RewardsConfig,
    context: Ctx,
}

impl<Ctx> Rewards<Ctx>
where
    Ctx: RewardsContext + Clone,
{
    #[inline(always)]
    pub fn new(env: &Env, page_size: u64, context: Ctx) -> Rewards<Ctx> {
        Rewards {
            env: env.clone(),
            config: RewardsConfig { page_size },
            context,
        }
    }

    pub fn storage(&self) -> Storage {
        Storage::new(&self.env)
    }

    pub fn manager(&self) -> Manager<Ctx> {
        Manager::new(
            &self.env,
            self.storage(),
            &self.config,
            self.context.clone(),
        )
    }
}
