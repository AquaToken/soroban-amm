use soroban_sdk::{Address, BytesN, Env, Symbol};

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

pub(crate) trait FactoryEvents {
    fn deploy(
        &self,
        operator: Address,
        fee_destination: Address,
        max_swap_fee_fraction: u32,
        address: Address,
    );
}

pub(crate) trait FactoryConfigEvents {
    fn set_wasm(&self, new_wasm: BytesN<32>);
}

impl FactoryEvents for Events {
    fn deploy(
        &self,
        operator: Address,
        fee_destination: Address,
        max_swap_fee_fraction: u32,
        address: Address,
    ) {
        self.env().events().publish(
            (Symbol::new(self.env(), "deploy"),),
            (operator, fee_destination, max_swap_fee_fraction, address),
        );
    }
}

impl FactoryConfigEvents for Events {
    fn set_wasm(&self, new_wasm: BytesN<32>) {
        self.env()
            .events()
            .publish((Symbol::new(self.env(), "set_wasm"),), (new_wasm,));
    }
}
