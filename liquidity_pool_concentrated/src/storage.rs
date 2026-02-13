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

pub const MIN_TICK: i32 = -887_272;
pub const MAX_TICK: i32 = 887_272;
pub const FEE_DENOMINATOR: u128 = 10_000;
pub const MAX_USER_POSITIONS: u32 = 50;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Router,
    Plane,
    Token0,
    Token1,
    Fee,
    TickSpacing,
    Slot0,
    Liquidity,
    FeeGrowthGlobal0X128,
    FeeGrowthGlobal1X128,
    ProtocolFees,
    ProtocolFeeFraction,
    IsKilledDeposit,
    IsKilledSwap,
    TokenFutureWasm,
    GaugeFutureWasm,
    TickBitmap(i32),

    Position(Address, i32, i32),
    Tick(i32),
    UserPositions(Address),

    // Distance-weighted liquidity for rewards.
    DistanceWeightConfig,
    UserRawLiquidity(Address),
    UserWeightedLiquidity(Address),
    TotalRawLiquidity,
    TotalWeightedLiquidity,

    ClaimKilled,
}

generate_instance_storage_getter_and_setter!(router, DataKey::Router, Address);
generate_instance_storage_getter_and_setter!(plane, DataKey::Plane, Address);
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
