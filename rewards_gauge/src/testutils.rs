#![allow(dead_code)]
#![cfg(test)]

use crate::contract::RewardsGaugeArgs;
use crate::{RewardsGauge, RewardsGaugeClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::TokenClient;
use soroban_sdk::{Address, Env};

pub fn create_contract<'a>(
    e: &Env,
    pool: &Address,
    reward_token: &Address,
) -> RewardsGaugeClient<'a> {
    let client = RewardsGaugeClient::new(
        e,
        &e.register(
            RewardsGauge {},
            RewardsGaugeArgs::__constructor(pool, reward_token),
        ),
    );
    client
}

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,

    pub(crate) pool_address: Address,
    pub(crate) reward_token: TokenClient<'a>,
    pub(crate) contract: RewardsGaugeClient<'a>,
}

impl Default for Setup<'_> {
    fn default() -> Self {
        Setup::with_mocked_pool()
    }
}

impl Setup<'_> {
    pub(crate) fn with_mocked_pool() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&env);
        let pool_address = Address::generate(&env);
        let reward_token = TokenClient::new(
            &env,
            &env.register_stellar_asset_contract_v2(admin.clone())
                .address(),
        );
        let contract = create_contract(&env, &pool_address, &reward_token.address);

        Setup {
            env,
            pool_address,
            reward_token,
            contract,
        }
    }
}
