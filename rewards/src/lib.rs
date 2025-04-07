#![no_std]

use soroban_sdk::Env;

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

#[derive(Clone)]
pub struct Rewards {
    env: Env,
    config: RewardsConfig,
}

impl Rewards {
    #[inline(always)]
    pub fn new(env: &Env, page_size: u64) -> Rewards {
        Rewards {
            env: env.clone(),
            config: RewardsConfig { page_size },
        }
    }

    pub fn storage(&self) -> Storage {
        Storage::new(&self.env)
    }

    pub fn manager(&self) -> Manager {
        Manager::new(&self.env, self.storage(), &self.config)
    }
}
