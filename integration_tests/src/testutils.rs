#![allow(dead_code)]
#![cfg(test)]
extern crate std;
use crate::contracts;
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{Address, BytesN, Env, Vec};

/// Soroban cost model coefficients for VM memory (from soroban-env-host 25.0.1).
/// ScaledU64 values are divided by 128 (COST_MODEL_LIN_TERM_SCALE_BITS = 7).
pub(crate) struct WasmStats {
    wasm_bytes: u64,
    instructions: u64,
    functions: u64,
    globals: u64,
    table_entries: u64,
    types: u64,
    data_segments: u64,
    elem_segments: u64,
    imports: u64,
    exports: u64,
    data_segment_bytes: u64,
}

impl WasmStats {
    /// Memory cost of first-time VmInstantiation: const=130065 + lin=5064/128 * wasm_bytes
    fn vm_instantiation_mem(&self) -> u64 {
        130065 + 5064 * self.wasm_bytes / 128
    }

    /// Memory cost of cached VmCachedInstantiation: const=69472 + lin=1217/128 * wasm_bytes
    fn vm_cached_instantiation_mem(&self) -> u64 {
        69472 + 1217 * self.wasm_bytes / 128
    }

    /// Memory from ParseWasm* phases (first parse only)
    fn parse_wasm_mem(&self) -> u64 {
        // ParseWasmInstructions: const=17564 + lin=6457/128 * instructions
        let m = 17564 + 6457 * self.instructions / 128;
        // ParseWasmFunctions: lin=47464/128 * functions
        let m = m + 47464 * self.functions / 128;
        // ParseWasmGlobals: lin=13420/128 * globals
        let m = m + 13420 * self.globals / 128;
        // ParseWasmTableEntries: lin=6285/128 * table_entries
        let m = m + 6285 * self.table_entries / 128;
        // ParseWasmTypes: lin=64670/128 * types
        let m = m + 64670 * self.types / 128;
        // ParseWasmDataSegments: lin=29074/128 * data_segments
        let m = m + 29074 * self.data_segments / 128;
        // ParseWasmElemSegments: lin=48095/128 * elem_segments
        let m = m + 48095 * self.elem_segments / 128;
        // ParseWasmImports: lin=103229/128 * imports
        let m = m + 103229 * self.imports / 128;
        // ParseWasmExports: lin=36394/128 * exports
        let m = m + 36394 * self.exports / 128;
        // ParseWasmDataSegmentBytes: lin=257/128 * data_segment_bytes
        m + 257 * self.data_segment_bytes / 128
    }

    /// Memory from InstantiateWasm* phases (per cached instantiation)
    fn instantiate_wasm_mem(&self) -> u64 {
        // InstantiateWasmInstructions: const=70704
        let m = 70704u64;
        // InstantiateWasmFunctions: lin=14613/128 * functions
        let m = m + 14613 * self.functions / 128;
        // InstantiateWasmGlobals: lin=6833/128 * globals
        let m = m + 6833 * self.globals / 128;
        // InstantiateWasmTableEntries: lin=1025/128 * table_entries
        let m = m + 1025 * self.table_entries / 128;
        // InstantiateWasmTypes: 0
        // InstantiateWasmDataSegments: lin=129632/128 * data_segments
        let m = m + 129632 * self.data_segments / 128;
        // InstantiateWasmElemSegments: lin=13665/128 * elem_segments
        let m = m + 13665 * self.elem_segments / 128;
        // InstantiateWasmImports: lin=97637/128 * imports
        let m = m + 97637 * self.imports / 128;
        // InstantiateWasmExports: lin=9176/128 * exports
        let m = m + 9176 * self.exports / 128;
        // InstantiateWasmDataSegmentBytes: lin=126/128 * data_segment_bytes
        m + 126 * self.data_segment_bytes / 128
    }

    /// Total memory for first instantiation (parse + instantiate + VM)
    fn first_call_mem(&self) -> u64 {
        self.vm_instantiation_mem() + self.parse_wasm_mem() + self.instantiate_wasm_mem()
    }

    /// Total memory for subsequent cached instantiation
    fn cached_call_mem(&self) -> u64 {
        self.vm_cached_instantiation_mem() + self.instantiate_wasm_mem()
    }
}

