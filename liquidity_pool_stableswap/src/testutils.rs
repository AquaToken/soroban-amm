#![cfg(test)]

use crate::plane::pool_plane;
use crate::plane::pool_plane::Client as PoolPlaneClient;
use crate::LiquidityPoolClient;
use soroban_sdk::testutils::arbitrary::std;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{Address, BytesN, Env, Vec};
use token_share::token_contract::Client as ShareTokenClient;

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
    token_reward: &Address,
    plane: &Address,
) -> LiquidityPoolClient<'a> {
    let liqpool = LiquidityPoolClient::new(e, &e.register_contract(None, crate::LiquidityPool {}));
    liqpool.initialize_all(
        admin,
        &(
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
        token_reward,
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

pub fn create_plane_contract<'a>(e: &Env) -> PoolPlaneClient<'a> {
    PoolPlaneClient::new(e, &e.register_contract_wasm(None, pool_plane::WASM))
}

pub(crate) struct TestConfig {
    pub(crate) a: u128,
    pub(crate) liq_pool_fee: u32,
}

impl Default for TestConfig {
    fn default() -> Self {
        TestConfig {
            a: 85,
            liq_pool_fee: 30,
        }
    }
}

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,

    pub(crate) token1: SorobanTokenClient<'a>,
    pub(crate) token2: SorobanTokenClient<'a>,
    pub(crate) token_reward: SorobanTokenClient<'a>,
    pub(crate) token_share: ShareTokenClient<'a>,

    pub(crate) liq_pool: LiquidityPoolClient<'a>,
    pub(crate) router: Address,
    pub(crate) plane: PoolPlaneClient<'a>,

    pub(crate) admin: Address,
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
        env.budget().reset_unlimited();

        let mut token_admin1 = Address::generate(&env);
        let mut token_admin2 = Address::generate(&env);

        let mut token1 = create_token_contract(&env, &token_admin1);
        let mut token2 = create_token_contract(&env, &token_admin2);
        let token_reward = create_token_contract(&env, &token_admin1);

        let plane = create_plane_contract(&env);

        if &token2.address < &token1.address {
            std::mem::swap(&mut token1, &mut token2);
            std::mem::swap(&mut token_admin1, &mut token_admin2);
        }

        let router = Address::generate(&env);

        let admin = Address::generate(&env);
        let liq_pool = create_liqpool_contract(
            &env,
            &admin,
            &router,
            &install_token_wasm(&env),
            &Vec::from_array(&env, [token1.address.clone(), token2.address.clone()]),
            config.a,
            config.liq_pool_fee,
            &token_reward.address.clone(),
            &plane.address,
        );

        let rewards_admin = Address::generate(&env);
        let operations_admin = Address::generate(&env);
        let pause_admin = Address::generate(&env);
        let emergency_pause_admin = Address::generate(&env);
        liq_pool.set_privileged_addrs(
            &admin,
            &rewards_admin.clone(),
            &operations_admin.clone(),
            &pause_admin.clone(),
            &Vec::from_array(&env, [emergency_pause_admin.clone()]),
        );

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
            rewards_admin,
            operations_admin,
            pause_admin,
            emergency_pause_admin,
        }
    }
}