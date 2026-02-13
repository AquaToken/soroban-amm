use crate::errors::Error;
use crate::math::{
    amount0_delta, amount1_delta, fee_growth_delta_x128, max_sqrt_ratio, min_sqrt_ratio,
    mul_div_fee_growth, mul_div_u128, sqrt_ratio_at_tick, tick_at_sqrt_ratio,
};
use crate::plane::update_plane;
use crate::plane_interface::Plane;
use crate::pool_interface::{
    AdminInterfaceTrait, ConcentratedPoolExtensionsTrait, LiquidityPoolInterfaceTrait,
    ManagedLiquidityPool, RewardsTrait, UpgradeableContract,
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
    MAX_TICK, MIN_TICK,
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

impl ConcentratedLiquidityPool {
    fn has_admin_role(e: &Env) -> bool {
        AccessControl::new(e).get_role_safe(&Role::Admin).is_some()
    }

    fn require_admin(e: &Env, admin: &Address) {
        admin.require_auth();
        AccessControl::new(e).assert_address_has_role(admin, &Role::Admin);
    }

    fn check_ticks_internal(e: &Env, tick_lower: i32, tick_upper: i32) -> Result<(), Error> {
        if tick_lower >= tick_upper {
            return Err(Error::TickLowerNotLessThanUpper);
        }
        if tick_lower < MIN_TICK {
            return Err(Error::TickLowerTooLow);
        }
        if tick_upper > MAX_TICK {
            return Err(Error::TickUpperTooHigh);
        }

        let spacing = get_tick_spacing(e);
        if spacing <= 0 {
            return Err(Error::InvalidTickSpacing);
        }
        if tick_lower % spacing != 0 || tick_upper % spacing != 0 {
            return Err(Error::TickNotSpacedCorrectly);
        }
        Ok(())
    }

    fn abs_i128(v: i128) -> u128 {
        if v < 0 {
            (-v) as u128
        } else {
            v as u128
        }
    }

    fn u128_to_i128(v: u128) -> Result<i128, Error> {
        if v > i128::MAX as u128 {
            return Err(Error::LiquidityOverflow);
        }
        Ok(v as i128)
    }

    fn u256_to_array(v: &U256) -> [u8; 32] {
        let bytes = v.to_be_bytes();
        let mut out = [0u8; 32];
        bytes.copy_into_slice(&mut out);
        out
    }

    fn u256_from_array(e: &Env, bytes: &[u8; 32]) -> U256 {
        U256::from_be_bytes(e, &Bytes::from_array(e, bytes))
    }

    fn bit_is_set(word: &[u8; 32], bit_pos: u32) -> bool {
        if bit_pos >= 256 {
            return false;
        }

        let byte_idx = 31usize - (bit_pos / 8) as usize;
        let bit_idx = (bit_pos % 8) as u8;
        (word[byte_idx] & (1u8 << bit_idx)) != 0
    }

    fn set_bit(word: &mut [u8; 32], bit_pos: u32, value: bool) {
        if bit_pos >= 256 {
            return;
        }

        let byte_idx = 31usize - (bit_pos / 8) as usize;
        let bit_idx = (bit_pos % 8) as u8;
        let mask = 1u8 << bit_idx;
        if value {
            word[byte_idx] |= mask;
        } else {
            word[byte_idx] &= !mask;
        }
    }

    fn find_prev_set_bit(word: &[u8; 32], from_bit: u32) -> Option<u32> {
        let mut bit = from_bit.min(255) as i32;
        while bit >= 0 {
            if Self::bit_is_set(word, bit as u32) {
                return Some(bit as u32);
            }
            bit -= 1;
        }
        None
    }

    fn find_next_set_bit(word: &[u8; 32], from_bit: u32) -> Option<u32> {
        let mut bit = from_bit.min(255);
        while bit < 256 {
            if Self::bit_is_set(word, bit) {
                return Some(bit);
            }
            bit += 1;
        }
        None
    }

    fn compress_tick(tick: i32, spacing: i32) -> i32 {
        let mut compressed = tick / spacing;
        if tick < 0 && tick % spacing != 0 {
            compressed -= 1;
        }
        compressed
    }

    fn position(compressed_tick: i32) -> (i32, u32) {
        let word_pos = compressed_tick >> 8;
        let bit_pos = (compressed_tick & 255) as u32;
        (word_pos, bit_pos)
    }

    pub(crate) fn flip_tick(e: &Env, tick_idx: i32, spacing: i32, initialized: bool) {
        let compressed = Self::compress_tick(tick_idx, spacing);
        let (word_pos, bit_pos) = Self::position(compressed);

        let mut word = Self::u256_to_array(&get_tick_bitmap_word(e, word_pos));
        Self::set_bit(&mut word, bit_pos, initialized);
        set_tick_bitmap_word(e, word_pos, &Self::u256_from_array(e, &word));
    }

    pub(crate) fn next_initialized_tick_within_one_word(
        e: &Env,
        tick: i32,
        spacing: i32,
        lte: bool,
    ) -> (i32, bool) {
        let compressed = Self::compress_tick(tick, spacing);

        let (next_compressed, initialized) = if lte {
            let (word_pos, bit_pos) = Self::position(compressed);
            let word = Self::u256_to_array(&get_tick_bitmap_word(e, word_pos));

            if let Some(msb) = Self::find_prev_set_bit(&word, bit_pos) {
                ((word_pos << 8) + msb as i32, true)
            } else {
                (word_pos << 8, false)
            }
        } else {
            let compressed_plus_one = compressed.saturating_add(1);
            let (word_pos, bit_pos) = Self::position(compressed_plus_one);
            let word = Self::u256_to_array(&get_tick_bitmap_word(e, word_pos));

            if let Some(lsb) = Self::find_next_set_bit(&word, bit_pos) {
                ((word_pos << 8) + lsb as i32, true)
            } else {
                ((word_pos << 8) + 255, false)
            }
        };

        let next_tick = next_compressed.saturating_mul(spacing);
        let next_tick = if next_tick < MIN_TICK {
            MIN_TICK
        } else if next_tick > MAX_TICK {
            MAX_TICK
        } else {
            next_tick
        };

        (next_tick, initialized)
    }

    fn update_tick_liquidity(
        e: &Env,
        tick_idx: i32,
        liquidity_delta: i128,
        is_upper: bool,
    ) -> Result<(), Error> {
        let mut tick = get_tick(e, tick_idx);
        let prev_initialized = tick.initialized;

        let delta = Self::abs_i128(liquidity_delta);
        if liquidity_delta >= 0 {
            tick.liquidity_gross = tick.liquidity_gross.saturating_add(delta);
        } else {
            if tick.liquidity_gross < delta {
                return Err(Error::LiquidityUnderflow);
            }
            tick.liquidity_gross -= delta;
        }

        if is_upper {
            tick.liquidity_net = tick.liquidity_net.saturating_sub(liquidity_delta);
        } else {
            tick.liquidity_net = tick.liquidity_net.saturating_add(liquidity_delta);
        }

        tick.initialized = tick.liquidity_gross > 0;

        if !prev_initialized && tick.initialized {
            let slot = get_slot0(e);
            if tick_idx <= slot.tick {
                tick.fee_growth_outside_0_x128 = get_fee_growth_global_0_x128(e);
                tick.fee_growth_outside_1_x128 = get_fee_growth_global_1_x128(e);
            }
            Self::flip_tick(e, tick_idx, get_tick_spacing(e), true);
        } else if prev_initialized && !tick.initialized {
            Self::flip_tick(e, tick_idx, get_tick_spacing(e), false);
        }

        set_tick(e, tick_idx, &tick);
        Ok(())
    }

    fn ensure_user_range_exists(e: &Env, user: &Address, tick_lower: i32, tick_upper: i32) {
        let mut ranges = get_user_positions(e, user);
        for range in ranges.iter() {
            if range.tick_lower == tick_lower && range.tick_upper == tick_upper {
                return;
            }
        }

        ranges.push_back(PositionRange {
            tick_lower,
            tick_upper,
        });
        set_user_positions(e, user, &ranges);
    }

    fn remove_user_range_if_empty(e: &Env, user: &Address, tick_lower: i32, tick_upper: i32) {
        let ranges = get_user_positions(e, user);
        let mut updated = Vec::new(e);
        for range in ranges.iter() {
            if range.tick_lower == tick_lower && range.tick_upper == tick_upper {
                continue;
            }
            updated.push_back(range);
        }
        set_user_positions(e, user, &updated);
    }

    fn recompute_user_weighted_liquidity(e: &Env, user: &Address) -> u128 {
        let cfg = get_distance_weight_config(e);
        let tick_current = get_slot0(e).tick;

        let ranges = get_user_positions(e, user);
        let mut weighted = 0u128;

        for range in ranges.iter() {
            if let Some(position) = get_position(e, user, range.tick_lower, range.tick_upper) {
                if position.liquidity == 0 {
                    continue;
                }
                let multiplier =
                    position_multiplier_bps(tick_current, range.tick_lower, range.tick_upper, cfg);
                weighted =
                    weighted.saturating_add(apply_multiplier(position.liquidity, multiplier));
            }
        }

        let prev_weighted = get_user_weighted_liquidity(e, user);
        let mut total_weighted = get_total_weighted_liquidity(e);

        if weighted >= prev_weighted {
            total_weighted = total_weighted.saturating_add(weighted - prev_weighted);
        } else {
            total_weighted = total_weighted.saturating_sub(prev_weighted - weighted);
        }

        set_user_weighted_liquidity(e, user, weighted);
        set_total_weighted_liquidity(e, &total_weighted);

        weighted
    }

    fn update_user_raw_liquidity(e: &Env, user: &Address, delta: i128) {
        let prev_user_raw = get_user_raw_liquidity(e, user);
        let prev_total_raw = get_total_raw_liquidity(e);

        if delta >= 0 {
            let inc = delta as u128;
            set_user_raw_liquidity(e, user, prev_user_raw.saturating_add(inc));
            set_total_raw_liquidity(e, &prev_total_raw.saturating_add(inc));
        } else {
            let dec = (-delta) as u128;
            set_user_raw_liquidity(e, user, prev_user_raw.saturating_sub(dec));
            set_total_raw_liquidity(e, &prev_total_raw.saturating_sub(dec));
        }
    }

    fn rewards_manager(e: &Env) -> Rewards {
        Rewards::new(e, 100)
    }

    fn rewards_checkpoint_user(e: &Env, user: &Address) {
        let rewards = Self::rewards_manager(e);
        let total_weighted = get_total_weighted_liquidity(e);
        let user_weighted = get_user_weighted_liquidity(e, user);

        let mut manager = rewards.manager();
        rewards_gauge::operations::checkpoint_user(
            e,
            user,
            manager.get_working_balance(user, user_weighted),
            manager.get_working_supply(total_weighted),
        );
        manager.checkpoint_user(user, total_weighted, user_weighted);
    }

    fn rewards_refresh_working_balance(e: &Env, user: &Address) {
        let rewards = Self::rewards_manager(e);
        let total_weighted = get_total_weighted_liquidity(e);
        let user_weighted = get_user_weighted_liquidity(e, user);

        let mut manager = rewards.manager();
        rewards_gauge::operations::checkpoint_user(
            e,
            user,
            manager.get_working_balance(user, 0),
            manager.get_working_supply(0),
        );
        manager.update_working_balance(user, total_weighted, user_weighted);
    }

    fn compute_fee_growth_inside(
        e: &Env,
        tick_lower: i32,
        tick_upper: i32,
        tick_current: i32,
    ) -> (U256, U256) {
        let fee_growth_global_0 = get_fee_growth_global_0_x128(e);
        let fee_growth_global_1 = get_fee_growth_global_1_x128(e);

        let lower = get_tick(e, tick_lower);
        let upper = get_tick(e, tick_upper);

        let fee_growth_below_0 = if tick_current >= tick_lower {
            lower.fee_growth_outside_0_x128
        } else {
            fee_growth_global_0.sub(&lower.fee_growth_outside_0_x128)
        };
        let fee_growth_below_1 = if tick_current >= tick_lower {
            lower.fee_growth_outside_1_x128
        } else {
            fee_growth_global_1.sub(&lower.fee_growth_outside_1_x128)
        };

        let fee_growth_above_0 = if tick_current < tick_upper {
            upper.fee_growth_outside_0_x128
        } else {
            fee_growth_global_0.sub(&upper.fee_growth_outside_0_x128)
        };
        let fee_growth_above_1 = if tick_current < tick_upper {
            upper.fee_growth_outside_1_x128
        } else {
            fee_growth_global_1.sub(&upper.fee_growth_outside_1_x128)
        };

        (
            fee_growth_global_0
                .sub(&fee_growth_below_0)
                .sub(&fee_growth_above_0),
            fee_growth_global_1
                .sub(&fee_growth_below_1)
                .sub(&fee_growth_above_1),
        )
    }

    fn accrue_position_fees(
        e: &Env,
        position: &mut PositionData,
        tick_lower: i32,
        tick_upper: i32,
        tick_current: i32,
    ) -> Result<(), Error> {
        let (inside_0, inside_1) =
            Self::compute_fee_growth_inside(e, tick_lower, tick_upper, tick_current);

        let delta_0 = inside_0.sub(&position.fee_growth_inside_0_last_x128);
        let delta_1 = inside_1.sub(&position.fee_growth_inside_1_last_x128);

        let owed_0 = mul_div_fee_growth(e, &delta_0, position.liquidity)?;
        let owed_1 = mul_div_fee_growth(e, &delta_1, position.liquidity)?;

        position.tokens_owed_0 = position.tokens_owed_0.saturating_add(owed_0);
        position.tokens_owed_1 = position.tokens_owed_1.saturating_add(owed_1);
        position.fee_growth_inside_0_last_x128 = inside_0;
        position.fee_growth_inside_1_last_x128 = inside_1;

        Ok(())
    }

    fn get_or_create_position(
        e: &Env,
        owner: &Address,
        tick_lower: i32,
        tick_upper: i32,
    ) -> PositionData {
        if let Some(position) = get_position(e, owner, tick_lower, tick_upper) {
            return position;
        }

        let tick_current = get_slot0(e).tick;
        let (inside_0, inside_1) =
            Self::compute_fee_growth_inside(e, tick_lower, tick_upper, tick_current);

        PositionData {
            fee_growth_inside_0_last_x128: inside_0,
            fee_growth_inside_1_last_x128: inside_1,
            liquidity: 0,
            tokens_owed_0: 0,
            tokens_owed_1: 0,
        }
    }

    fn collect_internal(
        e: &Env,
        owner: &Address,
        recipient: &Address,
        tick_lower: i32,
        tick_upper: i32,
        amount0_requested: u128,
        amount1_requested: u128,
        require_owner_auth: bool,
    ) -> Result<(u128, u128), Error> {
        if require_owner_auth {
            owner.require_auth();
        }

        let mut position = match get_position(e, owner, tick_lower, tick_upper) {
            Some(pos) => pos,
            None => return Err(Error::PositionNotFound),
        };

        let tick_current = get_slot0(e).tick;
        Self::accrue_position_fees(e, &mut position, tick_lower, tick_upper, tick_current)?;

        let amount0 = position.tokens_owed_0.min(amount0_requested);
        let amount1 = position.tokens_owed_1.min(amount1_requested);

        position.tokens_owed_0 -= amount0;
        position.tokens_owed_1 -= amount1;

        if position.liquidity == 0 && position.tokens_owed_0 == 0 && position.tokens_owed_1 == 0 {
            remove_position(e, owner, tick_lower, tick_upper);
            Self::remove_user_range_if_empty(e, owner, tick_lower, tick_upper);
        } else {
            set_position(e, owner, tick_lower, tick_upper, &position);
        }

        let token0 = get_token0(e);
        let token1 = get_token1(e);
        let contract = e.current_contract_address();

        if amount0 > 0 {
            SorobanTokenClient::new(e, &token0).transfer(&contract, recipient, &(amount0 as i128));
        }
        if amount1 > 0 {
            SorobanTokenClient::new(e, &token1).transfer(&contract, recipient, &(amount1 as i128));
        }

        update_plane(e);

        Ok((amount0, amount1))
    }

    fn cross_tick(e: &Env, tick_idx: i32) -> i128 {
        let mut tick = get_tick(e, tick_idx);
        let fee_growth_global_0 = get_fee_growth_global_0_x128(e);
        let fee_growth_global_1 = get_fee_growth_global_1_x128(e);

        tick.fee_growth_outside_0_x128 = fee_growth_global_0.sub(&tick.fee_growth_outside_0_x128);
        tick.fee_growth_outside_1_x128 = fee_growth_global_1.sub(&tick.fee_growth_outside_1_x128);

        let liquidity_net = tick.liquidity_net;
        set_tick(e, tick_idx, &tick);
        liquidity_net
    }

    fn add_fee_growth_global(
        e: &Env,
        zero_for_one: bool,
        fee_amount_for_lp: u128,
        liquidity: u128,
    ) -> Result<(), Error> {
        if fee_amount_for_lp == 0 || liquidity == 0 {
            return Ok(());
        }

        let growth_delta = fee_growth_delta_x128(e, fee_amount_for_lp, liquidity)?;
        if zero_for_one {
            let next = get_fee_growth_global_0_x128(e).add(&growth_delta);
            set_fee_growth_global_0_x128(e, &next);
        } else {
            let next = get_fee_growth_global_1_x128(e).add(&growth_delta);
            set_fee_growth_global_1_x128(e, &next);
        }

        Ok(())
    }

    fn midpoint(a: &U256, b: &U256) -> U256 {
        a.add(b).shr(1)
    }

    fn solve_sqrt_for_input(
        e: &Env,
        sqrt_current: &U256,
        sqrt_target: &U256,
        liquidity: u128,
        amount_in: u128,
        zero_for_one: bool,
    ) -> Result<U256, Error> {
        if amount_in == 0 || sqrt_current == sqrt_target {
            return Ok(sqrt_current.clone());
        }

        let one = U256::from_u32(e, 1);

        if zero_for_one {
            let mut low = sqrt_target.clone();
            let mut high = sqrt_current.clone();

            while high > low.add(&one) {
                let mid = Self::midpoint(&low, &high);
                let required = amount0_delta(e, &mid, sqrt_current, liquidity, true)?;
                if required > amount_in {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Ok(high)
        } else {
            let mut low = sqrt_current.clone();
            let mut high = sqrt_target.clone();

            while high > low.add(&one) {
                let mid = Self::midpoint(&low, &high);
                let required = amount1_delta(e, sqrt_current, &mid, liquidity, true)?;
                if required <= amount_in {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Ok(low)
        }
    }

    fn solve_sqrt_for_output(
        e: &Env,
        sqrt_current: &U256,
        sqrt_target: &U256,
        liquidity: u128,
        amount_out: u128,
        zero_for_one: bool,
    ) -> Result<U256, Error> {
        if amount_out == 0 || sqrt_current == sqrt_target {
            return Ok(sqrt_current.clone());
        }

        let one = U256::from_u32(e, 1);

        if zero_for_one {
            let mut low = sqrt_target.clone();
            let mut high = sqrt_current.clone();

            while high > low.add(&one) {
                let mid = Self::midpoint(&low, &high);
                let produced = amount1_delta(e, &mid, sqrt_current, liquidity, false)?;
                if produced > amount_out {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Ok(high)
        } else {
            let mut low = sqrt_current.clone();
            let mut high = sqrt_target.clone();

            while high > low.add(&one) {
                let mid = Self::midpoint(&low, &high);
                let produced = amount0_delta(e, sqrt_current, &mid, liquidity, false)?;
                if produced <= amount_out {
                    low = mid;
                } else {
                    high = mid;
                }
            }
            Ok(low)
        }
    }

    fn compute_swap_step(
        e: &Env,
        sqrt_current: &U256,
        sqrt_target: &U256,
        liquidity: u128,
        amount_remaining: u128,
        fee_pips: u32,
        zero_for_one: bool,
        exact_input: bool,
    ) -> Result<SwapStep, Error> {
        if liquidity == 0 {
            return Ok(SwapStep {
                sqrt_next: sqrt_current.clone(),
                amount_in: 0,
                amount_out: 0,
                fee_amount: 0,
            });
        }

        let fee = fee_pips as u128;
        let fee_complement = FEE_DENOMINATOR - fee;

        if exact_input {
            let amount_remaining_less_fee =
                mul_div_u128(e, amount_remaining, fee_complement, FEE_DENOMINATOR, false)?;

            let amount_in_to_target = if zero_for_one {
                amount0_delta(e, sqrt_target, sqrt_current, liquidity, true)?
            } else {
                amount1_delta(e, sqrt_current, sqrt_target, liquidity, true)?
            };

            let sqrt_next = if amount_remaining_less_fee >= amount_in_to_target {
                sqrt_target.clone()
            } else {
                Self::solve_sqrt_for_input(
                    e,
                    sqrt_current,
                    sqrt_target,
                    liquidity,
                    amount_remaining_less_fee,
                    zero_for_one,
                )?
            };

            let max_reached = sqrt_next == *sqrt_target;

            let amount_in = if zero_for_one {
                amount0_delta(e, &sqrt_next, sqrt_current, liquidity, true)?
            } else {
                amount1_delta(e, sqrt_current, &sqrt_next, liquidity, true)?
            };

            let amount_out = if zero_for_one {
                amount1_delta(e, &sqrt_next, sqrt_current, liquidity, false)?
            } else {
                amount0_delta(e, sqrt_current, &sqrt_next, liquidity, false)?
            };

            let fee_amount = if max_reached {
                mul_div_u128(e, amount_in, fee, fee_complement, true)?
            } else {
                amount_remaining.saturating_sub(amount_in)
            };

            Ok(SwapStep {
                sqrt_next,
                amount_in,
                amount_out,
                fee_amount,
            })
        } else {
            let amount_out_to_target = if zero_for_one {
                amount1_delta(e, sqrt_target, sqrt_current, liquidity, false)?
            } else {
                amount0_delta(e, sqrt_current, sqrt_target, liquidity, false)?
            };

            let sqrt_next = if amount_remaining >= amount_out_to_target {
                sqrt_target.clone()
            } else {
                Self::solve_sqrt_for_output(
                    e,
                    sqrt_current,
                    sqrt_target,
                    liquidity,
                    amount_remaining,
                    zero_for_one,
                )?
            };

            let amount_in = if zero_for_one {
                amount0_delta(e, &sqrt_next, sqrt_current, liquidity, true)?
            } else {
                amount1_delta(e, sqrt_current, &sqrt_next, liquidity, true)?
            };

            let mut amount_out = if zero_for_one {
                amount1_delta(e, &sqrt_next, sqrt_current, liquidity, false)?
            } else {
                amount0_delta(e, sqrt_current, &sqrt_next, liquidity, false)?
            };

            if amount_out > amount_remaining {
                amount_out = amount_remaining;
            }

            let fee_amount = mul_div_u128(e, amount_in, fee, fee_complement, true)?;

            Ok(SwapStep {
                sqrt_next,
                amount_in,
                amount_out,
                fee_amount,
            })
        }
    }

    fn validate_price_limit(
        e: &Env,
        slot: &Slot0,
        zero_for_one: bool,
        sqrt_price_limit_x96: U256,
    ) -> Result<U256, Error> {
        let min = min_sqrt_ratio(e);
        let max = max_sqrt_ratio(e);
        let zero = U256::from_u32(e, 0);

        let limit = if sqrt_price_limit_x96 == zero {
            if zero_for_one {
                min.add(&U256::from_u32(e, 1))
            } else {
                max.sub(&U256::from_u32(e, 1))
            }
        } else {
            sqrt_price_limit_x96
        };

        if zero_for_one {
            if limit <= min || limit >= slot.sqrt_price_x96 {
                return Err(Error::InvalidPriceLimit);
            }
        } else if limit >= max || limit <= slot.sqrt_price_x96 {
            return Err(Error::InvalidPriceLimit);
        }

        Ok(limit)
    }

    fn direction_from_indexes(in_idx: u32, out_idx: u32) -> Result<bool, Error> {
        if in_idx > 1 || out_idx > 1 || in_idx == out_idx {
            return Err(Error::InvalidAmount);
        }
        Ok(in_idx == 0 && out_idx == 1)
    }

    fn direction_from_tokens(
        e: &Env,
        token_in: &Address,
        token_out: &Address,
    ) -> Result<bool, Error> {
        if token_in == token_out {
            return Err(Error::InvalidAmount);
        }

        let token0 = get_token0(e);
        let token1 = get_token1(e);
        if *token_in == token0 && *token_out == token1 {
            return Ok(true);
        }
        if *token_in == token1 && *token_out == token0 {
            return Ok(false);
        }

        Err(Error::InvalidAmount)
    }

    fn full_range_ticks(e: &Env) -> Result<(i32, i32), Error> {
        let spacing = get_tick_spacing(e);
        if spacing <= 0 {
            return Err(Error::InvalidTickSpacing);
        }

        let mut tick_lower = MIN_TICK - (MIN_TICK % spacing);
        if tick_lower < MIN_TICK {
            tick_lower = tick_lower.saturating_add(spacing);
        }

        let mut tick_upper = MAX_TICK - (MAX_TICK % spacing);
        if tick_upper > MAX_TICK {
            tick_upper = tick_upper.saturating_sub(spacing);
        }

        Self::check_ticks_internal(e, tick_lower, tick_upper)?;
        Ok((tick_lower, tick_upper))
    }

    fn amounts_for_liquidity(
        e: &Env,
        slot: &Slot0,
        tick_lower: i32,
        tick_upper: i32,
        liquidity: u128,
        round_up: bool,
    ) -> Result<(u128, u128), Error> {
        let sqrt_lower = sqrt_ratio_at_tick(e, tick_lower)?;
        let sqrt_upper = sqrt_ratio_at_tick(e, tick_upper)?;

        if slot.sqrt_price_x96 <= sqrt_lower {
            Ok((
                amount0_delta(e, &sqrt_lower, &sqrt_upper, liquidity, round_up)?,
                0,
            ))
        } else if slot.sqrt_price_x96 < sqrt_upper {
            Ok((
                amount0_delta(e, &slot.sqrt_price_x96, &sqrt_upper, liquidity, round_up)?,
                amount1_delta(e, &sqrt_lower, &slot.sqrt_price_x96, liquidity, round_up)?,
            ))
        } else {
            Ok((
                0,
                amount1_delta(e, &sqrt_lower, &sqrt_upper, liquidity, round_up)?,
            ))
        }
    }

    fn max_liquidity_for_amounts(
        e: &Env,
        tick_lower: i32,
        tick_upper: i32,
        desired_amount0: u128,
        desired_amount1: u128,
    ) -> Result<u128, Error> {
        if desired_amount0 == 0 && desired_amount1 == 0 {
            return Ok(0);
        }

        let slot = get_slot0(e);
        let mut liquidity = if desired_amount0 == 0 {
            desired_amount1
        } else if desired_amount1 == 0 {
            desired_amount0
        } else {
            desired_amount0.min(desired_amount1)
        };

        while liquidity > 0 {
            match Self::amounts_for_liquidity(e, &slot, tick_lower, tick_upper, liquidity, true) {
                Ok((amount0, amount1))
                    if amount0 <= desired_amount0 && amount1 <= desired_amount1 =>
                {
                    return Ok(liquidity);
                }
                _ => {
                    // Conservative fallback to avoid overflow-prone upper-bound search.
                    liquidity >>= 1;
                }
            }
        }

        Ok(0)
    }

    fn simulate_swap_amounts(
        e: &Env,
        zero_for_one: bool,
        amount_specified: i128,
        sqrt_price_limit_x96: U256,
    ) -> Result<(i128, i128), Error> {
        if amount_specified == 0 {
            return Err(Error::InvalidAmount);
        }

        let exact_input = amount_specified > 0;
        let fee = get_fee(e);

        let mut slot = get_slot0(e);
        let price_limit = Self::validate_price_limit(e, &slot, zero_for_one, sqrt_price_limit_x96)?;
        let mut liquidity = get_liquidity(e);

        let mut amount_remaining = Self::abs_i128(amount_specified);
        let mut amount_calculated: u128 = 0;
        let tick_spacing = get_tick_spacing(e);

        while amount_remaining > 0 && slot.sqrt_price_x96 != price_limit {
            if liquidity == 0 {
                break;
            }

            let (next_tick, next_tick_initialized) = Self::next_initialized_tick_within_one_word(
                e,
                slot.tick,
                tick_spacing,
                zero_for_one,
            );
            let next_tick_price = sqrt_ratio_at_tick(e, next_tick)?;

            let sqrt_target = if zero_for_one {
                if next_tick_price < price_limit {
                    price_limit.clone()
                } else {
                    next_tick_price.clone()
                }
            } else if next_tick_price > price_limit {
                price_limit.clone()
            } else {
                next_tick_price.clone()
            };

            let step = Self::compute_swap_step(
                e,
                &slot.sqrt_price_x96,
                &sqrt_target,
                liquidity,
                amount_remaining,
                fee,
                zero_for_one,
                exact_input,
            )?;

            if step.amount_in == 0 && step.amount_out == 0 && step.fee_amount == 0 {
                // Empty word boundary can produce a zero-step at the current price.
                // Move the tick cursor forward to continue scanning initialized ticks.
                if slot.sqrt_price_x96 == sqrt_target {
                    slot.tick = if zero_for_one {
                        next_tick.saturating_sub(1).max(MIN_TICK)
                    } else {
                        next_tick.min(MAX_TICK)
                    };
                    if (zero_for_one && slot.tick == MIN_TICK)
                        || (!zero_for_one && slot.tick == MAX_TICK)
                    {
                        break;
                    }
                    continue;
                }
                break;
            }

            if exact_input {
                amount_remaining = amount_remaining
                    .saturating_sub(step.amount_in)
                    .saturating_sub(step.fee_amount);
                amount_calculated = amount_calculated.saturating_add(step.amount_out);
            } else {
                amount_remaining = amount_remaining.saturating_sub(step.amount_out);
                amount_calculated = amount_calculated
                    .saturating_add(step.amount_in)
                    .saturating_add(step.fee_amount);
            }

            slot.sqrt_price_x96 = step.sqrt_next;

            if slot.sqrt_price_x96 == sqrt_target {
                if next_tick_initialized {
                    let mut liquidity_net = get_tick(e, next_tick).liquidity_net;
                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }
                    if liquidity_net < 0 {
                        let dec = (-liquidity_net) as u128;
                        if liquidity < dec {
                            return Err(Error::LiquidityUnderflow);
                        }
                        liquidity -= dec;
                    } else {
                        liquidity = liquidity.saturating_add(liquidity_net as u128);
                    }
                }

                slot.tick = if zero_for_one {
                    next_tick.saturating_sub(1).max(MIN_TICK)
                } else {
                    next_tick.min(MAX_TICK)
                };
            } else {
                slot.tick = tick_at_sqrt_ratio(e, &slot.sqrt_price_x96)?;
            }
        }

        if !exact_input && amount_remaining > 0 {
            return Err(Error::InsufficientLiquidity);
        }

        let original_spec = Self::abs_i128(amount_specified);
        let amount_spec_used = original_spec.saturating_sub(amount_remaining);

        if zero_for_one {
            if exact_input {
                Ok((
                    Self::u128_to_i128(amount_spec_used)?,
                    -Self::u128_to_i128(amount_calculated)?,
                ))
            } else {
                Ok((
                    Self::u128_to_i128(amount_calculated)?,
                    -Self::u128_to_i128(amount_spec_used)?,
                ))
            }
        } else if exact_input {
            Ok((
                -Self::u128_to_i128(amount_calculated)?,
                Self::u128_to_i128(amount_spec_used)?,
            ))
        } else {
            Ok((
                -Self::u128_to_i128(amount_spec_used)?,
                Self::u128_to_i128(amount_calculated)?,
            ))
        }
    }

    fn swap_internal(
        e: &Env,
        sender: &Address,
        recipient: &Address,
        zero_for_one: bool,
        amount_specified: i128,
        sqrt_price_limit_x96: U256,
    ) -> Result<SwapResult, Error> {
        if amount_specified == 0 {
            return Err(Error::InvalidAmount);
        }

        let exact_input = amount_specified > 0;
        let fee = get_fee(e);

        let mut slot = get_slot0(e);
        let price_limit = Self::validate_price_limit(e, &slot, zero_for_one, sqrt_price_limit_x96)?;

        let mut liquidity = get_liquidity(e);
        let mut protocol_fees = get_protocol_fees(e);

        let mut amount_remaining = Self::abs_i128(amount_specified);
        let mut amount_calculated: u128 = 0;
        let tick_spacing = get_tick_spacing(e);

        while amount_remaining > 0 && slot.sqrt_price_x96 != price_limit {
            if liquidity == 0 {
                break;
            }

            let (next_tick, next_tick_initialized) = Self::next_initialized_tick_within_one_word(
                e,
                slot.tick,
                tick_spacing,
                zero_for_one,
            );
            let next_tick_price = sqrt_ratio_at_tick(e, next_tick)?;

            let sqrt_target = if zero_for_one {
                if next_tick_price < price_limit {
                    price_limit.clone()
                } else {
                    next_tick_price.clone()
                }
            } else if next_tick_price > price_limit {
                price_limit.clone()
            } else {
                next_tick_price.clone()
            };

            let step = Self::compute_swap_step(
                e,
                &slot.sqrt_price_x96,
                &sqrt_target,
                liquidity,
                amount_remaining,
                fee,
                zero_for_one,
                exact_input,
            )?;

            if step.amount_in == 0 && step.amount_out == 0 && step.fee_amount == 0 {
                // Empty word boundary can produce a zero-step at the current price.
                // Move the tick cursor forward to continue scanning initialized ticks.
                if slot.sqrt_price_x96 == sqrt_target {
                    slot.tick = if zero_for_one {
                        next_tick.saturating_sub(1).max(MIN_TICK)
                    } else {
                        next_tick.min(MAX_TICK)
                    };
                    if (zero_for_one && slot.tick == MIN_TICK)
                        || (!zero_for_one && slot.tick == MAX_TICK)
                    {
                        break;
                    }
                    continue;
                }
                break;
            }

            if exact_input {
                amount_remaining = amount_remaining
                    .saturating_sub(step.amount_in)
                    .saturating_sub(step.fee_amount);
                amount_calculated = amount_calculated.saturating_add(step.amount_out);
            } else {
                amount_remaining = amount_remaining.saturating_sub(step.amount_out);
                amount_calculated = amount_calculated
                    .saturating_add(step.amount_in)
                    .saturating_add(step.fee_amount);
            }

            let protocol_cut =
                step.fee_amount * get_protocol_fee_fraction(e) as u128 / FEE_DENOMINATOR;
            let fee_for_lp = step.fee_amount.saturating_sub(protocol_cut);
            if zero_for_one {
                protocol_fees.token0 = protocol_fees.token0.saturating_add(protocol_cut);
            } else {
                protocol_fees.token1 = protocol_fees.token1.saturating_add(protocol_cut);
            }

            Self::add_fee_growth_global(e, zero_for_one, fee_for_lp, liquidity)?;

            slot.sqrt_price_x96 = step.sqrt_next;

            if slot.sqrt_price_x96 == sqrt_target {
                if next_tick_initialized {
                    let mut liquidity_net = Self::cross_tick(e, next_tick);
                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }

                    if liquidity_net < 0 {
                        let dec = (-liquidity_net) as u128;
                        if liquidity < dec {
                            return Err(Error::LiquidityUnderflow);
                        }
                        liquidity -= dec;
                    } else {
                        liquidity = liquidity.saturating_add(liquidity_net as u128);
                    }
                }

                slot.tick = if zero_for_one {
                    next_tick.saturating_sub(1).max(MIN_TICK)
                } else {
                    next_tick.min(MAX_TICK)
                };
            } else {
                slot.tick = tick_at_sqrt_ratio(e, &slot.sqrt_price_x96)?;
            }
        }

        if !exact_input && amount_remaining > 0 {
            return Err(Error::InsufficientLiquidity);
        }

        set_protocol_fees(e, &protocol_fees);
        set_liquidity(e, &liquidity);
        set_slot0(e, &slot);
        update_plane(e);

        let original_spec = Self::abs_i128(amount_specified);
        let amount_spec_used = original_spec.saturating_sub(amount_remaining);

        let (amount0, amount1) = if zero_for_one {
            if exact_input {
                (
                    Self::u128_to_i128(amount_spec_used)?,
                    -Self::u128_to_i128(amount_calculated)?,
                )
            } else {
                (
                    Self::u128_to_i128(amount_calculated)?,
                    -Self::u128_to_i128(amount_spec_used)?,
                )
            }
        } else if exact_input {
            (
                -Self::u128_to_i128(amount_calculated)?,
                Self::u128_to_i128(amount_spec_used)?,
            )
        } else {
            (
                -Self::u128_to_i128(amount_spec_used)?,
                Self::u128_to_i128(amount_calculated)?,
            )
        };

        let token0 = get_token0(e);
        let token1 = get_token1(e);
        let contract = e.current_contract_address();

        if amount0 > 0 {
            SorobanTokenClient::new(e, &token0).transfer(sender, &contract, &amount0);
        }
        if amount1 > 0 {
            SorobanTokenClient::new(e, &token1).transfer(sender, &contract, &amount1);
        }

        if amount0 < 0 {
            SorobanTokenClient::new(e, &token0).transfer(&contract, recipient, &(-amount0));
        }
        if amount1 < 0 {
            SorobanTokenClient::new(e, &token1).transfer(&contract, recipient, &(-amount1));
        }

        Self::recompute_user_weighted_liquidity(e, sender);
        if sender != recipient {
            Self::recompute_user_weighted_liquidity(e, recipient);
        }

        Ok(SwapResult {
            amount0,
            amount1,
            liquidity,
            sqrt_price_x96: slot.sqrt_price_x96,
            tick: slot.tick,
        })
    }
}

#[contractimpl]
impl ManagedLiquidityPool for ConcentratedLiquidityPool {
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
    ) {
        let (reward_token, reward_boost_token, reward_boost_feed) = reward_config;
        Self::init_pools_plane(e.clone(), plane);
        Self::initialize(
            e.clone(),
            admin,
            privileged_addrs,
            router,
            tokens,
            fee,
            tick_spacing,
        );
        Self::initialize_boost_config(e.clone(), reward_boost_token, reward_boost_feed);
        Self::initialize_rewards_config(e, reward_token);
    }
}

#[contractimpl]
impl LiquidityPoolInterfaceTrait for ConcentratedLiquidityPool {
    fn pool_type(e: Env) -> Symbol {
        Symbol::new(&e, "concentrated")
    }

    fn initialize(
        e: Env,
        admin: Address,
        privileged_addrs: (Address, Address, Address, Address, Vec<Address>, Address),
        router: Address,
        tokens: Vec<Address>,
        fee: u32,
        tick_spacing: i32,
    ) {
        if Self::has_admin_role(&e) {
            panic_with_error!(&e, Error::PoolAlreadyInitialized);
        }
        if tokens.len() != 2 {
            panic_with_error!(&e, Error::InvalidTickRange);
        }
        if fee as u128 >= FEE_DENOMINATOR {
            panic_with_error!(&e, Error::InvalidFee);
        }
        if tick_spacing <= 0 {
            panic_with_error!(&e, Error::InvalidTickSpacing);
        }

        let token0 = tokens.get_unchecked(0);
        let token1 = tokens.get_unchecked(1);
        if token0 == token1 {
            panic_with_error!(&e, Error::InvalidTickRange);
        }

        let access_control = AccessControl::new(&e);
        access_control.set_role_address(&Role::Admin, &admin);
        access_control.set_role_address(&Role::EmergencyAdmin, &privileged_addrs.0);
        access_control.set_role_address(&Role::RewardsAdmin, &privileged_addrs.1);
        access_control.set_role_address(&Role::OperationsAdmin, &privileged_addrs.2);
        access_control.set_role_address(&Role::PauseAdmin, &privileged_addrs.3);
        access_control.set_role_addresses(&Role::EmergencyPauseAdmin, &privileged_addrs.4);
        access_control.set_role_address(&Role::SystemFeeAdmin, &privileged_addrs.5);

        set_router(&e, &router);
        set_token0(&e, &token0);
        set_token1(&e, &token1);
        set_fee(&e, &fee);
        set_tick_spacing(&e, &tick_spacing);

        set_liquidity(&e, &0);
        set_fee_growth_global_0_x128(&e, &U256::from_u32(&e, 0));
        set_fee_growth_global_1_x128(&e, &U256::from_u32(&e, 0));
        set_protocol_fees(
            &e,
            &ProtocolFees {
                token0: 0,
                token1: 0,
            },
        );
        set_protocol_fee_fraction(&e, &5_000);
        set_is_killed_deposit(&e, &false);
        set_is_killed_swap(&e, &false);
        set_claim_killed(&e, &false);
        set_distance_weight_config(
            &e,
            &DistanceWeightConfig {
                max_distance_ticks: 5_000,
                min_multiplier_bps: 0,
            },
        );

        let sqrt_price_x96 = sqrt_ratio_at_tick(&e, 0).unwrap();
        set_slot0(
            &e,
            &Slot0 {
                sqrt_price_x96,
                tick: 0,
            },
        );

        update_plane(&e);
    }

    fn get_fee_fraction(e: Env) -> u32 {
        get_fee(&e)
    }

    fn get_protocol_fee_fraction(e: Env) -> u32 {
        get_protocol_fee_fraction(&e)
    }

    fn share_id(e: Env) -> Address {
        e.current_contract_address()
    }

    fn get_total_shares(e: Env) -> u128 {
        get_total_raw_liquidity(&e)
    }

    fn get_user_shares(e: Env, user: Address) -> u128 {
        get_user_raw_liquidity(&e, &user)
    }

    fn get_reserves(e: Env) -> Vec<u128> {
        let contract = e.current_contract_address();
        let fees = get_protocol_fees(&e);
        let balance0 = SorobanTokenClient::new(&e, &get_token0(&e)).balance(&contract) as u128;
        let balance1 = SorobanTokenClient::new(&e, &get_token1(&e)).balance(&contract) as u128;
        Vec::from_array(
            &e,
            [
                balance0.saturating_sub(fees.token0),
                balance1.saturating_sub(fees.token1),
            ],
        )
    }

    fn get_tokens(e: Env) -> Vec<Address> {
        Vec::from_array(&e, [get_token0(&e), get_token1(&e)])
    }

    fn deposit(
        e: Env,
        user: Address,
        desired_amounts: Vec<u128>,
        min_shares: u128,
    ) -> (Vec<u128>, u128) {
        if desired_amounts.len() != 2 {
            panic_with_error!(&e, Error::InvalidAmount);
        }

        let (tick_lower, tick_upper) = match Self::full_range_ticks(&e) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let desired_amount0 = desired_amounts.get_unchecked(0);
        let desired_amount1 = desired_amounts.get_unchecked(1);

        let liquidity = match Self::max_liquidity_for_amounts(
            &e,
            tick_lower,
            tick_upper,
            desired_amount0,
            desired_amount1,
        ) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        if liquidity == 0 {
            panic_with_error!(&e, Error::AmountShouldBeGreaterThanZero);
        }
        if liquidity < min_shares {
            panic_with_error!(&e, Error::InvalidAmount);
        }

        let (amount0, amount1) = match Self::deposit_position(
            e.clone(),
            user.clone(),
            user.clone(),
            tick_lower,
            tick_upper,
            liquidity,
        ) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        (Vec::from_array(&e, [amount0, amount1]), liquidity)
    }

    fn estimate_deposit(e: Env, desired_amounts: Vec<u128>) -> u128 {
        if desired_amounts.len() != 2 {
            panic_with_error!(&e, Error::InvalidAmount);
        }

        let (tick_lower, tick_upper) = match Self::full_range_ticks(&e) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        match Self::max_liquidity_for_amounts(
            &e,
            tick_lower,
            tick_upper,
            desired_amounts.get_unchecked(0),
            desired_amounts.get_unchecked(1),
        ) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        }
    }

    fn swap(
        e: Env,
        user: Address,
        in_idx: u32,
        out_idx: u32,
        in_amount: u128,
        out_min: u128,
    ) -> u128 {
        let zero_for_one = match Self::direction_from_indexes(in_idx, out_idx) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let amount_specified = match Self::u128_to_i128(in_amount) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        let token_in = if zero_for_one {
            get_token0(&e)
        } else {
            get_token1(&e)
        };
        let token_out = if zero_for_one {
            get_token1(&e)
        } else {
            get_token0(&e)
        };

        let result = match Self::swap_by_tokens(
            e.clone(),
            user.clone(),
            user.clone(),
            token_in,
            token_out,
            amount_specified,
            U256::from_u32(&e, 0),
        ) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let amount_out = if zero_for_one {
            if result.amount1 > 0 {
                panic_with_error!(&e, Error::InvalidAmount);
            }
            (-result.amount1) as u128
        } else {
            if result.amount0 > 0 {
                panic_with_error!(&e, Error::InvalidAmount);
            }
            (-result.amount0) as u128
        };

        if amount_out < out_min {
            panic_with_error!(&e, Error::InvalidAmount);
        }

        amount_out
    }

    fn estimate_swap(e: Env, in_idx: u32, out_idx: u32, in_amount: u128) -> u128 {
        let zero_for_one = match Self::direction_from_indexes(in_idx, out_idx) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        let amount_specified = match Self::u128_to_i128(in_amount) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let (amount0, amount1) = match Self::simulate_swap_amounts(
            &e,
            zero_for_one,
            amount_specified,
            U256::from_u32(&e, 0),
        ) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        if zero_for_one {
            if amount1 > 0 {
                panic_with_error!(&e, Error::InvalidAmount);
            }
            (-amount1) as u128
        } else {
            if amount0 > 0 {
                panic_with_error!(&e, Error::InvalidAmount);
            }
            (-amount0) as u128
        }
    }

    fn swap_strict_receive(
        e: Env,
        user: Address,
        in_idx: u32,
        out_idx: u32,
        out_amount: u128,
        in_max: u128,
    ) -> u128 {
        let zero_for_one = match Self::direction_from_indexes(in_idx, out_idx) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let out_amount_i128 = match Self::u128_to_i128(out_amount) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        let token_in = if zero_for_one {
            get_token0(&e)
        } else {
            get_token1(&e)
        };
        let token_out = if zero_for_one {
            get_token1(&e)
        } else {
            get_token0(&e)
        };

        let result = match Self::swap_by_tokens(
            e.clone(),
            user.clone(),
            user.clone(),
            token_in,
            token_out,
            -out_amount_i128,
            U256::from_u32(&e, 0),
        ) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let amount_in = if zero_for_one {
            if result.amount0 <= 0 {
                panic_with_error!(&e, Error::InvalidAmount);
            }
            result.amount0 as u128
        } else {
            if result.amount1 <= 0 {
                panic_with_error!(&e, Error::InvalidAmount);
            }
            result.amount1 as u128
        };

        if amount_in > in_max {
            panic_with_error!(&e, Error::InvalidAmount);
        }

        amount_in
    }

    fn estimate_swap_strict_receive(e: Env, in_idx: u32, out_idx: u32, out_amount: u128) -> u128 {
        let zero_for_one = match Self::direction_from_indexes(in_idx, out_idx) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        let out_amount_i128 = match Self::u128_to_i128(out_amount) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let (amount0, amount1) = match Self::simulate_swap_amounts(
            &e,
            zero_for_one,
            -out_amount_i128,
            U256::from_u32(&e, 0),
        ) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        if zero_for_one {
            if amount0 <= 0 {
                panic_with_error!(&e, Error::InvalidAmount);
            }
            amount0 as u128
        } else {
            if amount1 <= 0 {
                panic_with_error!(&e, Error::InvalidAmount);
            }
            amount1 as u128
        }
    }

    fn withdraw(e: Env, user: Address, share_amount: u128, min_amounts: Vec<u128>) -> Vec<u128> {
        if min_amounts.len() != 2 {
            panic_with_error!(&e, Error::InvalidAmount);
        }

        let (tick_lower, tick_upper) = match Self::full_range_ticks(&e) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let (burn_amount0, burn_amount1) = match Self::withdraw_position(
            e.clone(),
            user.clone(),
            tick_lower,
            tick_upper,
            share_amount,
        ) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let (amount0, amount1) = match Self::collect_internal(
            &e,
            &user,
            &user,
            tick_lower,
            tick_upper,
            burn_amount0,
            burn_amount1,
            false,
        ) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        if amount0 < min_amounts.get_unchecked(0) || amount1 < min_amounts.get_unchecked(1) {
            panic_with_error!(&e, Error::InvalidAmount);
        }

        Vec::from_array(&e, [amount0, amount1])
    }

    fn get_info(e: Env) -> Map<Symbol, Val> {
        let mut result = Map::new(&e);
        result.set(
            symbol_short!("pool_type"),
            Self::pool_type(e.clone()).into_val(&e),
        );
        result.set(
            symbol_short!("fee"),
            Self::get_fee_fraction(e.clone()).into_val(&e),
        );
        result.set(
            Symbol::new(&e, "tick_spacing"),
            get_tick_spacing(&e).into_val(&e),
        );
        result
    }

    fn get_total_excluded_shares(e: Env) -> u128 {
        Self::rewards_manager(&e)
            .storage()
            .get_total_excluded_shares()
    }
}

#[contractimpl]
impl RewardsTrait for ConcentratedLiquidityPool {
    fn initialize_rewards_config(e: Env, reward_token: Address) {
        let rewards = Self::rewards_manager(&e);
        if rewards.storage().has_reward_token() {
            panic_with_error!(&e, Error::RewardsAlreadyInitialized);
        }
        rewards.storage().put_reward_token(reward_token);
    }

    fn initialize_boost_config(e: Env, reward_boost_token: Address, reward_boost_feed: Address) {
        let rewards = Self::rewards_manager(&e);
        if rewards.storage().has_reward_boost_token() {
            panic_with_error!(&e, Error::RewardsAlreadyInitialized);
        }
        rewards.storage().put_reward_boost_token(reward_boost_token);
        rewards.storage().put_reward_boost_feed(reward_boost_feed);
    }

    fn set_rewards_config(e: Env, admin: Address, expired_at: u64, tps: u128) {
        admin.require_auth();
        if admin != get_router(&e) {
            require_rewards_admin_or_owner(&e, &admin);
        }
        let mut manager = Self::rewards_manager(&e).manager();
        manager.set_reward_config(get_total_weighted_liquidity(&e), expired_at, tps);
    }

    fn get_rewards_info(e: Env, user: Address) -> Map<Symbol, i128> {
        let rewards = Self::rewards_manager(&e);
        let storage = rewards.storage();
        let mut manager = rewards.manager();

        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);

        let user_data = manager.checkpoint_user(&user, total_weighted, user_weighted);
        let config = storage.get_pool_reward_config();

        map![
            &e,
            (Symbol::new(&e, "user_reward"), user_data.to_claim as i128),
            (Symbol::new(&e, "tps"), config.tps as i128),
            (Symbol::new(&e, "expired_at"), config.expired_at as i128),
            (
                Symbol::new(&e, "working_balance"),
                manager.get_working_balance(&user, user_weighted) as i128
            ),
            (
                Symbol::new(&e, "working_supply"),
                manager.get_working_supply(total_weighted) as i128
            ),
        ]
    }

    fn get_user_reward(e: Env, user: Address) -> u128 {
        Self::recompute_user_weighted_liquidity(&e, &user);
        Self::rewards_manager(&e).manager().get_amount_to_claim(
            &user,
            get_total_weighted_liquidity(&e),
            get_user_weighted_liquidity(&e, &user),
        )
    }

    fn get_total_accumulated_reward(e: Env) -> u128 {
        Self::rewards_manager(&e)
            .manager()
            .get_total_accumulated_reward(get_total_weighted_liquidity(&e))
    }

    fn get_total_configured_reward(e: Env) -> u128 {
        Self::rewards_manager(&e)
            .manager()
            .get_total_configured_reward(get_total_weighted_liquidity(&e))
    }

    fn get_total_claimed_reward(e: Env) -> u128 {
        Self::rewards_manager(&e)
            .manager()
            .get_total_claimed_reward(get_total_weighted_liquidity(&e))
    }

    fn claim(e: Env, user: Address) -> u128 {
        if get_claim_killed(&e) {
            panic_with_error!(&e, Error::ClaimKilled)
        }

        user.require_auth();

        Self::recompute_user_weighted_liquidity(&e, &user);

        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);

        let rewards = Self::rewards_manager(&e);
        let mut manager = rewards.manager();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            manager.get_working_balance(&user, user_weighted),
            manager.get_working_supply(total_weighted),
        );
        let reward = manager.claim_reward(&user, total_weighted, user_weighted);

        RewardEvents::new(&e).claim(user.clone(), rewards.storage().get_reward_token(), reward);

        let manager_after = rewards.manager();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            manager_after.get_working_balance(&user, user_weighted),
            manager_after.get_working_supply(total_weighted),
        );

        reward
    }
}

