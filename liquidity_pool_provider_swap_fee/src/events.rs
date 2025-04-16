use soroban_sdk::{Address, Env, Symbol};

#[derive(Clone)]
pub(crate) struct Events(Env);

impl Events {
    #[inline(always)]
    pub(crate) fn env(&self) -> &Env {
        &self.0
    }

    #[inline(always)]
    pub(crate) fn new(env: &Env) -> Events {
        Events(env.clone())
    }
}

pub(crate) trait ProviderFeeEvents {
    fn charge_provider_fee(&self, token: Address, amount: u128);

    fn claim_fee(&self, token: Address, amount: u128, swapped_to: Address, swapped_amount: u128);

    fn set_swap_fee_fraction(&self, new_swap_fee_fraction: u32);
}

impl ProviderFeeEvents for Events {
    fn charge_provider_fee(&self, token: Address, amount: u128) {
        self.env().events().publish(
            (Symbol::new(self.env(), "charge_provider_fee"),),
            (token, amount),
        );
    }

    fn claim_fee(&self, token: Address, amount: u128, swapped_to: Address, swapped_amount: u128) {
        self.env().events().publish(
            (Symbol::new(self.env(), "withdraw_fee"),),
            (token, amount, swapped_to, swapped_amount),
        );
    }

    fn set_swap_fee_fraction(&self, new_swap_fee_fraction: u32) {
        self.env().events().publish(
            (Symbol::new(self.env(), "set_swap_fee_fraction"),),
            (new_swap_fee_fraction,),
        );
    }
}
