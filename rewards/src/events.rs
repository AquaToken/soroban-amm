use soroban_sdk::{Env, Symbol};

#[derive(Clone)]
pub struct Events(Env);

impl Events {
    #[inline(always)]
    pub fn env(&self) -> &Env {
        &self.0
    }

    #[inline(always)]
    pub fn new(env: &Env) -> Events {
        Events(env.clone())
    }

    pub fn set_rewards_config(&self, expired_at: u64, tps: u128) {
        self.env().events().publish(
            (Symbol::new(self.env(), "set_rewards_config"),),
            (expired_at, tps),
        )
    }
}