#[contractimpl]
impl AdminInterfaceTrait for ConcentratedLiquidityPool {
    fn set_distance_weighting(
        e: Env,
        admin: Address,
        max_distance_ticks: u32,
        min_multiplier_bps: u32,
    ) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);
        set_distance_weight_config(
            &e,
            &DistanceWeightConfig {
                max_distance_ticks,
                min_multiplier_bps,
            },
        );
    }

    fn get_distance_weighting(e: Env) -> DistanceWeightConfig {
        get_distance_weight_config(&e)
    }

    fn set_claim_killed(e: Env, admin: Address, value: bool) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_claim_killed(&e, &value);
    }

    fn get_claim_killed(e: Env) -> bool {
        get_claim_killed(&e)
    }

    fn set_privileged_addrs(
        e: Env,
        admin: Address,
        rewards_admin: Address,
        operations_admin: Address,
        pause_admin: Address,
        emergency_pause_admins: Vec<Address>,
        system_fee_admin: Address,
    ) {
        Self::require_admin(&e, &admin);
        let access_control = AccessControl::new(&e);
        access_control.set_role_address(&Role::RewardsAdmin, &rewards_admin);
        access_control.set_role_address(&Role::OperationsAdmin, &operations_admin);
        access_control.set_role_address(&Role::PauseAdmin, &pause_admin);
        access_control.set_role_addresses(&Role::EmergencyPauseAdmin, &emergency_pause_admins);
        access_control.set_role_address(&Role::SystemFeeAdmin, &system_fee_admin);
        AccessControlEvents::new(&e).set_privileged_addrs(
            rewards_admin,
            operations_admin,
            pause_admin,
            emergency_pause_admins,
            system_fee_admin,
        );
    }

    fn get_privileged_addrs(e: Env) -> Map<Symbol, Vec<Address>> {
        let access_control = AccessControl::new(&e);
        let mut result: Map<Symbol, Vec<Address>> = Map::new(&e);
        for role in [
            Role::Admin,
            Role::EmergencyAdmin,
            Role::RewardsAdmin,
            Role::OperationsAdmin,
            Role::PauseAdmin,
            Role::SystemFeeAdmin,
        ] {
            result.set(
                role.as_symbol(&e),
                match access_control.get_role_safe(&role) {
                    Some(v) => Vec::from_array(&e, [v]),
                    None => Vec::new(&e),
                },
            );
        }
        result.set(
            Role::EmergencyPauseAdmin.as_symbol(&e),
            access_control.get_role_addresses(&Role::EmergencyPauseAdmin),
        );
        result
    }

    fn kill_deposit(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_is_killed_deposit(&e, &true);
    }

    fn kill_swap(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_is_killed_swap(&e, &true);
    }

    fn kill_claim(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_claim_killed(&e, &true);
    }

    fn unkill_deposit(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_admin_or_owner(&e, &admin);
        set_is_killed_deposit(&e, &false);
    }

    fn unkill_swap(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_admin_or_owner(&e, &admin);
        set_is_killed_swap(&e, &false);
    }

    fn unkill_claim(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_admin_or_owner(&e, &admin);
        set_claim_killed(&e, &false);
    }

    fn get_is_killed_deposit(e: Env) -> bool {
        get_is_killed_deposit(&e)
    }

    fn get_is_killed_swap(e: Env) -> bool {
        get_is_killed_swap(&e)
    }

    fn get_is_killed_claim(e: Env) -> bool {
        get_claim_killed(&e)
    }

    fn set_protocol_fee_fraction(e: Env, admin: Address, new_fraction: u32) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);
        if new_fraction as u128 > FEE_DENOMINATOR {
            panic_with_error!(&e, Error::InvalidFee);
        }
        set_protocol_fee_fraction(&e, &new_fraction);
    }

    fn get_protocol_fees(e: Env) -> Vec<u128> {
        let fees = get_protocol_fees(&e);
        Vec::from_array(&e, [fees.token0, fees.token1])
    }

    fn claim_protocol_fees(e: Env, admin: Address, destination: Address) -> Vec<u128> {
        admin.require_auth();
        require_system_fee_admin_or_owner(&e, &admin);

        let mut fees = get_protocol_fees(&e);
        let amount0 = fees.token0;
        let amount1 = fees.token1;
        if amount0 == 0 && amount1 == 0 {
            return Vec::from_array(&e, [0, 0]);
        }

        let contract = e.current_contract_address();
        if amount0 > 0 {
            SorobanTokenClient::new(&e, &get_token0(&e)).transfer(
                &contract,
                &destination,
                &(amount0 as i128),
            );
            fees.token0 = 0;
        }
        if amount1 > 0 {
            SorobanTokenClient::new(&e, &get_token1(&e)).transfer(
                &contract,
                &destination,
                &(amount1 as i128),
            );
            fees.token1 = 0;
        }
        set_protocol_fees(&e, &fees);
        update_plane(&e);
        Vec::from_array(&e, [amount0, amount1])
    }
}

