#![cfg(test)]

use crate::contract::ConfigStorageArgs;
use crate::{ConfigStorage, ConfigStorageClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, Env};

pub fn create_contract<'a>(
    e: &Env,
    admin: &Address,
    emergency_admin: &Address,
) -> ConfigStorageClient<'a> {
    let client = ConfigStorageClient::new(
        e,
        &e.register(
            ConfigStorage {},
            ConfigStorageArgs::__constructor(admin, emergency_admin),
        ),
    );
    client
}

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,

    pub(crate) admin: Address,
    pub(crate) emergency_admin: Address,
    pub(crate) contract: ConfigStorageClient<'a>,
}

impl Default for Setup<'_> {
    fn default() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&env);
        let emergency_admin = Address::generate(&env);
        let contract = create_contract(&env, &admin, &emergency_admin);

        Setup {
            env,
            admin,
            emergency_admin,
            contract,
        }
    }
}
