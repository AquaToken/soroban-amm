#![allow(dead_code)]
#![cfg(any(test, feature = "testutils"))]
extern crate std;
use soroban_sdk::{testutils::Address as _, Address, Env};

soroban_sdk::contractimport!(file = "../contracts/soroban_config_storage_contract.wasm");

pub fn deploy_config_storage<'a>(
    e: &Env,
    admin: &Address,
    emergency_admin: &Address,
) -> Client<'a> {
    Client::new(
        e,
        &e.register(WASM, Args::__constructor(admin, emergency_admin)),
    )
}

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,

    pub(crate) admin: Address,
    pub(crate) emergency_admin: Address,
    pub(crate) contract: Client<'a>,
}

impl Default for Setup<'_> {
    fn default() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&env);
        let emergency_admin = Address::generate(&env);
        let contract = deploy_config_storage(&env, &admin, &emergency_admin);

        Setup {
            env,
            admin,
            emergency_admin,
            contract,
        }
    }
}