#[contractimpl]
impl UpgradeableContract for ConcentratedLiquidityPool {
    fn version() -> u32 {
        180
    }

    fn contract_name(e: Env) -> Symbol {
        Symbol::new(&e, "ConcentratedLiquidityPool")
    }

    fn commit_upgrade(
        e: Env,
        admin: Address,
        new_wasm_hash: BytesN<32>,
        token_new_wasm_hash: BytesN<32>,
        gauges_new_wasm_hash: BytesN<32>,
    ) {
        Self::require_admin(&e, &admin);
        commit_upgrade(&e, &new_wasm_hash);
        set_token_future_wasm(&e, &token_new_wasm_hash);
        set_gauge_future_wasm(&e, &gauges_new_wasm_hash);
        UpgradeEvents::new(&e).commit_upgrade(Vec::from_array(
            &e,
            [new_wasm_hash, token_new_wasm_hash, gauges_new_wasm_hash],
        ));
    }

    fn apply_upgrade(e: Env, admin: Address) -> (BytesN<32>, BytesN<32>) {
        Self::require_admin(&e, &admin);
        let new_wasm_hash = apply_upgrade(&e);
        let token_new_wasm_hash = get_token_future_wasm(&e);
        rewards_gauge::operations::upgrade(&e, &get_gauge_future_wasm(&e));
        UpgradeEvents::new(&e).apply_upgrade(Vec::from_array(
            &e,
            [new_wasm_hash.clone(), token_new_wasm_hash.clone()],
        ));
        (new_wasm_hash, token_new_wasm_hash)
    }

