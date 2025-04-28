#![cfg(test)]
extern crate std;
use crate::contracts;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{Address, BytesN, Env, Vec};

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,
    pub(crate) admin: Address,
    pub(crate) operator: Address,
    pub(crate) emergency_admin: Address,
    pub(crate) fee_collector_factory: contracts::swap_fee_factory::Client<'a>,
    pub(crate) router: contracts::router::Client<'a>,
    pub(crate) fee_destination: Address,
    pub(crate) reward_token: Address,
    pub(crate) locked_token: Address,
}

impl Default for Setup<'_> {
    fn default() -> Self {
        Self::setup()
    }
}

impl Setup<'_> {
    pub(crate) fn setup() -> Self {
        let e: Env = Env::default();
        e.mock_all_auths();
        e.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&e);
        let operator = Address::generate(&e);
        let emergency_admin = Address::generate(&e);
        let fee_destination = Address::generate(&e);

        let reward_token = create_token_contract(&e, &admin);
        let locked_token = create_token_contract(&e, &admin);
        let locked_token_admin = get_token_admin_client(&e, &locked_token.address);

        // init boost feed
        let boost_feed = create_reward_boost_feed_contract(&e, &admin, &operator, &emergency_admin);
        locked_token_admin.mint(&admin, &53_000_000_000_0000000);
        boost_feed.set_total_supply(&operator, &53_000_000_000_0000000);

        // init swap router
        let pool_hash = e
            .deployer()
            .upload_contract_wasm(contracts::constant_product_pool::WASM);
        let token_hash = e.deployer().upload_contract_wasm(contracts::lp_token::WASM);
        let plane = deploy_plane_contract(&e);

        let router = deploy_liqpool_router_contract(e.clone());
        router.init_admin(&admin);
        router.set_pool_hash(&admin, &pool_hash);
        router.set_stableswap_pool_hash(
            &admin,
            &e.deployer()
                .upload_contract_wasm(contracts::stableswap_pool::WASM),
        );
        router.set_token_hash(&admin, &token_hash);
        router.set_reward_token(&admin, &reward_token.address);
        router.set_pools_plane(&admin, &plane.address);
        router.configure_init_pool_payment(
            &admin,
            &reward_token.address,
            &10_0000000,
            &1_0000000,
            &router.address,
        );
        router.set_reward_boost_config(&admin, &locked_token.address, &boost_feed.address);

        let fee_collector_factory =
            deploy_provider_swap_fee_factory(&e, &admin, &emergency_admin, &router.address);

        Self {
            env: e,
            admin,
            operator,
            emergency_admin,
            fee_destination,
            fee_collector_factory,
            router,
            reward_token: reward_token.address,
            locked_token: locked_token.address,
        }
    }

    pub(crate) fn deploy_standard_pool(
        &self,
        token_a: &Address,
        token_b: &Address,
        fee_fraction: u32,
    ) -> (contracts::constant_product_pool::Client, BytesN<32>) {
        get_token_admin_client(&self.env, &self.reward_token).mint(&self.admin, &10_0000000);
        let (pool_hash, pool_address) = self.router.init_standard_pool(
            &self.admin,
            &Vec::from_array(&self.env, [token_a.clone(), token_b.clone()]),
            &fee_fraction,
        );
        (
            contracts::constant_product_pool::Client::new(&self.env, &pool_address),
            pool_hash,
        )
    }

    pub(crate) fn deploy_stableswap_pool(
        &self,
        token_a: &Address,
        token_b: &Address,
        fee_fraction: u32,
    ) -> (contracts::stableswap_pool::Client, BytesN<32>) {
        get_token_admin_client(&self.env, &self.reward_token).mint(&self.admin, &1_0000000);
        let (pool_hash, pool_address) = self.router.init_stableswap_pool(
            &self.admin,
            &Vec::from_array(&self.env, [token_a.clone(), token_b.clone()]),
            &fee_fraction,
        );
        (
            contracts::stableswap_pool::Client::new(&self.env, &pool_address),
            pool_hash,
        )
    }

    pub(crate) fn deploy_swap_fee_contract(
        &self,
        operator: &Address,
        fee_destination: &Address,
        max_fee_fraction: u32,
    ) -> contracts::swap_fee::Client {
        contracts::swap_fee::Client::new(
            &self.env,
            &self.fee_collector_factory.deploy_swap_fee_contract(
                &operator,
                &fee_destination,
                &max_fee_fraction,
            ),
        )
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

pub fn deploy_provider_swap_fee_factory<'a>(
    e: &Env,
    admin: &Address,
    emergency_admin: &Address,
    router: &Address,
) -> contracts::swap_fee_factory::Client<'a> {
    let swap_fee_wasm = e.deployer().upload_contract_wasm(contracts::swap_fee::WASM);
    contracts::swap_fee_factory::Client::new(
        e,
        &e.register(
            contracts::swap_fee_factory::WASM,
            (admin, emergency_admin, router, swap_fee_wasm),
        ),
    )
}

fn deploy_liqpool_router_contract<'a>(e: Env) -> contracts::router::Client<'a> {
    contracts::router::Client::new(&e, &e.register(contracts::router::WASM, ()))
}

fn deploy_plane_contract<'a>(e: &Env) -> contracts::pool_plane::Client {
    contracts::pool_plane::Client::new(e, &e.register(contracts::pool_plane::WASM, ()))
}

pub(crate) fn create_reward_boost_feed_contract<'a>(
    e: &Env,
    admin: &Address,
    operations_admin: &Address,
    emergency_admin: &Address,
) -> contracts::boost_feed::Client<'a> {
    contracts::boost_feed::Client::new(
        e,
        &e.register(
            contracts::boost_feed::WASM,
            contracts::boost_feed::Args::__constructor(admin, operations_admin, emergency_admin),
        ),
    )
}
