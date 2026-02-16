use crate::types::{PositionData, PositionRange, ProtocolFees, Slot0, TickInfo};
use paste::paste;
use rewards::concentrated_weight::DistanceWeightConfig;
use soroban_sdk::{contracttype, panic_with_error, Address, BytesN, Env, Vec};
use utils::storage_errors::StorageError;
use utils::{
    generate_instance_storage_getter, generate_instance_storage_getter_and_setter,
    generate_instance_storage_getter_and_setter_with_default,
    generate_instance_storage_getter_with_default, generate_instance_storage_setter,
};

// Uniswap V3 tick bounds: tick_at_sqrt_ratio(MIN_SQRT_RATIO) and tick_at_sqrt_ratio(MAX_SQRT_RATIO).
// Price range: [1.0001^-887272, 1.0001^887272] ≈ [2.35e-39, 4.25e+38].
pub const MIN_TICK: i32 = -887_272;
pub const MAX_TICK: i32 = 887_272;

// Fee precision: fee=30 means 30/10_000 = 0.3%.
pub const FEE_DENOMINATOR: u128 = 10_000;

// Max positions per user account (prevents storage bloat from griefing).
pub const MAX_USER_POSITIONS: u32 = 20;

// Storage layout. Instance storage for pool-wide config (accessed every tx),
// persistent storage for per-user and per-tick data (accessed selectively).
#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    // ── Instance storage: pool config & global state ──
    Router,             // Address — router contract that manages this pool
    Plane,              // Address — pool plane for batch metadata queries
    Token0,             // Address — first token (sorted)
    Token1,             // Address — second token (sorted)
    Fee,                // u32 — fee in basis points (e.g. 30 = 0.3%)
    TickSpacing,        // i32 — min distance between initialized ticks
    Slot0,              // Slot0 — current sqrt_price and tick
    Liquidity,          // u128 — active liquidity (positions overlapping current tick)
    FeeGrowthGlobal0X128, // U256 — cumulative fee growth for token0, Q128
    FeeGrowthGlobal1X128, // U256 — cumulative fee growth for token1, Q128
    ProtocolFees,       // ProtocolFees — uncollected protocol fee amounts
    ProtocolFeeFraction, // u32 — protocol's share of fees, per FEE_DENOMINATOR
    IsKilledDeposit,    // bool — deposit kill switch
    IsKilledSwap,       // bool — swap kill switch
    TokenFutureWasm,    // BytesN<32> — staged LP token WASM hash for upgrade
    GaugeFutureWasm,    // BytesN<32> — staged gauge WASM hash for upgrade

    // ── Persistent storage: per-tick ──
    TickBitmap(i32),    // U256 — 256-bit bitmap word; each bit = one tick_spacing slot.
                        //   word_pos = tick / (tick_spacing * 256). Bit = (tick/spacing) % 256.
    Tick(i32),          // TickInfo — per-tick liquidity deltas and fee growth snapshots

    // ── Persistent storage: per-user ──
    Position(Address, i32, i32), // PositionData — keyed by (owner, tick_lower, tick_upper)
    UserPositions(Address),      // Vec<PositionRange> — list of user's active ranges

    // ── Rewards: distance-weighted liquidity ──
    DistanceWeightConfig,         // DistanceWeightConfig — how position distance affects rewards
    UserRawLiquidity(Address),    // u128 — user's total liquidity across all positions
    UserWeightedLiquidity(Address), // u128 — user's liquidity after distance multiplier
    TotalRawLiquidity,            // u128 — sum of all users' raw liquidity
    TotalWeightedLiquidity,       // u128 — sum of all users' weighted liquidity

    ClaimKilled,        // bool — reward claim kill switch
}

generate_instance_storage_getter_and_setter!(router, DataKey::Router, Address);
generate_instance_storage_getter_and_setter!(plane, DataKey::Plane, Address);

pub fn has_plane(e: &Env) -> bool {
    e.storage().instance().has(&DataKey::Plane)
}
generate_instance_storage_getter_and_setter!(token0, DataKey::Token0, Address);
generate_instance_storage_getter_and_setter!(token1, DataKey::Token1, Address);
generate_instance_storage_getter_and_setter!(fee, DataKey::Fee, u32);
generate_instance_storage_getter_and_setter!(tick_spacing, DataKey::TickSpacing, i32);
generate_instance_storage_getter_and_setter!(slot0, DataKey::Slot0, Slot0);
generate_instance_storage_getter_and_setter_with_default!(liquidity, DataKey::Liquidity, u128, 0);
generate_instance_storage_getter_and_setter!(
    fee_growth_global_0_x128,
    DataKey::FeeGrowthGlobal0X128,
    soroban_sdk::U256
);
generate_instance_storage_getter_and_setter!(
    fee_growth_global_1_x128,
    DataKey::FeeGrowthGlobal1X128,
    soroban_sdk::U256
);
generate_instance_storage_getter_and_setter_with_default!(
    protocol_fees,
    DataKey::ProtocolFees,
    ProtocolFees,
    ProtocolFees {
        token0: 0,
        token1: 0,
    }
);
generate_instance_storage_getter_and_setter_with_default!(
    protocol_fee_fraction,
    DataKey::ProtocolFeeFraction,
    u32,
    5_000
);
generate_instance_storage_getter_and_setter_with_default!(
    is_killed_deposit,
    DataKey::IsKilledDeposit,
    bool,
    false
);
generate_instance_storage_getter_and_setter_with_default!(
    is_killed_swap,
    DataKey::IsKilledSwap,
    bool,
    false
);
generate_instance_storage_getter_and_setter!(
    token_future_wasm,
    DataKey::TokenFutureWasm,
    BytesN<32>
);
generate_instance_storage_getter_and_setter!(
    gauge_future_wasm,
    DataKey::GaugeFutureWasm,
    BytesN<32>
);
generate_instance_storage_getter_and_setter_with_default!(
    distance_weight_config,
    DataKey::DistanceWeightConfig,
    DistanceWeightConfig,
    DistanceWeightConfig {
        max_distance_ticks: 5_000,
        min_multiplier_bps: 0,
    }
);
generate_instance_storage_getter_and_setter_with_default!(
    total_raw_liquidity,
    DataKey::TotalRawLiquidity,
    u128,
    0
);
generate_instance_storage_getter_and_setter_with_default!(
    total_weighted_liquidity,
    DataKey::TotalWeightedLiquidity,
    u128,
    0
);
generate_instance_storage_getter_and_setter_with_default!(
    claim_killed,
    DataKey::ClaimKilled,
    bool,
    false
);