    fn revert_upgrade(e: Env, admin: Address) {
        Self::require_admin(&e, &admin);
        revert_upgrade(&e);
        UpgradeEvents::new(&e).revert_upgrade();
    }

    fn set_emergency_mode(e: Env, emergency_admin: Address, value: bool) {
        emergency_admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&emergency_admin, &Role::EmergencyAdmin);
        set_emergency_mode(&e, &value);
        AccessControlEvents::new(&e).set_emergency_mode(value);
    }

    fn get_emergency_mode(e: Env) -> bool {
        get_emergency_mode(&e)
    }
}

#[contractimpl]
impl Plane for ConcentratedLiquidityPool {
    fn init_pools_plane(e: Env, plane: Address) {
        set_plane(&e, &plane);
    }

    fn set_pools_plane(e: Env, admin: Address, plane: Address) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        set_plane(&e, &plane);
    }

    fn get_pools_plane(e: Env) -> Address {
        get_plane(&e)
    }

    fn backfill_plane_data(e: Env) {
        update_plane(&e);
    }
}

#[contractimpl]
impl TransferableContract for ConcentratedLiquidityPool {
    fn commit_transfer_ownership(e: Env, admin: Address, role_name: Symbol, new_address: Address) {
        Self::require_admin(&e, &admin);
        let role = Role::from_symbol(&e, role_name);
        let access_control = AccessControl::new(&e);
        access_control.commit_transfer_ownership(&role, &new_address);
        AccessControlEvents::new(&e).commit_transfer_ownership(role, new_address);
    }

