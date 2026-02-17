#![allow(dead_code)]

use crate::types::{
    PoolState, PoolStateWithBalances, PositionData, ProtocolFees, Slot0, SwapResult, TickInfo,
    UserPositionSnapshot,
};
use crate::Error;
use soroban_sdk::{Address, BytesN, Env, Map, Symbol, Val, Vec, U256};

pub trait ManagedLiquidityPool {
    fn initialize_all(
        e: Env,
        admin: Address,
        privileged_addrs: (Address, Address, Address, Address, Vec<Address>, Address),
        router: Address,
        tokens: Vec<Address>,
        fee: u32,
        tick_spacing: i32,
        reward_config: (Address, Address, Address),
        plane: Address,
    );
}

pub trait LiquidityPoolInterfaceTrait {
    fn pool_type(e: Env) -> Symbol;

    fn initialize(
        e: Env,
        admin: Address,
        privileged_addrs: (Address, Address, Address, Address, Vec<Address>, Address),
        router: Address,
        tokens: Vec<Address>,
        fee: u32,
        tick_spacing: i32,
    );

    fn get_fee_fraction(e: Env) -> u32;
    fn get_protocol_fee_fraction(e: Env) -> u32;
    fn share_id(e: Env) -> Address;
    fn get_total_shares(e: Env) -> u128;
    fn get_user_shares(e: Env, user: Address) -> u128;
    fn get_reserves(e: Env) -> Vec<u128>;
    fn get_tokens(e: Env) -> Vec<Address>;

    fn deposit(
        e: Env,
        user: Address,
        desired_amounts: Vec<u128>,
        min_shares: u128,
    ) -> (Vec<u128>, u128);
    fn estimate_deposit(e: Env, desired_amounts: Vec<u128>) -> u128;

    fn swap(
        e: Env,
        user: Address,
        in_idx: u32,
        out_idx: u32,
        in_amount: u128,
        out_min: u128,
    ) -> u128;
    fn estimate_swap(e: Env, in_idx: u32, out_idx: u32, in_amount: u128) -> u128;

    fn swap_strict_receive(
        e: Env,
        user: Address,
        in_idx: u32,
        out_idx: u32,
        out_amount: u128,
        in_max: u128,
    ) -> u128;
    fn estimate_swap_strict_receive(e: Env, in_idx: u32, out_idx: u32, out_amount: u128) -> u128;

    fn withdraw(e: Env, user: Address, share_amount: u128, min_amounts: Vec<u128>) -> Vec<u128>;
    fn get_info(e: Env) -> Map<Symbol, Val>;
    fn get_total_excluded_shares(e: Env) -> u128;
}

pub trait RewardsTrait {
    fn initialize_rewards_config(e: Env, reward_token: Address);
    fn initialize_boost_config(e: Env, reward_boost_token: Address, reward_boost_feed: Address);
    fn set_reward_boost_config(
        e: Env,
        admin: Address,
        reward_boost_token: Address,
        reward_boost_feed: Address,
    );
    fn set_rewards_config(e: Env, admin: Address, expired_at: u64, tps: u128);
    fn get_unused_reward(e: Env) -> u128;
    fn return_unused_reward(e: Env, admin: Address) -> u128;
    fn get_rewards_info(e: Env, user: Address) -> Map<Symbol, i128>;
    fn get_user_reward(e: Env, user: Address) -> u128;
    fn estimate_working_balance(e: Env, user: Address, new_user_shares: u128) -> (u128, u128);
    fn get_total_accumulated_reward(e: Env) -> u128;
    fn get_total_configured_reward(e: Env) -> u128;
    fn adjust_total_accumulated_reward(e: Env, admin: Address, diff: i128);
    fn get_total_claimed_reward(e: Env) -> u128;
    fn claim(e: Env, user: Address) -> u128;
    fn get_rewards_state(e: Env, user: Address) -> bool;
    fn set_rewards_state(e: Env, user: Address, state: bool);
    fn admin_set_rewards_state(e: Env, admin: Address, user: Address, state: bool);
}

pub trait AdminInterfaceTrait {
    fn set_distance_weighting(
        e: Env,
        admin: Address,
        max_distance_ticks: u32,
        min_multiplier_bps: u32,
    );
    fn get_distance_weighting(e: Env) -> rewards::concentrated_weight::DistanceWeightConfig;

    fn set_claim_killed(e: Env, admin: Address, value: bool);
    fn get_claim_killed(e: Env) -> bool;