// master contract stats (from wasm-objdump)
pub(crate) const ROUTER_MASTER: WasmStats = WasmStats {
    wasm_bytes: 42474,
    instructions: 207,
    functions: 207,
    globals: 4,
    table_entries: 0,
    types: 38,
    data_segments: 22,
    elem_segments: 0,
    imports: 55,
    exports: 76,
    data_segment_bytes: 1982,
};
pub(crate) const POOL_MASTER: WasmStats = WasmStats {
    wasm_bytes: 54839,
    instructions: 259,
    functions: 259,
    globals: 4,
    table_entries: 0,
    types: 46,
    data_segments: 7,
    elem_segments: 0,
    imports: 59,
    exports: 80,
    data_segment_bytes: 1721,
};
pub(crate) const TOKEN_MASTER: WasmStats = WasmStats {
    wasm_bytes: 10677,
    instructions: 70,
    functions: 70,
    globals: 4,
    table_entries: 0,
    types: 25,
    data_segments: 1,
    elem_segments: 0,
    imports: 26,
    exports: 23,
    data_segment_bytes: 465,
};

pub(crate) const PLANE_MASTER: WasmStats = WasmStats {
    wasm_bytes: 7428,
    instructions: 49,
    functions: 49,
    globals: 4,
    table_entries: 0,
    types: 17,
    data_segments: 1,
    elem_segments: 0,
    imports: 23,
    exports: 18,
    data_segment_bytes: 472,
};

// v1.7.0 contract stats
pub(crate) const ROUTER_V170: WasmStats = WasmStats {
    wasm_bytes: 42459,
    instructions: 208,
    functions: 208,
    globals: 3,
    table_entries: 0,
    types: 38,
    data_segments: 23,
    elem_segments: 0,
    imports: 55,
    exports: 76,
    data_segment_bytes: 1960,
};
pub(crate) const POOL_V170: WasmStats = WasmStats {
    wasm_bytes: 49859,
    instructions: 243,
    functions: 243,
    globals: 3,
    table_entries: 0,
    types: 44,
    data_segments: 7,
    elem_segments: 0,
    imports: 59,
    exports: 73,
    data_segment_bytes: 1659,
};
pub(crate) const TOKEN_V170: WasmStats = WasmStats {
    wasm_bytes: 9813,
    instructions: 71,
    functions: 71,
    globals: 3,
    table_entries: 0,
    types: 25,
    data_segments: 1,
    elem_segments: 0,
    imports: 25,
    exports: 23,
    data_segment_bytes: 459,
};
pub(crate) const PLANE_V170: WasmStats = WasmStats {
    wasm_bytes: 7452,
    instructions: 50,
    functions: 50,
    globals: 3,
    table_entries: 0,
    types: 17,
    data_segments: 1,
    elem_segments: 0,
    imports: 23,
    exports: 18,
    data_segment_bytes: 472,
};

/// Estimate VM memory overhead for a swap scenario.
///
/// In production, each unique contract is parsed once (first call), then
/// cached-instantiated on subsequent calls within the same transaction.
/// SAC tokens run natively (no VM cost), but custom Wasm tokens add VM overhead.
///
/// `token_calls`: number of cross-contract calls to token contracts (balance, transfer).
///   In master with _sync_reserves, each pool.swap/estimate adds 2 token.balance() calls.
pub(crate) fn estimate_vm_overhead(
    label: &str,
    router: &WasmStats,
    pool: &WasmStats,
    token: &WasmStats,
    plane: &WasmStats,
    num_pools: u64,
    token_calls: u64,
    plane_calls: u64,
) -> u64 {
    // Router: 1 first call
    let router_mem = router.first_call_mem();

    // Pool: first pool is first call, rest are cached
    let pool_mem = if num_pools > 0 {
        pool.first_call_mem() + (num_pools - 1) * pool.cached_call_mem()
    } else {
        0
    };

    // Token: each unique token wasm is first call, rest cached.
    // For simplicity, assume 1 unique token wasm type.
    let token_mem = if token_calls > 0 {
        token.first_call_mem() + (token_calls - 1) * token.cached_call_mem()
    } else {
        0
    };

    // Plane: 1 first call, rest cached
    let plane_mem = if plane_calls > 0 {
        plane.first_call_mem() + (plane_calls - 1) * plane.cached_call_mem()
    } else {
        0
    };

    let total = router_mem + pool_mem + token_mem + plane_mem;
    std::println!("=== VM overhead estimate: {} ===", label);
    std::println!(
        "  Router (1 first):   {} ({:.2} MB)",
        router_mem,
        router_mem as f64 / 1_048_576.0
    );
    std::println!(
        "  Pools ({} calls):    {} ({:.2} MB)",
        num_pools,
        pool_mem,
        pool_mem as f64 / 1_048_576.0
    );
    std::println!(
        "  Tokens ({} calls):   {} ({:.2} MB)",
        token_calls,
        token_mem,
        token_mem as f64 / 1_048_576.0
    );
    std::println!(
        "  Plane ({} calls):    {} ({:.2} MB)",
        plane_calls,
        plane_mem,
        plane_mem as f64 / 1_048_576.0
    );
    std::println!(
        "  Total VM overhead:  {} ({:.2} MB)",
        total,
        total as f64 / 1_048_576.0
    );
    total
}

