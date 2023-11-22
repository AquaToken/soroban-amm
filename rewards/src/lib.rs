#![no_std]

use soroban_sdk::Env;

pub mod manager;
pub mod storage;

pub mod reward_token {
    soroban_sdk::contractimport!(
        file = "../token/target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
}
pub use manager::Manager;
pub use reward_token::Client;
pub use storage::Storage;
pub use utils;

#[derive(Clone)]
pub struct Rewards(Env);

impl Rewards {
    #[inline(always)]
    pub fn new(env: &Env) -> Rewards {
        Rewards(env.clone())
    }

    pub fn storage(&self) -> Storage {
        Storage::new(&self.0)
    }

    pub fn manager(&self) -> Manager {
        Manager::new(&self.0, self.storage())
    }
}
