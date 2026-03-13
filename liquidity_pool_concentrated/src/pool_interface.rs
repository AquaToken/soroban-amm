#![allow(dead_code)]

use crate::types::{
    PoolState, PoolStateWithBalances, PositionData, ProtocolFees, Slot0, TickInfo,
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
    fn estimate_working_balance(
        e: Env,
        user: Address,
        tick_lower: i32,
        tick_upper: i32,
        new_liquidity: u128,
    ) -> (u128, u128);
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

    // ── Migration (temporary, remove after all pools migrated) ──

    // Build WordBitmap (L2) entries and compute MinInitTick/MaxInitTick
    // from existing ChunkBitmap words in range [from_word, to_word] inclusive.
    // Call in batches covering the full L1 word range for the pool's tick_spacing.
    fn migrate_bitmap(e: Env, admin: Address, from_word: i32, to_word: i32);

    // Move pool price from MIN_TICK/MAX_TICK to just outside the initialized
    // tick range, so that the next swap can activate liquidity naturally.
    fn unbrick_pool(e: Env, admin: Address);
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
    fn estimate_deposit_position(
        e: Env,
        tick_lower: i32,
        tick_upper: i32,
        desired_amounts: Vec<u128>,
    ) -> (Vec<u128>, u128);

    fn deposit_position(
        e: Env,
        sender: Address,
        tick_lower: i32,
        tick_upper: i32,
        desired_amounts: Vec<u128>,
        min_liquidity: u128,
    ) -> (Vec<u128>, u128);

    fn estimate_withdraw_position(
        e: Env,
        owner: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount: u128,
    ) -> Vec<u128>;

    fn withdraw_position(
        e: Env,
        owner: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount: u128,
        min_amounts: Vec<u128>,
    ) -> Vec<u128>;

    fn get_position_fees(e: Env, owner: Address, tick_lower: i32, tick_upper: i32) -> Vec<u128>;

    fn claim_position_fees(e: Env, owner: Address, tick_lower: i32, tick_upper: i32) -> Vec<u128>;

    fn get_all_position_fees(e: Env, owner: Address) -> Vec<u128>;
    fn claim_all_position_fees(e: Env, owner: Address) -> Vec<u128>;

    fn get_slot0(e: Env) -> Slot0;
    fn get_tick_spacing(e: Env) -> i32;
    fn get_chunk_bitmap(e: Env, word_pos: i32) -> U256;
    fn get_active_liquidity(e: Env) -> u128;
    fn get_fee_growth_global_0_x128(e: Env) -> U256;
    fn get_fee_growth_global_1_x128(e: Env) -> U256;
    fn get_tick(e: Env, tick: i32) -> TickInfo;
    fn get_position(e: Env, recipient: Address, tick_lower: i32, tick_upper: i32) -> PositionData;
    fn get_user_position_snapshot(e: Env, user: Address) -> UserPositionSnapshot;
    fn get_total_weighted_liquidity(e: Env) -> u128;
    fn get_total_raw_liquidity(e: Env) -> u128;

    // Batch-read consecutive chunk bitmap words starting at `start_word`.
    fn get_chunk_bitmap_batch(e: Env, start_word: i32, count: u32) -> Vec<U256>;

    // Batch-read tick info for the specified ticks.
    fn get_ticks_batch(e: Env, ticks: Vec<i32>) -> Vec<TickInfo>;

    // Compute the tick corresponding to the price ratio amount1/amount0.
    // Useful for frontends to derive the initial price tick before the first deposit.
    fn tick_from_amounts(e: Env, amount0: u128, amount1: u128) -> i32;

    // Returns (min_init_tick, max_init_tick) — the bounds of initialized ticks.
    // When min > max the pool has no initialized ticks.
    fn get_tick_bounds(e: Env) -> (i32, i32);
}