    fn apply_transfer_ownership(e: Env, admin: Address, role_name: Symbol) {
        Self::require_admin(&e, &admin);
        let role = Role::from_symbol(&e, role_name);
        let access_control = AccessControl::new(&e);
        let new_address = access_control.apply_transfer_ownership(&role);
        AccessControlEvents::new(&e).apply_transfer_ownership(role, new_address);
    }

    fn revert_transfer_ownership(e: Env, admin: Address, role_name: Symbol) {
        Self::require_admin(&e, &admin);
        let role = Role::from_symbol(&e, role_name);
        let access_control = AccessControl::new(&e);
        access_control.revert_transfer_ownership(&role);
        AccessControlEvents::new(&e).revert_transfer_ownership(role);
    }

    fn get_future_address(e: Env, role_name: Symbol) -> Address {
        let role = Role::from_symbol(&e, role_name);
        AccessControl::new(&e).get_future_address(&role)
    }
}

#[contractimpl]
impl ConcentratedPoolExtensionsTrait for ConcentratedLiquidityPool {
    fn check_ticks(e: Env, tick_lower: i32, tick_upper: i32) -> Result<(), Error> {
        Self::check_ticks_internal(&e, tick_lower, tick_upper)
    }

    fn block_timestamp(e: Env) -> u64 {
        e.ledger().timestamp()
    }

