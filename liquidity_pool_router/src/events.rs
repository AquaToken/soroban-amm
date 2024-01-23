use soroban_sdk::{Address, BytesN, Env, Symbol, Val, Vec};

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

pub(crate) trait LiquidityPoolRouterEvents {
    fn deposit(
        &self,
        tokens: Vec<Address>,
        user: Address,
        pool_id: Address,
        amounts: Vec<u128>,
        share_amount: u128,
    );

    fn swap(
        &self,
        tokens: Vec<Address>,
        user: Address,
        pool_id: Address,
        token_in: Address,
        token_out: Address,
        in_amount: u128,
        out_amt: u128,
    );

    fn withdraw(
        &self,
        tokens: Vec<Address>,
        user: Address,
        pool_id: Address,
        amounts: Vec<u128>,
        share_amount: u128,
    );

    fn add_pool(
        &self,
        tokens: Vec<Address>,
        pool_address: Address,
        pool_type: Symbol,
        subpool_salt: BytesN<32>,
        init_args: Vec<Val>,
    );
}

impl LiquidityPoolRouterEvents for Events {
    fn deposit(
        &self,
        tokens: Vec<Address>,
        user: Address,
        pool_id: Address,
        amounts: Vec<u128>,
        share_amount: u128,
    ) {
        self.env().events().publish(
            (Symbol::new(self.env(), "deposit"), tokens, user),
            (pool_id, amounts.clone(), share_amount),
        );
    }

    fn swap(
        &self,
        tokens: Vec<Address>,
        user: Address,
        pool_id: Address,
        token_in: Address,
        token_out: Address,
        in_amount: u128,
        out_amt: u128,
    ) {
        self.env().events().publish(
            (Symbol::new(self.env(), "swap"), tokens, user),
            (pool_id, token_in, token_out, in_amount, out_amt),
        );
    }

    fn withdraw(
        &self,
        tokens: Vec<Address>,
        user: Address,
        pool_id: Address,
        amounts: Vec<u128>,
        share_amount: u128,
    ) {
        self.env().events().publish(
            (Symbol::new(self.env(), "withdraw"), tokens, user),
            (pool_id, share_amount, amounts),
        );
    }

    fn add_pool(
        &self,
        tokens: Vec<Address>,
        pool_address: Address,
        pool_type: Symbol,
        subpool_salt: BytesN<32>,
        init_args: Vec<Val>,
    ) {
        self.env().events().publish(
            (Symbol::new(self.env(), "add_pool"), tokens),
            (pool_address, pool_type, subpool_salt, init_args),
        );
    }
}
