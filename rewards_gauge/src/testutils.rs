#![cfg(test)]

use crate::contract::RewardsGaugeArgs;
use crate::{RewardsGauge, RewardsGaugeClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::TokenClient;
use soroban_sdk::{contract, contractimpl, symbol_short, Address, BytesN, Env};

#[contract]
pub struct MockedPool;

#[contractimpl]
impl MockedPool {
    pub fn set_total_shares(e: Env, shares: u128) {
        e.storage().instance().set(&symbol_short!("total"), &shares);
    }

    pub fn get_total_shares(e: Env) -> u128 {
        e.storage()
            .instance()
            .get(&symbol_short!("total"))
            .unwrap_or_default()
    }
}

pub fn install_dummy_wasm<'a>(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(file = "../contracts/dummy_contract.wasm");
    e.deployer().upload_contract_wasm(WASM)
}

pub fn create_contract<'a>(
    e: &Env,
    pool: &Address,
    operator: &Address,
    reward_token: &Address,
) -> RewardsGaugeClient<'a> {
    let client = RewardsGaugeClient::new(
        e,
        &e.register(
            RewardsGauge {},
            RewardsGaugeArgs::__constructor(pool, operator, reward_token),
        ),
    );
    client
}

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,

    pub(crate) admin: Address,
    pub(crate) operator: Address,
    pub(crate) pool_address: Address,
    pub(crate) reward_token: TokenClient<'a>,
    pub(crate) contract: RewardsGaugeClient<'a>,
}

impl Setup<'_> {
    pub(crate) fn with_mocked_pool() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&env);
        let operator = Address::generate(&env);
        let pool_address = env.register(MockedPool, ());
        let reward_token = TokenClient::new(
            &env,
            &env.register_stellar_asset_contract_v2(admin.clone())
                .address(),
        );
        let contract = create_contract(&env, &pool_address, &operator, &reward_token.address);

        Setup {
            env,
            admin,
            operator,
            pool_address,
            reward_token,
            contract,
        }
    }
}
