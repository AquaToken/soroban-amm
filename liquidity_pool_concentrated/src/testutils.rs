#![allow(dead_code)]
#![cfg(test)]

use crate::{ConcentratedLiquidityPool, ConcentratedLiquidityPoolClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{Address, Env, Vec};

mod pool_plane {
    soroban_sdk::contractimport!(file = "../contracts/soroban_liquidity_pool_plane_contract.wasm");
}

mod reward_boost_feed {
    soroban_sdk::contractimport!(file = "../contracts/soroban_locker_feed_contract.wasm");
}

mod rewards_gauge {
    soroban_sdk::contractimport!(file = "../contracts/soroban_rewards_gauge_contract.wasm");
}

fn create_plane_contract<'a>(e: &Env) -> pool_plane::Client<'a> {
    pool_plane::Client::new(e, &e.register(pool_plane::WASM, ()))
}

pub(crate) fn create_reward_boost_feed_contract<'a>(
    e: &Env,
    admin: &Address,
    operations_admin: &Address,
    emergency_admin: &Address,
) -> reward_boost_feed::Client<'a> {
    reward_boost_feed::Client::new(
        e,
        &e.register(
            reward_boost_feed::WASM,
            reward_boost_feed::Args::__constructor(admin, operations_admin, emergency_admin),
        ),
    )
}

pub(crate) fn deploy_rewards_gauge<'a>(
    e: &Env,
    pool: &Address,
    reward_token: &Address,
) -> rewards_gauge::Client<'a> {
    rewards_gauge::Client::new(
        e,
        &e.register(
            rewards_gauge::WASM,
            rewards_gauge::Args::__constructor(pool, reward_token),
        ),
    )
}

pub(crate) fn create_token_contract<'a>(e: &Env, admin: &Address) -> SorobanTokenClient<'a> {
    SorobanTokenClient::new(
        e,
        &e.register_stellar_asset_contract_v2(admin.clone())
            .address(),
    )
}

pub(crate) fn get_token_admin_client<'a>(
    e: &Env,
    address: &Address,
) -> SorobanTokenAdminClient<'a> {
    SorobanTokenAdminClient::new(e, address)
}

pub(crate) fn create_pool_contract<'a>(
    e: &Env,
    admin: &Address,
    router: &Address,
    plane: &Address,
    tokens: &Vec<Address>,
    fee: u32,
    tick_spacing: i32,
) -> ConcentratedLiquidityPoolClient<'a> {
    let client =
        ConcentratedLiquidityPoolClient::new(e, &e.register(ConcentratedLiquidityPool {}, ()));
    client.init_pools_plane(plane);
    client.initialize(
        admin,
        &(
            admin.clone(),
            admin.clone(),
            admin.clone(),
            admin.clone(),
            Vec::from_array(e, [admin.clone()]),
            admin.clone(),
        ),
        router,
        tokens,
        &fee,
        &tick_spacing,
    );
    client
}

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,
    pub(crate) pool: ConcentratedLiquidityPoolClient<'a>,
    pub(crate) plane: Address,
    pub(crate) admin: Address,
    pub(crate) router: Address,
    pub(crate) user: Address,
    pub(crate) token0: SorobanTokenClient<'a>,
    pub(crate) token1: SorobanTokenClient<'a>,
    pub(crate) reward_token: SorobanTokenClient<'a>,
    pub(crate) reward_boost_token: SorobanTokenClient<'a>,
    pub(crate) reward_boost_feed: reward_boost_feed::Client<'a>,
}

impl Default for Setup<'_> {
    fn default() -> Self {
        Self::new()
    }
}

impl Setup<'_> {
    pub(crate) fn new() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&env);
        let router = Address::generate(&env);
        let user = Address::generate(&env);

        let token0 = create_token_contract(&env, &admin);
        let token1 = create_token_contract(&env, &admin);
        let reward_token = create_token_contract(&env, &admin);
        let reward_boost_token = create_token_contract(&env, &admin);
        let reward_boost_feed = create_reward_boost_feed_contract(&env, &admin, &admin, &admin);
        let plane = create_plane_contract(&env);

        let pool = create_pool_contract(
            &env,
            &admin,
            &router,
            &plane.address,
            &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
            30,
            1,
        );

        Self {
            env,
            pool,
            plane: plane.address,
            admin,
            router,
            user,
            token0,
            token1,
            reward_token,
            reward_boost_token,
            reward_boost_feed,
        }
    }

    pub(crate) fn mint_user_tokens(&self, amount0: i128, amount1: i128) {
        get_token_admin_client(&self.env, &self.token0.address).mint(&self.user, &amount0);
        get_token_admin_client(&self.env, &self.token1.address).mint(&self.user, &amount1);
    }
}
