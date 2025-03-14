#![cfg(test)]

use crate::TokenClient;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{contract, contractimpl, Address, Env, IntoVal};

pub fn create_token<'a>(e: &Env, admin: &Address) -> TokenClient<'a> {
    let token = TokenClient::new(e, &e.register(crate::contract::Token {}, ()));
    token.initialize(admin, &7, &"name".into_val(e), &"symbol".into_val(e));
    token
}

#[contract]
pub struct DummyPool;

#[contractimpl]
impl DummyPool {
    pub fn checkpoint_reward(_e: Env, token_contract: Address, _user: Address, _user_shares: u128) {
        token_contract.require_auth();
    }

    pub fn checkpoint_working_balance(
        _e: Env,
        token_contract: Address,
        _user: Address,
        _user_shares: u128,
    ) {
        token_contract.require_auth();
    }
}

pub fn create_dummy_pool<'a>(e: &Env) -> DummyPoolClient<'a> {
    DummyPoolClient::new(e, &e.register(DummyPool {}, ()))
}

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,

    pub(crate) admin: Address,
    pub(crate) token: TokenClient<'a>,
}

impl Default for Setup<'_> {
    // Create setup from default config and mint tokens for all users & set rewards config
    fn default() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&env);
        let token = create_token(&env, &admin);
        Setup { env, admin, token }
    }
}