pub(crate) fn measure_budget<F>(env: &Env, label: &str, f: F)
where
    F: FnOnce(),
{
    env.cost_estimate().budget().reset_unlimited();
    env.cost_estimate().budget().reset_tracker();
    f();
    let mem = env.cost_estimate().budget().memory_bytes_cost();
    let cpu = env.cost_estimate().budget().cpu_instruction_cost();
    std::println!("=== Budget: {} ===", label);
    std::println!("  CPU instructions: {}", cpu);
    std::println!(
        "  Memory bytes:     {} ({:.2} MB)",
        mem,
        mem as f64 / 1_048_576.0
    );
    std::println!("  Mainnet limit:    41943040 (40.00 MB)");
    if mem > 41943040 {
        std::println!(
            "  *** EXCEEDS MAINNET LIMIT by {:.2} MB ***",
            (mem - 41943040) as f64 / 1_048_576.0
        );
    } else {
        std::println!(
            "  Headroom:         {:.2} MB ({:.1}%)",
            (41943040 - mem) as f64 / 1_048_576.0,
            (41943040 - mem) as f64 / 41943040.0 * 100.0
        );
    }
    env.cost_estimate().budget().print();
}

/// Combined measurement: test-measured budget + estimated VM overhead.
pub(crate) fn measure_budget_with_vm<F>(
    env: &Env,
    label: &str,
    router: &WasmStats,
    pool: &WasmStats,
    token: &WasmStats,
    plane: &WasmStats,
    num_pools: u64,
    token_calls: u64,
    plane_calls: u64,
    f: F,
) where
    F: FnOnce(),
{
    env.cost_estimate().budget().reset_unlimited();
    env.cost_estimate().budget().reset_tracker();
    f();
    let measured_mem = env.cost_estimate().budget().memory_bytes_cost();
    let measured_cpu = env.cost_estimate().budget().cpu_instruction_cost();

    let vm_mem = estimate_vm_overhead(
        label,
        router,
        pool,
        token,
        plane,
        num_pools,
        token_calls,
        plane_calls,
    );
    let total_mem = measured_mem + vm_mem;

    std::println!("=== Combined estimate: {} ===", label);
    std::println!(
        "  Measured memory:  {} ({:.2} MB)",
        measured_mem,
        measured_mem as f64 / 1_048_576.0
    );
    std::println!(
        "  VM overhead:      {} ({:.2} MB)",
        vm_mem,
        vm_mem as f64 / 1_048_576.0
    );
    std::println!(
        "  TOTAL estimated:  {} ({:.2} MB)",
        total_mem,
        total_mem as f64 / 1_048_576.0
    );
    std::println!("  Measured CPU:     {}", measured_cpu);
    std::println!("  Mainnet limit:    41943040 (40.00 MB)");
    if total_mem > 41943040 {
        std::println!(
            "  *** EXCEEDS MAINNET LIMIT by {:.2} MB ***",
            (total_mem - 41943040) as f64 / 1_048_576.0
        );
    } else {
        std::println!(
            "  Headroom:         {:.2} MB ({:.1}%)",
            (41943040 - total_mem) as f64 / 1_048_576.0,
            (41943040 - total_mem) as f64 / 41943040.0 * 100.0
        );
    }
    std::println!();
}

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
        router.init_config_storage(
            &admin,
            &deploy_config_storage(&e, &admin, &emergency_admin).address,
        );
        router.set_rewards_gauge_hash(
            &admin,
            &e.deployer()
                .upload_contract_wasm(contracts::rewards_gauge::WASM),
        );
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
        router.set_protocol_fee_fraction(&admin, &5000);

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
        fee_denominator: u32,
    ) -> contracts::swap_fee::Client {
        contracts::swap_fee::Client::new(
            &self.env,
            &self.fee_collector_factory.deploy_swap_fee_contract(
                &operator,
                &fee_destination,
                &max_fee_fraction,
                &fee_denominator,
            ),
        )
    }
}

