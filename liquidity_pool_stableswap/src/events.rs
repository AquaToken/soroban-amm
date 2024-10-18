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

    pub fn commit_new_fee(&self, new_fee: u32) {
        self.env()
            .events()
            .publish((Symbol::new(self.env(), "commit_new_fee"),), (new_fee,))
    }

    pub fn apply_new_fee(&self, new_fee: u32) {
        self.env()
            .events()
            .publish((Symbol::new(self.env(), "apply_new_fee"),), (new_fee,))
    }

    pub fn revert_new_parameters(&self) {
        self.env()
            .events()
            .publish((Symbol::new(self.env(), "revert_new_parameters"),), ())
    }

    pub fn ramp_a(&self, future_a: u128, future_a_time: u64) {
        self.env().events().publish(
            (Symbol::new(self.env(), "ramp_a"),),
            (future_a, future_a_time),
        )
    }

    pub fn stop_ramp_a(&self, current_a: u128) {
        self.env()
            .events()
            .publish((Symbol::new(self.env(), "stop_ramp_a"),), (current_a,))
    }
}
