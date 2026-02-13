use crate::errors::Error;
use crate::math::{
    amount0_delta, amount1_delta, fee_growth_delta_x128, get_next_sqrt_price_from_input,
    get_next_sqrt_price_from_output, max_sqrt_ratio, min_sqrt_ratio, mul_div_fee_growth,
    mul_div_u128, sqrt_ratio_at_tick, tick_at_sqrt_ratio, wrapping_sub_u256,
};
use crate::plane::update_plane;
use crate::plane_interface::Plane;
use crate::pool_interface::{
    AdminInterfaceTrait, ConcentratedPoolExtensionsTrait, LiquidityPoolInterfaceTrait,
    ManagedLiquidityPool, RewardsGaugeInterfaceTrait, RewardsTrait, UpgradeableContract,
};
use crate::storage::{
    get_claim_killed, get_distance_weight_config, get_fee, get_fee_growth_global_0_x128,
    get_fee_growth_global_1_x128, get_gauge_future_wasm, get_is_killed_deposit, get_is_killed_swap,
    get_liquidity, get_plane, get_position, get_protocol_fee_fraction, get_protocol_fees,
    get_router, get_slot0, get_tick, get_tick_bitmap_word, get_tick_spacing, get_token0,
    get_token1, get_token_future_wasm, get_total_raw_liquidity, get_total_weighted_liquidity,
    get_user_positions, get_user_raw_liquidity, get_user_weighted_liquidity, remove_position,
    set_claim_killed, set_distance_weight_config, set_fee, set_fee_growth_global_0_x128,
    set_fee_growth_global_1_x128, set_gauge_future_wasm, set_is_killed_deposit, set_is_killed_swap,
    set_liquidity, set_plane, set_position, set_protocol_fee_fraction, set_protocol_fees,
    set_router, set_slot0, set_tick, set_tick_bitmap_word, set_tick_spacing, set_token0,
    set_token1, set_token_future_wasm, set_total_raw_liquidity, set_total_weighted_liquidity,
    set_user_positions, set_user_raw_liquidity, set_user_weighted_liquidity, FEE_DENOMINATOR,
    MAX_TICK, MAX_USER_POSITIONS, MIN_TICK,
};
use crate::types::{
    PoolState, PoolStateWithBalances, PositionData, PositionRange, ProtocolFees, Slot0, SwapResult,
    TickInfo, UserPositionSnapshot,
};
use access_control::access::{AccessControl, AccessControlTrait};
use access_control::emergency::{get_emergency_mode, set_emergency_mode};
use access_control::events::Events as AccessControlEvents;
use access_control::interface::TransferableContract;
use access_control::management::{MultipleAddressesManagementTrait, SingleAddressManagementTrait};
use access_control::role::{Role, SymbolRepresentation};
use access_control::transfer::TransferOwnershipTrait;
use access_control::utils::{
    require_operations_admin_or_owner, require_pause_admin_or_owner,
    require_pause_or_emergency_pause_admin_or_owner, require_rewards_admin_or_owner,
    require_system_fee_admin_or_owner,
};
use liquidity_pool_events::Events as PoolEvents;
use liquidity_pool_events::LiquidityPoolEvents;
use liqidity_pool_rewards_gauge as rewards_gauge;
use rewards::concentrated_weight::{
    apply_multiplier, position_multiplier_bps, DistanceWeightConfig,
};
use rewards::events::Events as RewardEvents;
use rewards::storage::{
    BoostFeedStorageTrait, BoostTokenStorageTrait, PoolRewardsStorageTrait, RewardTokenStorageTrait,
};
use rewards::Rewards;
use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{
    contract, contractimpl, contractmeta, map, panic_with_error, symbol_short, Address, Bytes,
    BytesN, Env, IntoVal, Map, Symbol, Val, Vec, U256,
};
use upgrade::events::Events as UpgradeEvents;
use upgrade::{apply_upgrade, commit_upgrade, revert_upgrade};

contractmeta!(
    key = "Description",
    val = "Concentrated liquidity pool inspired by Uniswap v3, adapted for Soroban"
);

#[contract]
pub struct ConcentratedLiquidityPool;

struct SwapStep {
    sqrt_next: U256,
    amount_in: u128,
    amount_out: u128,
    fee_amount: u128,
}

mod admin;
mod extensions;
mod internal;
mod liquidity_pool_interface;
mod managed;
mod plane;
mod rewards_gauge_impl;
mod rewards_impl;
mod transferable;
mod upgradeable;
