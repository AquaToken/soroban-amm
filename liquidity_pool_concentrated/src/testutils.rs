#![allow(dead_code)]
#![cfg(test)]

use crate::{ConcentratedLiquidityPool, ConcentratedLiquidityPoolClient};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{Address, Env, Vec};
use utils::test_utils::{count_events, event_data, event_topic_as_address, get_events_by_name};

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
    pub(crate) emergency_admin: Address,
    pub(crate) rewards_admin: Address,
    pub(crate) operations_admin: Address,
    pub(crate) pause_admin: Address,
    pub(crate) emergency_pause_admin: Address,
    pub(crate) system_fee_admin: Address,
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
        let emergency_admin = Address::generate(&env);
        let rewards_admin = Address::generate(&env);
        let operations_admin = Address::generate(&env);
        let pause_admin = Address::generate(&env);
        let emergency_pause_admin = Address::generate(&env);
        let system_fee_admin = Address::generate(&env);
        let router = Address::generate(&env);
        let user = Address::generate(&env);

        let token_a = create_token_contract(&env, &admin);
        let token_b = create_token_contract(&env, &admin);
        let (token0, token1) = if token_a.address < token_b.address {
            (token_a, token_b)
        } else {
            (token_b, token_a)
        };
        let reward_token = create_token_contract(&env, &admin);
        let reward_boost_token = create_token_contract(&env, &admin);
        let reward_boost_feed =
            create_reward_boost_feed_contract(&env, &admin, &operations_admin, &emergency_admin);
        let plane = create_plane_contract(&env);

        let client = ConcentratedLiquidityPoolClient::new(
            &env,
            &env.register(ConcentratedLiquidityPool {}, ()),
        );
        client.init_pools_plane(&plane.address);
        client.initialize(
            &admin,
            &(
                emergency_admin.clone(),
                rewards_admin.clone(),
                operations_admin.clone(),
                pause_admin.clone(),
                Vec::from_array(&env, [emergency_pause_admin.clone()]),
                system_fee_admin.clone(),
            ),
            &router,
            &Vec::from_array(&env, [token0.address.clone(), token1.address.clone()]),
            &30,
            &1,
        );

        Self {
            env,
            pool: client,
            plane: plane.address,
            admin,
            emergency_admin,
            rewards_admin,
            operations_admin,
            pause_admin,
            emergency_pause_admin,
            system_fee_admin,
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

// ── claim_fees event helpers ────────────────────────────────────────────

pub(crate) fn count_claim_fees_events(env: &Env, contract: &Address) -> usize {
    count_events(env, contract, "claim_fees")
}

pub(crate) fn assert_claim_fees_event(
    env: &Env,
    contract: &Address,
    owner: &Address,
    token0: &Address,
    token1: &Address,
    amount0: u128,
    amount1: u128,
) {
    let events = get_events_by_name(env, contract, "claim_fees");
    assert!(!events.is_empty(), "expected at least one claim_fees event");

    let event = &events[0];
    assert_eq!(
        &event_topic_as_address(env, event, 1),
        owner,
        "owner mismatch"
    );
    assert_eq!(
        &event_topic_as_address(env, event, 2),
        token0,
        "token0 mismatch"
    );
    assert_eq!(
        &event_topic_as_address(env, event, 3),
        token1,
        "token1 mismatch"
    );

    let (actual0, actual1): (i128, i128) = event_data(env, event);
    assert_eq!(actual0, amount0 as i128, "amount0 mismatch");
    assert_eq!(actual1, amount1 as i128, "amount1 mismatch");
}