// ── Position accessors (persistent storage) ──
// Keyed by (owner, tick_lower, tick_upper). A user can have up to MAX_USER_POSITIONS
// distinct ranges. Returns None if position doesn't exist.
pub fn get_position(
    e: &Env,
    owner: &Address,
    tick_lower: i32,
    tick_upper: i32,
) -> Option<PositionData> {
    e.storage()
        .persistent()
        .get(&DataKey::Position(owner.clone(), tick_lower, tick_upper))
}

pub fn set_position(
    e: &Env,
    owner: &Address,
    tick_lower: i32,
    tick_upper: i32,
    value: &PositionData,
) {
    e.storage().persistent().set(
        &DataKey::Position(owner.clone(), tick_lower, tick_upper),
        value,
    );
}

pub fn remove_position(e: &Env, owner: &Address, tick_lower: i32, tick_upper: i32) {
    e.storage()
        .persistent()
        .remove(&DataKey::Position(owner.clone(), tick_lower, tick_upper));
}

// ── Tick accessors (persistent storage) ──
// Returns default (zero/uninitialized) TickInfo if tick hasn't been written.
// Each initialized tick costs one ledger entry during swap traversal.
pub fn get_tick(e: &Env, tick: i32) -> TickInfo {
    e.storage()
        .persistent()
        .get(&DataKey::Tick(tick))
        .unwrap_or(TickInfo {
            fee_growth_outside_0_x128: soroban_sdk::U256::from_u32(e, 0),
            fee_growth_outside_1_x128: soroban_sdk::U256::from_u32(e, 0),
            initialized: false,
            liquidity_gross: 0,
            liquidity_net: 0,
        })
}

pub fn set_tick(e: &Env, tick: i32, value: &TickInfo) {
    e.storage().persistent().set(&DataKey::Tick(tick), value);
}

// ── Tick bitmap accessors (persistent storage) ──
// 256-bit words for efficient tick scanning. Each bit marks an initialized tick.
// word_pos = compressed_tick / 256, where compressed_tick = tick / tick_spacing.
pub fn get_tick_bitmap_word(e: &Env, word_pos: i32) -> soroban_sdk::U256 {
    e.storage()
        .persistent()
        .get(&DataKey::TickBitmap(word_pos))
        .unwrap_or_else(|| soroban_sdk::U256::from_u32(e, 0))
}

pub fn set_tick_bitmap_word(e: &Env, word_pos: i32, word: &soroban_sdk::U256) {
    e.storage()
        .persistent()
        .set(&DataKey::TickBitmap(word_pos), word);
}

// ── User position list (persistent storage) ──
// Tracks which tick ranges a user has positions in. Max MAX_USER_POSITIONS entries.
pub fn get_user_positions(e: &Env, user: &Address) -> Vec<PositionRange> {
    e.storage()
        .persistent()
        .get(&DataKey::UserPositions(user.clone()))
        .unwrap_or(Vec::new(e))
}

pub fn set_user_positions(e: &Env, user: &Address, ranges: &Vec<PositionRange>) {
    e.storage()
        .persistent()
        .set(&DataKey::UserPositions(user.clone()), ranges);
}

// ── Rewards liquidity tracking (persistent storage) ──
// Raw = unweighted sum of all position amounts.
// Weighted = raw * distance_multiplier (positions closer to price get higher weight).
pub fn get_user_raw_liquidity(e: &Env, user: &Address) -> u128 {
    e.storage()
        .persistent()
        .get(&DataKey::UserRawLiquidity(user.clone()))
        .unwrap_or(0)
}

pub fn set_user_raw_liquidity(e: &Env, user: &Address, value: u128) {
    e.storage()
        .persistent()
        .set(&DataKey::UserRawLiquidity(user.clone()), &value);
}

pub fn get_user_weighted_liquidity(e: &Env, user: &Address) -> u128 {
    e.storage()
        .persistent()
        .get(&DataKey::UserWeightedLiquidity(user.clone()))
        .unwrap_or(0)
}

pub fn set_user_weighted_liquidity(e: &Env, user: &Address, value: u128) {
    e.storage()
        .persistent()
        .set(&DataKey::UserWeightedLiquidity(user.clone()), &value);
}