pub(crate) struct SetupV170<'a> {
    pub(crate) env: Env,
    pub(crate) admin: Address,
    pub(crate) router: contracts::v170::router::Client<'a>,
    pub(crate) reward_token: Address,
}

impl SetupV170<'_> {
    pub(crate) fn setup() -> Self {
        let e: Env = Env::default();
        e.mock_all_auths();
        e.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&e);
        let operator = Address::generate(&e);
        let emergency_admin = Address::generate(&e);

        let reward_token = create_token_contract(&e, &admin);
        let locked_token = create_token_contract(&e, &admin);
        let locked_token_admin = get_token_admin_client(&e, &locked_token.address);

        let boost_feed = contracts::v170::boost_feed::Client::new(
            &e,
            &e.register(
                contracts::v170::boost_feed::WASM,
                contracts::v170::boost_feed::Args::__constructor(
                    &admin,
                    &operator,
                    &emergency_admin,
                ),
            ),
        );
        locked_token_admin.mint(&admin, &53_000_000_000_0000000);
        boost_feed.set_total_supply(&operator, &53_000_000_000_0000000);

        let pool_hash = e
            .deployer()
            .upload_contract_wasm(contracts::v170::constant_product_pool::WASM);
        let token_hash = e
            .deployer()
            .upload_contract_wasm(contracts::v170::lp_token::WASM);
        let plane = contracts::v170::pool_plane::Client::new(
            &e,
            &e.register(contracts::v170::pool_plane::WASM, ()),
        );

        let router = contracts::v170::router::Client::new(
            &e,
            &e.register(contracts::v170::router::WASM, ()),
        );
        router.init_admin(&admin);

        let config_storage = contracts::v170::config_storage::Client::new(
            &e,
            &e.register(
                contracts::v170::config_storage::WASM,
                contracts::v170::config_storage::Args::__constructor(&admin, &emergency_admin),
            ),
        );
        router.init_config_storage(&admin, &config_storage.address);
        router.set_rewards_gauge_hash(
            &admin,
            &e.deployer()
                .upload_contract_wasm(contracts::v170::rewards_gauge::WASM),
        );
        router.set_pool_hash(&admin, &pool_hash);
        router.set_stableswap_pool_hash(
            &admin,
            &e.deployer()
                .upload_contract_wasm(contracts::v170::stableswap_pool::WASM),
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
        router.set_protocol_fee_fraction(&admin, &5000);

        Self {
            env: e,
            admin,
            router,
            reward_token: reward_token.address,
        }
    }

    pub(crate) fn deploy_standard_pool(
        &self,
        token_a: &Address,
        token_b: &Address,
        fee_fraction: u32,
    ) -> (contracts::v170::constant_product_pool::Client, BytesN<32>) {
        get_token_admin_client(&self.env, &self.reward_token).mint(&self.admin, &10_0000000);
        let (pool_hash, pool_address) = self.router.init_standard_pool(
            &self.admin,
            &Vec::from_array(&self.env, [token_a.clone(), token_b.clone()]),
            &fee_fraction,
        );
        (
            contracts::v170::constant_product_pool::Client::new(&self.env, &pool_address),
            pool_hash,
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

fn deploy_config_storage<'a>(
    e: &Env,
    admin: &Address,
    emergency_admin: &Address,
) -> contracts::config_storage::Client<'a> {
    contracts::config_storage::Client::new(
        e,
        &e.register(
            contracts::config_storage::WASM,
            contracts::config_storage::Args::__constructor(admin, emergency_admin),
        ),
    )
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
