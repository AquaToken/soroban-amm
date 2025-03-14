#![cfg(test)]
extern crate std;

use crate::LiquidityPoolRouterClient;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::{Address, BytesN, Env, Symbol, Vec};

pub(crate) mod test_token {
    use soroban_sdk::contractimport;
    contractimport!(file = "../target/wasm32-unknown-unknown/release/soroban_token_contract.wasm");
}

pub fn create_token_contract<'a>(e: &Env, admin: &Address) -> test_token::Client<'a> {
    test_token::Client::new(
        e,
        &e.register_stellar_asset_contract_v2(admin.clone())
            .address(),
    )
}

pub fn create_liqpool_router_contract<'a>(e: &Env) -> LiquidityPoolRouterClient<'a> {
    let router = LiquidityPoolRouterClient::new(e, &e.register(crate::LiquidityPoolRouter {}, ()));
    router
}

pub fn install_token_wasm(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

pub mod standard_pool {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_contract.wasm"
    );
}

pub fn install_liq_pool_hash(e: &Env) -> BytesN<32> {
    e.deployer().upload_contract_wasm(standard_pool::WASM)
}

pub mod stableswap_pool {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_stableswap_contract.wasm"
    );
}

pub fn install_stableswap_liq_pool_hash(e: &Env) -> BytesN<32> {
    e.deployer().upload_contract_wasm(stableswap_pool::WASM)
}

mod pool_plane {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_plane_contract.wasm"
    );
}

pub fn create_plane_contract<'a>(e: &Env) -> pool_plane::Client<'a> {
    pool_plane::Client::new(e, &e.register(pool_plane::WASM, ()))
}

mod liquidity_calculator {
    soroban_sdk::contractimport!(
        file =
            "../target/wasm32-unknown-unknown/release/soroban_liquidity_pool_liquidity_calculator_contract.wasm"
    );
}

pub fn create_liquidity_calculator_contract<'a>(e: &Env) -> liquidity_calculator::Client<'a> {
    liquidity_calculator::Client::new(e, &e.register(liquidity_calculator::WASM, ()))
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

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,

    pub(crate) admin: Address,

    pub(crate) tokens: [test_token::Client<'a>; 4],
    pub(crate) reward_token: test_token::Client<'a>,
    pub(crate) reward_boost_token: test_token::Client<'a>,
    pub(crate) reward_boost_feed: reward_boost_feed::Client<'a>,

    pub(crate) router: LiquidityPoolRouterClient<'a>,

    pub(crate) emergency_admin: Address,
    pub(crate) rewards_admin: Address,
    pub(crate) operations_admin: Address,
    pub(crate) pause_admin: Address,
    pub(crate) emergency_pause_admin: Address,
}

impl Default for Setup<'_> {
    // Create setup from default config and mint tokens for all users & set rewards config
    fn default() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&env);

        let mut tokens = std::vec![
            create_token_contract(&env, &admin).address,
            create_token_contract(&env, &admin).address,
            create_token_contract(&env, &admin).address,
            create_token_contract(&env, &admin).address,
        ];
        tokens.sort();
        let tokens = [
            test_token::Client::new(&env, &tokens[0]),
            test_token::Client::new(&env, &tokens[1]),
            test_token::Client::new(&env, &tokens[2]),
            test_token::Client::new(&env, &tokens[3]),
        ];

        let reward_admin = Address::generate(&env);
        let admin = Address::generate(&env);
        let payment_for_creation_address = Address::generate(&env);

        let reward_token = create_token_contract(&env, &reward_admin);
        let reward_boost_token = create_token_contract(&env, &reward_admin);

        let pool_hash = install_liq_pool_hash(&env);
        let token_hash = install_token_wasm(&env);
        let router = create_liqpool_router_contract(&env);
        router.init_admin(&admin);
        let rewards_admin = soroban_sdk::Address::generate(&env);
        let operations_admin = soroban_sdk::Address::generate(&env);
        let pause_admin = soroban_sdk::Address::generate(&env);
        let emergency_pause_admin = soroban_sdk::Address::generate(&env);
        let reward_boost_feed = create_reward_boost_feed_contract(
            &env,
            &admin,
            &operations_admin,
            &emergency_pause_admin,
        );
        router.set_privileged_addrs(
            &admin,
            &rewards_admin,
            &operations_admin,
            &pause_admin,
            &Vec::from_array(&env, [emergency_pause_admin.clone()]),
        );
        router.set_pool_hash(&admin, &pool_hash);
        router.set_stableswap_pool_hash(&admin, &install_stableswap_liq_pool_hash(&env));
        router.set_token_hash(&admin, &token_hash);
        router.set_reward_token(&admin, &reward_token.address);
        router.configure_init_pool_payment(
            &admin,
            &reward_token.address,
            &1_0000000,
            &1_0000000,
            &payment_for_creation_address,
        );
        router.set_reward_boost_config(
            &admin,
            &reward_boost_token.address,
            &reward_boost_feed.address,
        );

        let emergency_admin = Address::generate(&env);
        router.commit_transfer_ownership(
            &admin,
            &Symbol::new(&env, "EmergencyAdmin"),
            &emergency_admin,
        );
        router.apply_transfer_ownership(&admin, &Symbol::new(&env, "EmergencyAdmin"));

        let plane = create_plane_contract(&env);
        router.set_pools_plane(&admin, &plane.address);

        let liquidity_calculator = create_liquidity_calculator_contract(&env);
        liquidity_calculator.init_admin(&admin);
        liquidity_calculator.set_pools_plane(&admin, &plane.address);
        router.set_liquidity_calculator(&admin, &liquidity_calculator.address);

        Setup {
            env,
            admin,
            tokens,
            reward_token,
            router,
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