    fn initialize_price(e: Env, admin: Address, sqrt_price_x96: U256) -> Result<(), Error> {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);

        if sqrt_price_x96 == U256::from_u32(&e, 0) {
            return Err(Error::InvalidSqrtPrice);
        }

        let tick = tick_at_sqrt_ratio(&e, &sqrt_price_x96)?;

        set_slot0(
            &e,
            &Slot0 {
                sqrt_price_x96,
                tick,
            },
        );
        update_plane(&e);
        Ok(())
    }

    fn swap_by_tokens(
        e: Env,
        sender: Address,
        recipient: Address,
        token_in: Address,
        token_out: Address,
        amount_specified: i128,
        sqrt_price_limit_x96: U256,
    ) -> Result<SwapResult, Error> {
        sender.require_auth();
        if get_is_killed_swap(&e) {
            return Err(Error::SwapKilled);
        }
        let zero_for_one = Self::direction_from_tokens(&e, &token_in, &token_out)?;
        Self::swap_internal(
            &e,
            &sender,
            &recipient,
            zero_for_one,
            amount_specified,
            sqrt_price_limit_x96,
        )
    }

    fn deposit_position(
        e: Env,
        sender: Address,
        recipient: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount: u128,
    ) -> Result<(u128, u128), Error> {
        sender.require_auth();
        if get_is_killed_deposit(&e) {
            return Err(Error::DepositKilled);
        }
        if amount == 0 {
            return Err(Error::AmountShouldBeGreaterThanZero);
        }

        Self::check_ticks_internal(&e, tick_lower, tick_upper)?;

        Self::recompute_user_weighted_liquidity(&e, &recipient);
        Self::rewards_checkpoint_user(&e, &recipient);

        let slot = get_slot0(&e);
        let sqrt_lower = sqrt_ratio_at_tick(&e, tick_lower)?;
        let sqrt_upper = sqrt_ratio_at_tick(&e, tick_upper)?;

        let (amount0, amount1) = if slot.sqrt_price_x96 <= sqrt_lower {
            (
                amount0_delta(&e, &sqrt_lower, &sqrt_upper, amount, true)?,
                0,
            )
        } else if slot.sqrt_price_x96 < sqrt_upper {
            (
                amount0_delta(&e, &slot.sqrt_price_x96, &sqrt_upper, amount, true)?,
                amount1_delta(&e, &sqrt_lower, &slot.sqrt_price_x96, amount, true)?,
            )
        } else {
            (
                0,
                amount1_delta(&e, &sqrt_lower, &sqrt_upper, amount, true)?,
            )
        };

        let token0 = get_token0(&e);
        let token1 = get_token1(&e);
        let contract = e.current_contract_address();

        if amount0 > 0 {
            SorobanTokenClient::new(&e, &token0).transfer(&sender, &contract, &(amount0 as i128));
        }
        if amount1 > 0 {
            SorobanTokenClient::new(&e, &token1).transfer(&sender, &contract, &(amount1 as i128));
        }

        let mut position = Self::get_or_create_position(&e, &recipient, tick_lower, tick_upper);
        Self::accrue_position_fees(&e, &mut position, tick_lower, tick_upper, slot.tick)?;
        position.liquidity = position.liquidity.saturating_add(amount);
        set_position(&e, &recipient, tick_lower, tick_upper, &position);
        Self::ensure_user_range_exists(&e, &recipient, tick_lower, tick_upper);

        Self::update_tick_liquidity(&e, tick_lower, amount as i128, false)?;
        Self::update_tick_liquidity(&e, tick_upper, amount as i128, true)?;

        if slot.tick >= tick_lower && slot.tick < tick_upper {
            set_liquidity(&e, &get_liquidity(&e).saturating_add(amount));
        }

        Self::update_user_raw_liquidity(&e, &recipient, amount as i128);
        Self::recompute_user_weighted_liquidity(&e, &recipient);
        Self::rewards_refresh_working_balance(&e, &recipient);
        update_plane(&e);

        Ok((amount0, amount1))
    }

    fn withdraw_position(
        e: Env,
        owner: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount: u128,
    ) -> Result<(u128, u128), Error> {
        owner.require_auth();
        if amount == 0 {
            return Err(Error::AmountShouldBeGreaterThanZero);
        }

        Self::check_ticks_internal(&e, tick_lower, tick_upper)?;

        Self::recompute_user_weighted_liquidity(&e, &owner);
        Self::rewards_checkpoint_user(&e, &owner);

        let mut position = match get_position(&e, &owner, tick_lower, tick_upper) {
            Some(pos) => pos,
            None => return Err(Error::PositionNotFound),
        };

        let slot = get_slot0(&e);
        Self::accrue_position_fees(&e, &mut position, tick_lower, tick_upper, slot.tick)?;

        if position.liquidity < amount {
            return Err(Error::InsufficientLiquidity);
        }

        let sqrt_lower = sqrt_ratio_at_tick(&e, tick_lower)?;
        let sqrt_upper = sqrt_ratio_at_tick(&e, tick_upper)?;

        let (amount0, amount1) = if slot.sqrt_price_x96 <= sqrt_lower {
            (
                amount0_delta(&e, &sqrt_lower, &sqrt_upper, amount, false)?,
                0,
            )
        } else if slot.sqrt_price_x96 < sqrt_upper {
            (
                amount0_delta(&e, &slot.sqrt_price_x96, &sqrt_upper, amount, false)?,
                amount1_delta(&e, &sqrt_lower, &slot.sqrt_price_x96, amount, false)?,
            )
        } else {
            (
                0,
                amount1_delta(&e, &sqrt_lower, &sqrt_upper, amount, false)?,
            )
        };

        position.liquidity -= amount;
        position.tokens_owed_0 = position.tokens_owed_0.saturating_add(amount0);
        position.tokens_owed_1 = position.tokens_owed_1.saturating_add(amount1);

        if position.liquidity == 0 && position.tokens_owed_0 == 0 && position.tokens_owed_1 == 0 {
            remove_position(&e, &owner, tick_lower, tick_upper);
            Self::remove_user_range_if_empty(&e, &owner, tick_lower, tick_upper);
        } else {
            set_position(&e, &owner, tick_lower, tick_upper, &position);
        }

        Self::update_tick_liquidity(&e, tick_lower, -(amount as i128), false)?;
        Self::update_tick_liquidity(&e, tick_upper, -(amount as i128), true)?;

        if slot.tick >= tick_lower && slot.tick < tick_upper {
            let active = get_liquidity(&e);
            if active < amount {
                return Err(Error::LiquidityUnderflow);
            }
            set_liquidity(&e, &(active - amount));
        }

        Self::update_user_raw_liquidity(&e, &owner, -(amount as i128));
        Self::recompute_user_weighted_liquidity(&e, &owner);
        Self::rewards_refresh_working_balance(&e, &owner);
        update_plane(&e);

        Ok((amount0, amount1))
    }

    fn claim_position_fees(
        e: Env,
        owner: Address,
        recipient: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount0_requested: u128,
        amount1_requested: u128,
    ) -> Result<(u128, u128), Error> {
        Self::collect_internal(
            &e,
            &owner,
            &recipient,
            tick_lower,
            tick_upper,
            amount0_requested,
            amount1_requested,
            true,
        )
    }

    fn slot0(e: Env) -> Slot0 {
        get_slot0(&e)
    }

    fn router(e: Env) -> Address {
        get_router(&e)
    }

    fn token0(e: Env) -> Address {
        get_token0(&e)
    }

    fn token1(e: Env) -> Address {
        get_token1(&e)
    }

    fn fee(e: Env) -> u32 {
        get_fee(&e)
    }

    fn tick_spacing(e: Env) -> i32 {
        get_tick_spacing(&e)
    }

    fn tick_bitmap(e: Env, word_pos: i32) -> U256 {
        get_tick_bitmap_word(&e, word_pos)
    }

    fn liquidity(e: Env) -> u128 {
        get_liquidity(&e)
    }

    fn fee_growth_global_0_x128(e: Env) -> U256 {
        get_fee_growth_global_0_x128(&e)
    }

    fn fee_growth_global_1_x128(e: Env) -> U256 {
        get_fee_growth_global_1_x128(&e)
    }

    fn protocol_fees(e: Env) -> ProtocolFees {
        get_protocol_fees(&e)
    }

    fn ticks(e: Env, tick: i32) -> TickInfo {
        get_tick(&e, tick)
    }

    fn get_position(e: Env, recipient: Address, tick_lower: i32, tick_upper: i32) -> PositionData {
        match get_position(&e, &recipient, tick_lower, tick_upper) {
            Some(pos) => pos,
            None => panic_with_error!(&e, Error::PositionNotFound),
        }
    }

    fn get_full_pool_state(e: Env) -> Option<PoolState> {
        let slot = get_slot0(&e);
        Some(PoolState {
            fee: get_fee(&e),
            liquidity: get_liquidity(&e),
            sqrt_price_x96: slot.sqrt_price_x96,
            tick: slot.tick,
            tick_spacing: get_tick_spacing(&e),
            token0: get_token0(&e),
            token1: get_token1(&e),
        })
    }

    fn get_pool_state_with_balances(e: Env) -> Option<PoolStateWithBalances> {
        let state = Self::get_full_pool_state(e.clone())?;
        let contract = e.current_contract_address();
        let reserve0 = SorobanTokenClient::new(&e, &state.token0).balance(&contract);
        let reserve1 = SorobanTokenClient::new(&e, &state.token1).balance(&contract);

        Some(PoolStateWithBalances {
            reserve0,
            reserve1,
            state,
        })
    }

    fn get_user_position_snapshot(e: Env, user: Address) -> UserPositionSnapshot {
        UserPositionSnapshot {
            ranges: get_user_positions(&e, &user),
            raw_liquidity: get_user_raw_liquidity(&e, &user),
            weighted_liquidity: get_user_weighted_liquidity(&e, &user),
        }
    }

    fn get_total_weighted_liquidity(e: Env) -> u128 {
        get_total_weighted_liquidity(&e)
    }

    fn get_total_raw_liquidity(e: Env) -> u128 {
        get_total_raw_liquidity(&e)
    }

    fn gauges_add(e: Env, admin: Address, gauge_address: Address) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);
        rewards_gauge::operations::add(&e, gauge_address);
    }

    fn gauges_remove(e: Env, admin: Address, reward_token: Address) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);
        rewards_gauge::operations::remove(&e, reward_token);
    }

    fn gauges_list(e: Env) -> Map<Address, Address> {
        rewards_gauge::operations::list(&e)
    }

    fn gauges_claim(e: Env, user: Address) -> Map<Address, u128> {
        user.require_auth();
        Self::recompute_user_weighted_liquidity(&e, &user);

        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);

        rewards_gauge::operations::claim(&e, &user, user_weighted, total_weighted)
    }

    fn gauges_get_rewards_info(e: Env, user: Address) -> Map<Address, Map<Symbol, i128>> {
        Self::recompute_user_weighted_liquidity(&e, &user);

        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);

        rewards_gauge::operations::get_rewards_info(&e, &user, user_weighted, total_weighted)
    }
}