    fn set_privileged_addrs(
        e: Env,
        admin: Address,
        rewards_admin: Address,
        operations_admin: Address,
        pause_admin: Address,
        emergency_pause_admins: Vec<Address>,
        system_fee_admin: Address,
    );
    fn get_privileged_addrs(e: Env) -> Map<Symbol, Vec<Address>>;

    fn kill_deposit(e: Env, admin: Address);
    fn kill_swap(e: Env, admin: Address);
    fn kill_claim(e: Env, admin: Address);
    fn unkill_deposit(e: Env, admin: Address);
    fn unkill_swap(e: Env, admin: Address);
    fn unkill_claim(e: Env, admin: Address);
    fn get_is_killed_deposit(e: Env) -> bool;
    fn get_is_killed_swap(e: Env) -> bool;
    fn get_is_killed_claim(e: Env) -> bool;

    fn set_protocol_fee_fraction(e: Env, admin: Address, new_fraction: u32);
    fn get_protocol_fees(e: Env) -> Vec<u128>;
    fn claim_protocol_fees(e: Env, admin: Address, destination: Address) -> Vec<u128>;
}

pub trait UpgradeableContract {
    fn version() -> u32;
    fn contract_name(e: Env) -> Symbol;

    fn commit_upgrade(
        e: Env,
        admin: Address,
        new_wasm_hash: BytesN<32>,
        token_new_wasm_hash: BytesN<32>,
        gauges_new_wasm_hash: BytesN<32>,
    );
    fn apply_upgrade(e: Env, admin: Address) -> (BytesN<32>, BytesN<32>);
    fn revert_upgrade(e: Env, admin: Address);

    fn set_emergency_mode(e: Env, emergency_admin: Address, value: bool);
    fn get_emergency_mode(e: Env) -> bool;
}

pub trait ConcentratedPoolExtensionsTrait {
    fn check_ticks(e: Env, tick_lower: i32, tick_upper: i32) -> Result<(), Error>;
    fn block_timestamp(e: Env) -> u64;
    fn initialize_price(e: Env, admin: Address, sqrt_price_x96: U256) -> Result<(), Error>;

    fn swap_by_tokens(
        e: Env,
        sender: Address,
        recipient: Address,
        token_in: Address,
        token_out: Address,
        amount_specified: i128,
        sqrt_price_limit_x96: U256,
    ) -> Result<SwapResult, Error>;

    fn deposit_position(
        e: Env,
        sender: Address,
        recipient: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount: u128,
    ) -> Result<(u128, u128), Error>;

    fn withdraw_position(
        e: Env,
        owner: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount: u128,
    ) -> Result<(u128, u128), Error>;

    fn claim_position_fees(
        e: Env,
        owner: Address,
        recipient: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount0_requested: u128,
        amount1_requested: u128,
    ) -> Result<(u128, u128), Error>;

    fn slot0(e: Env) -> Slot0;
    fn router(e: Env) -> Address;
    fn token0(e: Env) -> Address;
    fn token1(e: Env) -> Address;
    fn fee(e: Env) -> u32;
    fn tick_spacing(e: Env) -> i32;
    fn chunk_bitmap(e: Env, word_pos: i32) -> U256;
    fn liquidity(e: Env) -> u128;
    fn fee_growth_global_0_x128(e: Env) -> U256;
    fn fee_growth_global_1_x128(e: Env) -> U256;
    fn protocol_fees(e: Env) -> ProtocolFees;
    fn ticks(e: Env, tick: i32) -> TickInfo;
    fn get_position(e: Env, recipient: Address, tick_lower: i32, tick_upper: i32) -> PositionData;
    fn get_full_pool_state(e: Env) -> Option<PoolState>;
    fn get_pool_state_with_balances(e: Env) -> Option<PoolStateWithBalances>;
    fn get_user_position_snapshot(e: Env, user: Address) -> UserPositionSnapshot;
    fn get_total_weighted_liquidity(e: Env) -> u128;
    fn get_total_raw_liquidity(e: Env) -> u128;

    // Batch-read consecutive chunk bitmap words starting at `start_word`.
    fn get_chunk_bitmap_batch(e: Env, start_word: i32, count: u32) -> Vec<U256>;

    // Batch-read tick info for the specified ticks.
    fn get_ticks_batch(e: Env, ticks: Vec<i32>) -> Vec<TickInfo>;
}
