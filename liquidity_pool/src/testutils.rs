#![cfg(test)]
extern crate std;
use crate::plane::{pool_plane, PoolPlaneClient};
use crate::LiquidityPoolClient;
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{
    testutils::{Address as _, Ledger, LedgerInfo},
    Address, BytesN, Env, Vec,
};
use std::vec;
use token_share::token_contract::{Client as ShareTokenClient, WASM};

pub(crate) struct TestConfig {
    pub(crate) users_count: u32,
    pub(crate) mint_to_user: i128,
    pub(crate) rewards_count: i128,
    pub(crate) liq_pool_fee: u32,
    pub(crate) reward_tps: u128,
}

impl Default for TestConfig {
    fn default() -> Self {
        TestConfig {
            users_count: 2,
            mint_to_user: 1000,
            rewards_count: 1_000_000_0000000,
            liq_pool_fee: 30,
            reward_tps: 10_5000000_u128,
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
    pub(crate) token_share: ShareTokenClient<'a>,
    pub(crate) liq_pool: LiquidityPoolClient<'a>,
    pub(crate) plane: PoolPlaneClient<'a>,
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
        e.budget().reset_unlimited();

        let users = Self::generate_random_users(&e, config.users_count);

        let mut token_admin1 = Address::generate(&e);
        let mut token_admin2 = Address::generate(&e);

        let mut token1 = create_token_contract(&e, &token_admin1);
        let mut token2 = create_token_contract(&e, &token_admin2);
        let token_reward = create_token_contract(&e, &token_admin1);

        let plane = create_plane_contract(&e);

        if &token2.address < &token1.address {
            std::mem::swap(&mut token1, &mut token2);
            std::mem::swap(&mut token_admin1, &mut token_admin2);
        }
        let token1_admin_client = get_token_admin_client(&e, &token1.address.clone());
        let token2_admin_client = get_token_admin_client(&e, &token2.address.clone());
        let token_reward_admin_client = get_token_admin_client(&e, &token_reward.address.clone());

        let router = Address::generate(&e);

        let liq_pool = create_liqpool_contract(
            &e,
            &users[0],
            &users[0],
            &router,
            &install_token_wasm(&e),
            &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
            &token_reward.address,
            config.liq_pool_fee,
            &plane.address,
        );
        token_reward_admin_client.mint(&liq_pool.address, &config.rewards_count);

        let token_share = ShareTokenClient::new(&e, &liq_pool.share_id());

        Self {
            env: e,
            router,
            users,
            token1,
            token1_admin_client,
            token2,
            token2_admin_client,
            token_reward,
            token_reward_admin_client,
            token_share,
            liq_pool,
            plane,
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
    PoolPlaneClient::new(e, &e.register_contract_wasm(None, pool_plane::WASM))
}

pub fn create_liqpool_contract<'a>(
    e: &Env,
    admin: &Address,
    operator: &Address,
    router: &Address,
    token_wasm_hash: &BytesN<32>,
    tokens: &Vec<Address>,
    token_reward: &Address,
    fee_fraction: u32,
    plane: &Address,
) -> LiquidityPoolClient<'a> {
    let liqpool = LiquidityPoolClient::new(e, &e.register_contract(None, crate::LiquidityPool {}));
    liqpool.initialize_all(
        admin,
        operator,
        router,
        token_wasm_hash,
        tokens,
        &fee_fraction,
        token_reward,
        plane,
    );
    liqpool
}

pub fn install_token_wasm(e: &Env) -> BytesN<32> {
    e.deployer().upload_contract_wasm(WASM)
}

pub fn jump(e: &Env, time: u64) {
    e.ledger().set(LedgerInfo {
        timestamp: e.ledger().timestamp().saturating_add(time),
        protocol_version: e.ledger().protocol_version(),
        sequence_number: e.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 999999,
        min_persistent_entry_ttl: 999999,
        max_entry_ttl: u32::MAX,
    });
}

#[test]
fn test() {
    let config = TestConfig {
        users_count: 2,
        mint_to_user: 1000,
        rewards_count: 1_000_000_0000000,
        liq_pool_fee: 30,
        reward_tps: 10_5000000_u128,
    };
    let _setup = Setup::new_with_config(&config);
}
