use soroban_sdk::{Address, Env, Symbol};

#[derive(Clone)]
pub struct GaugeEvents(Env);

impl GaugeEvents {
    #[inline(always)]
    pub fn env(&self) -> &Env {
        &self.0
    }

    #[inline(always)]
    pub fn new(env: &Env) -> GaugeEvents {
        GaugeEvents(env.clone())
    }

    pub fn add(&self, gauge: Address) {
        self.env()
            .events()
            .publish((Symbol::new(self.env(), "rewards_gauge_add"),), (gauge,))
    }

    pub fn remove(&self, gauge: Address) {
        self.env()
            .events()
            .publish((Symbol::new(self.env(), "rewards_gauge_remove"),), (gauge,))
    }

    pub fn kill_claim(&self) {
        self.env()
            .events()
            .publish((Symbol::new(self.env(), "rewards_gauge_kill_claim"),), ())
    }

    pub fn unkill_claim(&self) {
        self.env()
            .events()
            .publish((Symbol::new(self.env(), "rewards_gauge_unkill_claim"),), ())
    }

    pub fn schedule_reward(
        &self,
        reward_token: Address,
        start_at: u64,
        expired_at: u64,
        tps: u128,
    ) {
        self.env().events().publish(
            (
                Symbol::new(self.env(), "rewards_gauge_schedule_reward"),
                reward_token,
            ),
            (start_at, expired_at, tps),
        )
    }

    pub fn claim(&self, user: Address, reward_token: Address, amount: u128) {
        self.env().events().publish(
            (
                Symbol::new(self.env(), "rewards_gauge_claim"),
                reward_token,
                user,
            ),
            (amount,),
        )
    }
}
