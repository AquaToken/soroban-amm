#![cfg(test)]
extern crate std;
use crate::plane::{pool_plane, PoolPlaneClient};
use crate::LiquidityPoolClient;
use access_control::constants::ADMIN_ACTIONS_DELAY;
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Symbol, Vec};
use std::vec;
use token_share::token_contract::{Client as ShareTokenClient, WASM};
use utils::test_utils::jump;

pub(crate) struct TestConfig {
    pub(crate) users_count: u32,
    pub(crate) mint_to_user: i128,
    pub(crate) rewards_count: i128,
    pub(crate) liq_pool_fee: u32,
    pub(crate) reward_tps: u128,
    pub(crate) reward_token_in_pool: bool,
}

impl Default for TestConfig {
    fn default() -> Self {
        TestConfig {
            users_count: 2,
            mint_to_user: 1000,
            rewards_count: 1_000_000_0000000,
            liq_pool_fee: 30,
            reward_tps: 10_5000000_u128,
            reward_token_in_pool: false,
        }
    }
}

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,
    pub(crate) router: Address,
    pub(crate) users: vec::Vec<Address>,
    pub(crate) token1: SorobanTokenClient<'a>,
    pub(crate) token1_admin_client: SorobanTokenAdminClient<'a>,
    pub(crate) token2: SorobanTokenClient<'a>,
    pub(crate) token2_admin_client: SorobanTokenAdminClient<'a>,
    pub(crate) token_reward: SorobanTokenClient<'a>,
    pub(crate) token_reward_admin_client: SorobanTokenAdminClient<'a>,
    pub(crate) reward_boost_token: SorobanTokenClient<'a>,
    pub(crate) reward_boost_feed: reward_boost_feed::Client<'a>,
    pub(crate) token_share: ShareTokenClient<'a>,
    pub(crate) liq_pool: LiquidityPoolClient<'a>,
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
    // Create setup from config and mint tokens for all users
    pub(crate) fn new_with_config(config: &TestConfig) -> Self {
        let setup = Self::setup(config);
        setup.mint_tokens_for_users(config.mint_to_user);
        setup.set_rewards_config(config.reward_tps);
        setup
    }

    // Create users, token1, token2, reward token, lp token
    //
    // Mint reward token (1_000_000_0000000) & approve for liquidity_pool token
    pub(crate) fn setup(config: &TestConfig) -> Self {
        let e: Env = Env::default();
        e.mock_all_auths();
        e.cost_estimate().budget().reset_unlimited();

        let users = Self::generate_random_users(&e, config.users_count);
        let admin = users[0].clone();
        let rewards_admin = Address::generate(&e);
        let operations_admin = Address::generate(&e);
        let pause_admin = Address::generate(&e);
        let emergency_pause_admin = Address::generate(&e);

        let mut token1 = create_token_contract(&e, &admin);
        let mut token2 = create_token_contract(&e, &admin);
        let reward_token = if config.reward_token_in_pool {
            SorobanTokenClient::new(&e, &token1.address.clone())
        } else {
            create_token_contract(&e, &admin)
        };
        let reward_boost_token = create_token_contract(&e, &admin);
        let reward_boost_feed = create_reward_boost_feed_contract(
            &e,
            &admin,
            &operations_admin,
            &emergency_pause_admin,
        );

        let plane = create_plane_contract(&e);

        if &token2.address < &token1.address {
            std::mem::swap(&mut token1, &mut token2);
        }
        let token1_admin_client = get_token_admin_client(&e, &token1.address.clone());
        let token2_admin_client = get_token_admin_client(&e, &token2.address.clone());
        let token_reward_admin_client = get_token_admin_client(&e, &reward_token.address.clone());

        let router = Address::generate(&e);

        let liq_pool = create_liqpool_contract(
            &e,
            &admin,
            &router,
            &install_token_wasm(&e),
            &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
            &reward_token.address,
            &reward_boost_token.address,
            &reward_boost_feed.address,
            config.liq_pool_fee,
            &plane.address,
        );
        token_reward_admin_client.mint(&liq_pool.address, &config.rewards_count);

        liq_pool.set_privileged_addrs(
            &admin,
            &rewards_admin.clone(),
            &operations_admin.clone(),
            &pause_admin.clone(),
            &Vec::from_array(&e, [emergency_pause_admin.clone()]),
        );

        let emergency_admin = Address::generate(&e);
        liq_pool.commit_transfer_ownership(
            &admin,
            &Symbol::new(&e, "EmergencyAdmin"),
            &emergency_admin,
        );
        jump(&e, ADMIN_ACTIONS_DELAY + 1); // delay is mandatory since emergency admin was set during initialization
        liq_pool.apply_transfer_ownership(&admin, &Symbol::new(&e, "EmergencyAdmin"));

        let token_share = ShareTokenClient::new(&e, &liq_pool.share_id());

        Self {
            env: e,
            router,
            users,
            token1,
            token1_admin_client,
            token2,
            token2_admin_client,
            token_reward: reward_token,
            token_reward_admin_client,
            token_share,
            liq_pool: liq_pool,
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

    pub(crate) fn generate_random_users(e: &Env, users_count: u32) -> vec::Vec<Address> {
        let mut users = vec![];
        for _c in 0..users_count {
            users.push(Address::generate(e));
        }
        users
    }

    pub(crate) fn mint_tokens_for_users(&self, amount: i128) {
        for user in self.users.iter() {
            self.token1_admin_client.mint(user, &amount);
            assert_eq!(self.token1.balance(user), amount.clone());

            self.token2_admin_client.mint(user, &amount);
            assert_eq!(self.token2.balance(user), amount.clone());
        }
    }

    pub(crate) fn set_rewards_config(&self, reward_tps: u128) {
        if reward_tps > 0 {
            self.liq_pool.set_rewards_config(
                &self.users[0],
                &self.env.ledger().timestamp().saturating_add(60),
                &reward_tps,
            );
        }
    }
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

pub(crate) fn create_plane_contract<'a>(e: &Env) -> PoolPlaneClient<'a> {
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

pub fn create_liqpool_contract<'a>(
    e: &Env,
    admin: &Address,
    router: &Address,
    token_wasm_hash: &BytesN<32>,
    tokens: &Vec<Address>,
    reward_token: &Address,
    reward_boost_token: &Address,
    reward_boost_feed: &Address,
    fee_fraction: u32,
    plane: &Address,
) -> LiquidityPoolClient<'a> {
    let liqpool = LiquidityPoolClient::new(e, &e.register(crate::LiquidityPool {}, ()));
    liqpool.initialize_all(
        &admin,
        &(
            admin.clone(),
            admin.clone(),
            admin.clone(),
            admin.clone(),
            Vec::from_array(e, [admin.clone()]),
        ),
        router,
        token_wasm_hash,
        tokens,
        &fee_fraction,
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
    e.deployer().upload_contract_wasm(WASM)
}

#[test]
fn test() {
    let config = TestConfig {
        users_count: 2,
        mint_to_user: 1000,
        rewards_count: 1_000_000_0000000,
        liq_pool_fee: 30,
        reward_tps: 10_5000000_u128,
        reward_token_in_pool: false,
    };
    let _setup = Setup::new_with_config(&config);
}
