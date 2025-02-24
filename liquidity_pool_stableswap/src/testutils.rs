#![cfg(test)]

use crate::plane::pool_plane;
use crate::plane::pool_plane::Client as PoolPlaneClient;
use crate::LiquidityPoolClient;
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::testutils::arbitrary::std;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{Address, BytesN, Env, IntoVal, Symbol, Vec};
use token_share::token_contract::Client as ShareTokenClient;
use utils::test_utils::jump;

pub(crate) fn create_token_contract<'a>(e: &Env, admin: &Address) -> SorobanTokenClient<'a> {
    SorobanTokenClient::new(
        e,
        &e.register_stellar_asset_contract_v2(admin.clone())
            .address(),
    )
}

pub(crate) fn get_token_admin_client<'a>(
    e: &'a Env,
    address: &'a Address,
) -> SorobanTokenAdminClient<'a> {
    SorobanTokenAdminClient::new(e, address)
}

pub fn create_liqpool_contract<'a>(
    e: &Env,
    admin: &Address,
    router: &Address,
    token_wasm_hash: &BytesN<32>,
    coins: &Vec<Address>,
    a: u128,
    fee: u32,
    reward_token: &Address,
    reward_boost_token: &Address,
    reward_boost_feed: &Address,
    plane: &Address,
) -> LiquidityPoolClient<'a> {
    let liqpool = LiquidityPoolClient::new(e, &e.register(crate::LiquidityPool {}, ()));
    liqpool.initialize_all(
        admin,
        &(
            admin.clone(),
            admin.clone(),
            admin.clone(),
            admin.clone(),
            Vec::from_array(&e, [admin.clone()]),
        ),
        router,
        token_wasm_hash,
        coins,
        &a,
        &fee,
        &(
            reward_token.clone(),
            reward_boost_token.clone(),
            reward_boost_feed.clone(),
        ),
        plane,
    );
    liqpool
}

pub fn install_token_wasm(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

pub fn install_token_wasm_with_decimal<'a>(
    e: &Env,
    admin: &Address,
    decimal: u32,
) -> ShareTokenClient<'a> {
    soroban_sdk::contractimport!(file = "../contracts/soroban_token.wasm");

    let token_client = ShareTokenClient::new(e, &e.register(WASM, ()));
    token_client.initialize(admin, &decimal, &"Token 1".into_val(e), &"TOK".into_val(e));
    token_client
}

pub fn create_plane_contract<'a>(e: &Env) -> PoolPlaneClient<'a> {
    PoolPlaneClient::new(e, &e.register(pool_plane::WASM, ()))
}

mod reward_boost_feed {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_locker_feed_contract.wasm"
    );
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

pub(crate) struct TestConfig {
    pub(crate) a: u128,
    pub(crate) liq_pool_fee: u32,
    pub(crate) reward_token_in_pool: bool,
    pub(crate) token_1_decimals: u32,
    pub(crate) token_2_decimals: u32,
}

impl Default for TestConfig {
    fn default() -> Self {
        TestConfig {
            a: 85,
            liq_pool_fee: 30,
            reward_token_in_pool: false,
            token_1_decimals: 7,
            token_2_decimals: 7,
        }
    }
}

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,

    pub(crate) token1: SorobanTokenClient<'a>,
    pub(crate) token2: SorobanTokenClient<'a>,
    pub(crate) token_reward: SorobanTokenClient<'a>,
    pub(crate) token_share: ShareTokenClient<'a>,
    pub(crate) reward_boost_token: SorobanTokenClient<'a>,
    pub(crate) reward_boost_feed: reward_boost_feed::Client<'a>,

    pub(crate) liq_pool: LiquidityPoolClient<'a>,
    pub(crate) router: Address,
    pub(crate) plane: PoolPlaneClient<'a>,

    pub(crate) admin: Address,
    pub(crate) emergency_admin: Address,
    pub(crate) rewards_admin: Address,
    pub(crate) operations_admin: Address,
    pub(crate) pause_admin: Address,
    pub(crate) emergency_pause_admin: Address,
}

impl Default for Setup<'_> {
    // Create setup from default config and mint tokens for all users & set rewards config
    fn default() -> Self {
        let default_config = TestConfig::default();
        Self::new_with_config(&default_config)
    }
}

impl Setup<'_> {
    // Create setup from config
    pub(crate) fn new_with_config(config: &TestConfig) -> Self {
        let setup = Self::setup(config);
        setup
    }

    // Create token1, token2, reward token, lp token
    pub(crate) fn setup(config: &TestConfig) -> Self {
        let env: Env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&env);
        let rewards_admin = Address::generate(&env);
        let operations_admin = Address::generate(&env);
        let pause_admin = Address::generate(&env);
        let emergency_pause_admin = Address::generate(&env);

        let token1 = SorobanTokenClient::new(
            &env,
            &install_token_wasm_with_decimal(&env, &admin, config.token_1_decimals).address,
        );
        let token2 = SorobanTokenClient::new(
            &env,
            &install_token_wasm_with_decimal(&env, &admin, config.token_2_decimals).address,
        );
        let token_reward = if config.reward_token_in_pool {
            SorobanTokenClient::new(&env, &token1.address.clone())
        } else {
            create_token_contract(&env, &admin)
        };
        let reward_boost_token = create_token_contract(&env, &admin);
        let reward_boost_feed = create_reward_boost_feed_contract(
            &env,
            &admin,
            &operations_admin,
            &emergency_pause_admin,
        );

        let plane = create_plane_contract(&env);

        let router = Address::generate(&env);
        let liq_pool = create_liqpool_contract(
            &env,
            &admin,
            &router,
            &install_token_wasm(&env),
            &Vec::from_array(&env, [token1.address.clone(), token2.address.clone()]),
            config.a,
            config.liq_pool_fee,
            &token_reward.address.clone(),
            &reward_boost_token.address.clone(),
            &reward_boost_feed.address.clone(),
            &plane.address,
        );

        liq_pool.set_privileged_addrs(
            &admin,
            &rewards_admin.clone(),
            &operations_admin.clone(),
            &pause_admin.clone(),
            &Vec::from_array(&env, [emergency_pause_admin.clone()]),
        );

        let emergency_admin = Address::generate(&env);
        liq_pool.commit_transfer_ownership(
            &admin,
            &Symbol::new(&env, "EmergencyAdmin"),
            &emergency_admin,
        );
        jump(&env, ADMIN_ACTIONS_DELAY + 1); // delay is mandatory since emergency admin was set during initialization
        liq_pool.apply_transfer_ownership(&admin, &Symbol::new(&env, "EmergencyAdmin"));

        let token_share = ShareTokenClient::new(&env, &liq_pool.share_id());

        Setup {
            env,
            token1,
            token2,
            token_reward,
            token_share,
            liq_pool,
            router,
            plane,
            admin,
            emergency_admin,
            rewards_admin,
            operations_admin,
            pause_admin,
            emergency_pause_admin,
            reward_boost_token,
            reward_boost_feed,
        }
    }
}
