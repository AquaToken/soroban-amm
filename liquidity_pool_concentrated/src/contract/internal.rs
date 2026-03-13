use super::*;
use crate::bitmap;

impl ConcentratedLiquidityPool {
    pub(super) fn has_admin_role(e: &Env) -> bool {
        AccessControl::new(e).get_role_safe(&Role::Admin).is_some()
    }

    pub(super) fn require_admin(e: &Env, admin: &Address) {
        admin.require_auth();
        AccessControl::new(e).assert_address_has_role(admin, &Role::Admin);
    }

    pub(super) fn check_ticks_internal(e: &Env, tick_lower: i32, tick_upper: i32) {
        if tick_lower >= tick_upper {
            panic_with_error!(e, Error::TickLowerNotLessThanUpper);
        }
        if tick_lower < MIN_TICK {
            panic_with_error!(e, Error::TickLowerTooLow);
        }
        if tick_upper > MAX_TICK {
            panic_with_error!(e, Error::TickUpperTooHigh);
        }

        let spacing = get_tick_spacing(e);
        if spacing <= 0 {
            panic_with_error!(e, Error::InvalidTickSpacing);
        }
        if tick_lower % spacing != 0 || tick_upper % spacing != 0 {
            panic_with_error!(e, Error::TickNotSpacedCorrectly);
        }
    }

    pub(super) fn update_chunk_bitmap_bit(e: &Env, chunk_pos: i32, has_initialized: bool) {
        let (word_pos, bit_pos) = bitmap::chunk_bitmap_position(chunk_pos);
        let mut word = bitmap::u256_to_array(&get_chunk_bitmap_word(e, word_pos));
        let was_empty = word == [0u8; 32];
        bitmap::set_bit(&mut word, bit_pos, has_initialized);
        let is_empty = word == [0u8; 32];
        set_chunk_bitmap_word(e, word_pos, &bitmap::u256_from_array(e, &word));

        // Maintain level-2 word bitmap
        if was_empty != is_empty {
            let (l2_word_pos, l2_bit_pos) = bitmap::word_bitmap_position(word_pos);
            let mut l2_word = bitmap::u256_to_array(&get_word_bitmap(e, l2_word_pos));
            bitmap::set_bit(&mut l2_word, l2_bit_pos, !is_empty);
            set_word_bitmap(e, l2_word_pos, &bitmap::u256_from_array(e, &l2_word));
        }
    }

    // Check if any tick in a chunk is initialized (liquidity_gross > 0).
    fn chunk_has_initialized(chunk: &Vec<TickData>) -> bool {
        for i in 0..TICKS_PER_CHUNK as u32 {
            if chunk.get(i).unwrap().2 > 0 {
                return true;
            }
        }
        false
    }

    // Scan a loaded chunk for the highest (last) initialized tick. Returns slot index.
    pub(super) fn scan_chunk_highest_init(chunk: &Vec<TickData>) -> Option<u32> {
        for s in (0..TICKS_PER_CHUNK as u32).rev() {
            if chunk.get(s).unwrap().2 > 0 {
                return Some(s);
            }
        }
        None
    }

    // Scan a loaded chunk for the lowest (first) initialized tick. Returns slot index.
    pub(super) fn scan_chunk_lowest_init(chunk: &Vec<TickData>) -> Option<u32> {
        for s in 0..TICKS_PER_CHUNK as u32 {
            if chunk.get(s).unwrap().2 > 0 {
                return Some(s);
            }
        }
        None
    }

    /// Find the next globally initialized tick using the 3-level bitmap.
    /// Called only when the global min or max tick is de-initialized, so all
    /// remaining ticks in the same bitmap word are on the correct side.
    /// `lte == true`: scan downward (find new max); `lte == false`: scan upward (find new min).
    /// Returns MIN_TICK / MAX_TICK sentinel when no tick found (pool now empty).
    fn find_next_global_tick(e: &Env, from_tick: i32, spacing: i32, lte: bool) -> i32 {
        let compressed = bitmap::compress_tick(from_tick, spacing);
        let (chunk_pos, _) = chunk_address(compressed);
        let (word_pos, _) = bitmap::chunk_bitmap_position(chunk_pos);

        // 1. Search current bitmap word (tick already de-initialized, so extreme is correct)
        let tick = Self::extreme_tick_in_bitmap_word(e, word_pos, spacing, lte);
        if lte && tick != MIN_TICK {
            return tick;
        }
        if !lte && tick != MAX_TICK {
            return tick;
        }

        // 2. Search via L2 for adjacent bitmap words
        let (l2_pos, l2_bit) = bitmap::word_bitmap_position(word_pos);
        let search_bit = |l2_pos: i32, from: u32| -> Option<i32> {
            let l2_word = bitmap::u256_to_array(&get_word_bitmap(e, l2_pos));
            let found = if lte {
                bitmap::find_prev_set_bit(&l2_word, from)
            } else {
                bitmap::find_next_set_bit(&l2_word, from)
            };
            found.map(|bit| (l2_pos << 8) + bit as i32)
        };

        // Try current L2 word (skip own bit)
        let adjacent = if lte && l2_bit > 0 {
            search_bit(l2_pos, l2_bit - 1)
        } else if !lte && l2_bit < 255 {
            search_bit(l2_pos, l2_bit + 1)
        } else {
            None
        };
        if let Some(found_word_pos) = adjacent {
            return Self::extreme_tick_in_bitmap_word(e, found_word_pos, spacing, lte);
        }

        // Try adjacent L2 word (at most 1 more covers entire range)
        let l2_adj = if lte { l2_pos - 1 } else { l2_pos + 1 };
        let from = if lte { 255 } else { 0 };
        if let Some(found_word_pos) = search_bit(l2_adj, from) {
            return Self::extreme_tick_in_bitmap_word(e, found_word_pos, spacing, lte);
        }

        // No initialized tick found — pool is empty; return sentinel
        if lte {
            MIN_TICK
        } else {
            MAX_TICK
        }
    }

    // Find the extreme (highest or lowest) initialized tick within a chunk bitmap word.
    // `highest=true`: find highest (for max bound); `highest=false`: find lowest (for min bound).
    pub(super) fn extreme_tick_in_bitmap_word(
        e: &Env,
        word_pos: i32,
        spacing: i32,
        highest: bool,
    ) -> i32 {
        let word = bitmap::u256_to_array(&get_chunk_bitmap_word(e, word_pos));
        let found_bit = if highest {
            bitmap::find_prev_set_bit(&word, 255)
        } else {
            bitmap::find_next_set_bit(&word, 0)
        };
        if let Some(bit) = found_bit {
            let chunk_pos = (word_pos << 8) + bit as i32;
            let chunk = get_or_create_tick_chunk(e, chunk_pos);
            let slot = if highest {
                Self::scan_chunk_highest_init(&chunk)
            } else {
                Self::scan_chunk_lowest_init(&chunk)
            };
            if let Some(s) = slot {
                return bitmap::compressed_to_tick(chunk_pos * TICKS_PER_CHUNK + s as i32, spacing);
            }
        }
        if highest {
            MIN_TICK
        } else {
            MAX_TICK
        }
    }

    // Two-level search: scan within current chunk, then across chunks via chunk bitmap.
    // Returns (next_tick, initialized) — same contract as the old find_initialized_tick_in_word.
    pub(super) fn find_initialized_tick_in_word(
        e: &Env,
        tick: i32,
        spacing: i32,
        lte: bool,
        cc: &mut ChunkCache,
    ) -> (i32, bool) {
        let compressed = bitmap::compress_tick(tick, spacing);

        if lte {
            // --- Scanning downward ---
            let (chunk_pos, slot) = chunk_address(compressed);

            // 1. Check current chunk: scan slots [0..=slot] downward
            if let Some(chunk) = cc.get_chunk(e, chunk_pos) {
                for s in (0..=slot).rev() {
                    if chunk.get(s).unwrap().2 > 0 {
                        let found_compressed = chunk_pos * TICKS_PER_CHUNK + s as i32;
                        return (bitmap::compressed_to_tick(found_compressed, spacing), true);
                    }
                }
            }

            // 2. Use chunk bitmap to find previous chunk with initialized ticks
            let (bm_word_pos, bm_bit_pos) = bitmap::chunk_bitmap_position(chunk_pos);
            let word = bitmap::u256_to_array(&get_chunk_bitmap_word(e, bm_word_pos));

            // Search for set bit below current chunk in this bitmap word
            if bm_bit_pos > 0 {
                if let Some(found_bit) = bitmap::find_prev_set_bit(&word, bm_bit_pos - 1) {
                    let found_chunk_pos = (bm_word_pos << 8) + found_bit as i32;
                    let chunk = cc.get_or_create_chunk(e, found_chunk_pos);
                    if let Some(s) = Self::scan_chunk_highest_init(&chunk) {
                        let found_compressed = found_chunk_pos * TICKS_PER_CHUNK + s as i32;
                        return (bitmap::compressed_to_tick(found_compressed, spacing), true);
                    }
                }
            }

            // Not found in this bitmap word — return boundary
            let boundary_compressed = (bm_word_pos << 8) * TICKS_PER_CHUNK;
            (
                bitmap::compressed_to_tick(boundary_compressed, spacing),
                false,
            )
        } else {
            // --- Scanning upward ---
            let compressed_plus_one = compressed.saturating_add(1);
            let (chunk_pos, slot) = chunk_address(compressed_plus_one);

            // 1. Check current chunk: scan slots [slot..TICKS_PER_CHUNK) upward
            if let Some(chunk) = cc.get_chunk(e, chunk_pos) {
                for s in slot..TICKS_PER_CHUNK as u32 {
                    if chunk.get(s).unwrap().2 > 0 {
                        let found_compressed = chunk_pos * TICKS_PER_CHUNK + s as i32;
                        return (bitmap::compressed_to_tick(found_compressed, spacing), true);
                    }
                }
            }

            // 2. Use chunk bitmap to find next chunk with initialized ticks
            let (bm_word_pos, bm_bit_pos) = bitmap::chunk_bitmap_position(chunk_pos);
            let word = bitmap::u256_to_array(&get_chunk_bitmap_word(e, bm_word_pos));

            if bm_bit_pos < 255 {
                if let Some(found_bit) = bitmap::find_next_set_bit(&word, bm_bit_pos + 1) {
                    let found_chunk_pos = (bm_word_pos << 8) + found_bit as i32;
                    let chunk = cc.get_or_create_chunk(e, found_chunk_pos);
                    if let Some(s) = Self::scan_chunk_lowest_init(&chunk) {
                        let found_compressed = found_chunk_pos * TICKS_PER_CHUNK + s as i32;
                        return (bitmap::compressed_to_tick(found_compressed, spacing), true);
                    }
                }
            }

            // Not found — return boundary at end of current bitmap word
            let boundary_compressed =
                ((bm_word_pos << 8) + 255) * TICKS_PER_CHUNK + (TICKS_PER_CHUNK - 1);
            (
                bitmap::compressed_to_tick(boundary_compressed, spacing),
                false,
            )
        }
    }

    pub(super) fn update_tick_liquidity(
        e: &Env,
        tick_idx: i32,
        liquidity_delta: i128,
        is_upper: bool,
    ) {
        let spacing = get_tick_spacing(e);
        let compressed = bitmap::compress_tick(tick_idx, spacing);
        let (chunk_pos, slot_idx) = chunk_address(compressed);

        let mut chunk = get_or_create_tick_chunk(e, chunk_pos);
        let td = chunk.get(slot_idx).unwrap();
        let mut tick = TickInfo::from(td);

        let was_initialized = tick.liquidity_gross > 0;

        let delta = liquidity_delta.unsigned_abs();
        if liquidity_delta >= 0 {
            tick.liquidity_gross = tick.liquidity_gross.saturating_add(delta);
        } else {
            if tick.liquidity_gross < delta {
                panic_with_error!(e, Error::LiquidityUnderflow);
            }
            tick.liquidity_gross -= delta;
        }

        if is_upper {
            tick.liquidity_net = tick.liquidity_net.saturating_sub(liquidity_delta);
        } else {
            tick.liquidity_net = tick.liquidity_net.saturating_add(liquidity_delta);
        }

        let is_initialized = tick.liquidity_gross > 0;

        if !was_initialized && is_initialized {
            let slot0 = get_slot0(e);
            if tick_idx <= slot0.tick {
                tick.fee_growth_outside_0_x128 = get_fee_growth_global_0_x128(e);
                tick.fee_growth_outside_1_x128 = get_fee_growth_global_1_x128(e);
            }
        } else if was_initialized && !is_initialized {
            // Clear stale fee accumulators on de-initialization to prevent
            // history-dependent fee attribution when the tick is later reused.
            tick.fee_growth_outside_0_x128 = soroban_sdk::U256::from_u32(e, 0);
            tick.fee_growth_outside_1_x128 = soroban_sdk::U256::from_u32(e, 0);
        }

        // Write tick back into chunk
        chunk.set(
            slot_idx,
            TickData(
                tick.fee_growth_outside_0_x128,
                tick.fee_growth_outside_1_x128,
                tick.liquidity_gross,
                tick.liquidity_net,
            ),
        );
        set_tick_chunk(e, chunk_pos, &chunk);

        // Update chunk bitmap on initialization state change
        if !was_initialized && is_initialized {
            Self::update_chunk_bitmap_bit(e, chunk_pos, true);
            // Expand tick bounds
            if tick_idx < get_min_init_tick(e) {
                set_min_init_tick(e, &tick_idx);
            }
            if tick_idx > get_max_init_tick(e) {
                set_max_init_tick(e, &tick_idx);
            }
        } else if was_initialized && !is_initialized {
            // Scan chunk to see if any tick remains initialized
            let any_init = Self::chunk_has_initialized(&chunk);
            if !any_init {
                Self::update_chunk_bitmap_bit(e, chunk_pos, false);
            }
            // Shrink tick bounds if this was the min or max
            if tick_idx == get_min_init_tick(e) {
                set_min_init_tick(e, &Self::find_next_global_tick(e, tick_idx, spacing, false));
            }
            if tick_idx == get_max_init_tick(e) {
                set_max_init_tick(e, &Self::find_next_global_tick(e, tick_idx, spacing, true));
            }
        }
    }

    pub(super) fn ensure_user_range_exists(
        e: &Env,
        user: &Address,
        tick_lower: i32,
        tick_upper: i32,
    ) {
        let mut state = get_user_state(e, user);
        for range in state.positions.iter() {
            if range.tick_lower == tick_lower && range.tick_upper == tick_upper {
                return;
            }
        }

        if state.positions.len() >= MAX_USER_POSITIONS {
            panic_with_error!(e, Error::TooManyPositions);
        }

        state.positions.push_back(PositionRange {
            tick_lower,
            tick_upper,
        });
        set_user_state(e, user, &state);
    }

    pub(super) fn remove_user_range_if_empty(
        e: &Env,
        user: &Address,
        tick_lower: i32,
        tick_upper: i32,
    ) {
        let mut state = get_user_state(e, user);
        let mut updated = Vec::new(e);
        for range in state.positions.iter() {
            if range.tick_lower == tick_lower && range.tick_upper == tick_upper {
                continue;
            }
            updated.push_back(range);
        }
        state.positions = updated;
        set_user_state(e, user, &state);
    }

    // Pure computation of weighted liquidity for a user's positions.
    // If target range matches an existing position, uses target_liquidity instead of stored value.
    // If target range is new and target_liquidity > 0, includes it as a new position.
    // No storage writes — used by both recompute (mutating) and estimate (read-only).
    pub(super) fn compute_user_weighted_liquidity(
        e: &Env,
        user: &Address,
        target_tick_lower: i32,
        target_tick_upper: i32,
        target_liquidity: u128,
    ) -> u128 {
        let tick_current = get_slot0(e).tick;
        let fee = get_fee(e);
        let state = get_user_state(e, user);
        let mut weighted = 0u128;
        let mut target_applied = false;

        for range in state.positions.iter() {
            let liq = if range.tick_lower == target_tick_lower
                && range.tick_upper == target_tick_upper
            {
                target_applied = true;
                target_liquidity
            } else if let Some(position) = get_position(e, user, range.tick_lower, range.tick_upper)
            {
                position.liquidity
            } else {
                0
            };
            if liq == 0 {
                continue;
            }
            let multiplier =
                position_multiplier_bps(tick_current, range.tick_lower, range.tick_upper, fee);
            weighted = weighted.saturating_add(apply_multiplier(liq, multiplier));
        }

        // New position not yet in user's list
        if !target_applied && target_liquidity > 0 {
            let multiplier =
                position_multiplier_bps(tick_current, target_tick_lower, target_tick_upper, fee);
            weighted = weighted.saturating_add(apply_multiplier(target_liquidity, multiplier));
        }

        weighted
    }

    pub(super) fn recompute_user_weighted_liquidity(e: &Env, user: &Address) -> u128 {
        let weighted = Self::compute_user_weighted_liquidity(e, user, 0, 0, 0);

        let mut state = get_user_state(e, user);
        let prev_weighted = state.weighted_liquidity;
        let mut total_weighted = get_total_weighted_liquidity(e);

        if weighted >= prev_weighted {
            total_weighted = total_weighted.saturating_add(weighted - prev_weighted);
        } else {
            total_weighted = total_weighted.saturating_sub(prev_weighted - weighted);
        }

        state.weighted_liquidity = weighted;
        set_user_state(e, user, &state);
        set_total_weighted_liquidity(e, &total_weighted);

        weighted
    }

    pub(super) fn update_user_raw_liquidity(e: &Env, user: &Address, delta: i128) {
        let mut state = get_user_state(e, user);
        let prev_total_raw = get_total_raw_liquidity(e);

        if delta >= 0 {
            let inc = delta as u128;
            state.raw_liquidity = state.raw_liquidity.saturating_add(inc);
            set_total_raw_liquidity(e, &prev_total_raw.saturating_add(inc));
        } else {
            let dec = (-delta) as u128;
            state.raw_liquidity = state.raw_liquidity.saturating_sub(dec);
            set_total_raw_liquidity(e, &prev_total_raw.saturating_sub(dec));
        }
        set_user_state(e, user, &state);
    }

    pub(super) fn rewards_manager(e: &Env) -> Rewards {
        Rewards::new(e, 100)
    }

    pub(super) fn rewards_checkpoint_user(e: &Env, user: &Address) {
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

    pub(super) fn rewards_refresh_working_balance(e: &Env, user: &Address) {
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
        manager.update_working_balance(user, total_weighted, user_weighted);
    }

    pub(super) fn compute_fee_growth_inside(
        e: &Env,
        tick_lower: i32,
        tick_upper: i32,
        tick_current: i32,
    ) -> (U256, U256) {
        let fee_growth_global_0 = get_fee_growth_global_0_x128(e);
        let fee_growth_global_1 = get_fee_growth_global_1_x128(e);

        let spacing = get_tick_spacing(e);
        let lower = get_tick(e, tick_lower, spacing);
        let upper = get_tick(e, tick_upper, spacing);

        let fee_growth_below_0 = if tick_current >= tick_lower {
            lower.fee_growth_outside_0_x128
        } else {
            wrapping_sub_u256(e, &fee_growth_global_0, &lower.fee_growth_outside_0_x128)
        };
        let fee_growth_below_1 = if tick_current >= tick_lower {
            lower.fee_growth_outside_1_x128
        } else {
            wrapping_sub_u256(e, &fee_growth_global_1, &lower.fee_growth_outside_1_x128)
        };

        let fee_growth_above_0 = if tick_current < tick_upper {
            upper.fee_growth_outside_0_x128
        } else {
            wrapping_sub_u256(e, &fee_growth_global_0, &upper.fee_growth_outside_0_x128)
        };
        let fee_growth_above_1 = if tick_current < tick_upper {
            upper.fee_growth_outside_1_x128
        } else {
            wrapping_sub_u256(e, &fee_growth_global_1, &upper.fee_growth_outside_1_x128)
        };

        (
            wrapping_sub_u256(
                e,
                &wrapping_sub_u256(e, &fee_growth_global_0, &fee_growth_below_0),
                &fee_growth_above_0,
            ),
            wrapping_sub_u256(
                e,
                &wrapping_sub_u256(e, &fee_growth_global_1, &fee_growth_below_1),
                &fee_growth_above_1,
            ),
        )
    }

    pub(super) fn accrue_position_fees(
        e: &Env,
        position: &mut PositionData,
        tick_lower: i32,
        tick_upper: i32,
        tick_current: i32,
    ) {
        let (inside_0, inside_1) =
            Self::compute_fee_growth_inside(e, tick_lower, tick_upper, tick_current);

        let delta_0 = wrapping_sub_u256(e, &inside_0, &position.fee_growth_inside_0_last_x128);
        let delta_1 = wrapping_sub_u256(e, &inside_1, &position.fee_growth_inside_1_last_x128);

        let owed_0 = mul_div_fee_growth(e, &delta_0, position.liquidity);
        let owed_1 = mul_div_fee_growth(e, &delta_1, position.liquidity);

        position.tokens_owed_0 = position.tokens_owed_0.saturating_add(owed_0);
        position.tokens_owed_1 = position.tokens_owed_1.saturating_add(owed_1);
        position.fee_growth_inside_0_last_x128 = inside_0;
        position.fee_growth_inside_1_last_x128 = inside_1;
    }

    pub(super) fn get_or_create_position(
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

    // Concentrated-specific event: current pool price state.
    // Emitted after swap, deposit_position, withdraw_position.
    pub(super) fn emit_pool_state(e: &Env, slot: &Slot0, liquidity: u128) {
        e.events().publish(
            (Symbol::new(e, "pool_state"),),
            (slot.sqrt_price_x96.clone(), slot.tick, liquidity as i128),
        );
    }

    // Concentrated-specific event: tick-level liquidity change.
    // Emitted on deposit_position (delta > 0) and withdraw_position (delta < 0).
    // Backend uses this to maintain an incremental tick map for swap estimation.
    pub(super) fn emit_position_update(
        e: &Env,
        user: &Address,
        tick_lower: i32,
        tick_upper: i32,
        liquidity_delta: i128,
    ) {
        e.events().publish(
            (Symbol::new(e, "position_update"), user.clone()),
            (tick_lower, tick_upper, liquidity_delta),
        );
    }

    pub(super) fn collect_internal(
        e: &Env,
        owner: &Address,
        tick_lower: i32,
        tick_upper: i32,
        requested_amounts: Vec<u128>,
        require_owner_auth: bool,
    ) -> Vec<u128> {
        if require_owner_auth {
            owner.require_auth();
        }
        if requested_amounts.len() != 2 {
            panic_with_error!(e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        let mut position = match get_position(e, owner, tick_lower, tick_upper) {
            Some(pos) => pos,
            None => panic_with_error!(e, Error::PositionNotFound),
        };

        let tick_current = get_slot0(e).tick;
        Self::accrue_position_fees(e, &mut position, tick_lower, tick_upper, tick_current);

        let amount0 = position
            .tokens_owed_0
            .min(requested_amounts.get_unchecked(0));
        let amount1 = position
            .tokens_owed_1
            .min(requested_amounts.get_unchecked(1));

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
            SorobanTokenClient::new(e, &token0).transfer(&contract, owner, &(amount0 as i128));
        }
        if amount1 > 0 {
            SorobanTokenClient::new(e, &token1).transfer(&contract, owner, &(amount1 as i128));
        }

        if amount0 > 0 || amount1 > 0 {
            ClaimFees {
                owner: owner.clone(),
                token0,
                token1,
                amount0: amount0 as i128,
                amount1: amount1 as i128,
            }
            .publish(e);
        }

        let res0 = get_reserve0(e) - amount0;
        let res1 = get_reserve1(e) - amount1;
        set_reserve0(e, &res0);
        set_reserve1(e, &res1);

        PoolEvents::new(e).update_reserves(Vec::from_array(e, [res0, res1]));
        update_plane(e);

        Vec::from_array(e, [amount0, amount1])
    }

    pub(super) fn cross_tick(e: &Env, tick_idx: i32, cc: &mut ChunkCache) -> i128 {
        let spacing = get_tick_spacing(e);
        let compressed = bitmap::compress_tick(tick_idx, spacing);
        let (chunk_pos, slot_idx) = chunk_address(compressed);

        let mut chunk = cc.get_or_create_chunk(e, chunk_pos);
        let td = chunk.get(slot_idx).unwrap();
        let mut tick = TickInfo::from(td);

        let fee_growth_global_0 = get_fee_growth_global_0_x128(e);
        let fee_growth_global_1 = get_fee_growth_global_1_x128(e);

        tick.fee_growth_outside_0_x128 =
            wrapping_sub_u256(e, &fee_growth_global_0, &tick.fee_growth_outside_0_x128);
        tick.fee_growth_outside_1_x128 =
            wrapping_sub_u256(e, &fee_growth_global_1, &tick.fee_growth_outside_1_x128);

        let liquidity_net = tick.liquidity_net;

        chunk.set(
            slot_idx,
            TickData(
                tick.fee_growth_outside_0_x128,
                tick.fee_growth_outside_1_x128,
                tick.liquidity_gross,
                tick.liquidity_net,
            ),
        );
        cc.set_chunk(chunk_pos, &chunk);

        liquidity_net
    }

    pub(super) fn add_fee_growth_global(
        e: &Env,
        zero_for_one: bool,
        fee_amount_for_lp: u128,
        liquidity: u128,
    ) {
        if fee_amount_for_lp == 0 || liquidity == 0 {
            return;
        }

        let growth_delta = fee_growth_delta_x128(e, fee_amount_for_lp, liquidity);
        if zero_for_one {
            let next = wrapping_add_u256(e, &get_fee_growth_global_0_x128(e), &growth_delta);
            set_fee_growth_global_0_x128(e, &next);
        } else {
            let next = wrapping_add_u256(e, &get_fee_growth_global_1_x128(e), &growth_delta);
            set_fee_growth_global_1_x128(e, &next);
        }
    }

    pub(super) fn compute_swap_step(
        e: &Env,
        sqrt_current: &U256,
        sqrt_target: &U256,
        liquidity: u128,
        amount_remaining: u128,
        fee_pips: u32,
        zero_for_one: bool,
        exact_input: bool,
    ) -> SwapStep {
        if liquidity == 0 {
            return SwapStep {
                sqrt_next: sqrt_target.clone(),
                amount_in: 0,
                amount_out: 0,
                fee_amount: 0,
            };
        }

        let fee = fee_pips as u128;
        let fee_complement = FEE_DENOMINATOR - fee;

        if exact_input {
            let amount_remaining_less_fee =
                mul_div_u128(e, amount_remaining, fee_complement, FEE_DENOMINATOR, false);

            let amount_in_to_target = if zero_for_one {
                amount0_delta(e, sqrt_target, sqrt_current, liquidity, true)
            } else {
                amount1_delta(e, sqrt_current, sqrt_target, liquidity, true)
            };

            let sqrt_next = if amount_remaining_less_fee >= amount_in_to_target {
                sqrt_target.clone()
            } else {
                let computed = get_next_sqrt_price_from_input(
                    e,
                    sqrt_current,
                    liquidity,
                    amount_remaining_less_fee,
                    zero_for_one,
                );
                // Clamp to [target, current] range
                if zero_for_one {
                    computed.max(sqrt_target.clone())
                } else {
                    computed.min(sqrt_target.clone())
                }
            };

            let max_reached = sqrt_next == *sqrt_target;

            let amount_in = if zero_for_one {
                amount0_delta(e, &sqrt_next, sqrt_current, liquidity, true)
            } else {
                amount1_delta(e, sqrt_current, &sqrt_next, liquidity, true)
            };

            let amount_out = if zero_for_one {
                amount1_delta(e, &sqrt_next, sqrt_current, liquidity, false)
            } else {
                amount0_delta(e, sqrt_current, &sqrt_next, liquidity, false)
            };

            let fee_amount = if max_reached {
                mul_div_u128(e, amount_in, fee, fee_complement, true)
            } else {
                amount_remaining.saturating_sub(amount_in)
            };

            SwapStep {
                sqrt_next,
                amount_in,
                amount_out,
                fee_amount,
            }
        } else {
            let amount_out_to_target = if zero_for_one {
                amount1_delta(e, sqrt_target, sqrt_current, liquidity, false)
            } else {
                amount0_delta(e, sqrt_current, sqrt_target, liquidity, false)
            };

            let sqrt_next = if amount_remaining >= amount_out_to_target {
                sqrt_target.clone()
            } else {
                let computed = get_next_sqrt_price_from_output(
                    e,
                    sqrt_current,
                    liquidity,
                    amount_remaining,
                    zero_for_one,
                );
                // Clamp to [target, current] range
                if zero_for_one {
                    computed.max(sqrt_target.clone())
                } else {
                    computed.min(sqrt_target.clone())
                }
            };

            let amount_in = if zero_for_one {
                amount0_delta(e, &sqrt_next, sqrt_current, liquidity, true)
            } else {
                amount1_delta(e, sqrt_current, &sqrt_next, liquidity, true)
            };

            let mut amount_out = if zero_for_one {
                amount1_delta(e, &sqrt_next, sqrt_current, liquidity, false)
            } else {
                amount0_delta(e, sqrt_current, &sqrt_next, liquidity, false)
            };

            if amount_out > amount_remaining {
                amount_out = amount_remaining;
            }

            let fee_amount = mul_div_u128(e, amount_in, fee, fee_complement, true);

            SwapStep {
                sqrt_next,
                amount_in,
                amount_out,
                fee_amount,
            }
        }
    }

    pub(super) fn validate_price_limit(
        e: &Env,
        slot: &Slot0,
        zero_for_one: bool,
        sqrt_price_limit_x96: U256,
    ) -> U256 {
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
                panic_with_error!(e, Error::InvalidPriceLimit);
            }
        } else if limit >= max || limit <= slot.sqrt_price_x96 {
            panic_with_error!(e, Error::InvalidPriceLimit);
        }

        limit
    }

    pub(super) fn direction_from_indexes(e: &Env, in_idx: u32, out_idx: u32) -> bool {
        if in_idx == out_idx {
            panic_with_error!(e, LiquidityPoolValidationError::CannotSwapSameToken);
        }
        if in_idx > 1 {
            panic_with_error!(e, LiquidityPoolValidationError::InTokenOutOfBounds);
        }
        if out_idx > 1 {
            panic_with_error!(e, LiquidityPoolValidationError::OutTokenOutOfBounds);
        }
        in_idx == 0 && out_idx == 1
    }

    /// Compute the initial price for a deposit from the amount ratio.
    ///
    /// Derives tick directly: price = a1/a0 → tick.
    /// Panics if the derived tick falls outside [tick_lower, tick_upper) —
    /// the caller must provide amounts whose price ratio matches the deposit range.
    /// Only called on the very first deposit when the pool has zero liquidity.
    pub(super) fn init_sqrt_price_for_range(
        e: &Env,
        tick_lower: i32,
        tick_upper: i32,
        desired_amount0: u128,
        desired_amount1: u128,
    ) -> (U256, i32) {
        let sqrt_price = sqrt_price_from_amounts(e, desired_amount0, desired_amount1);
        let tick = tick_at_sqrt_ratio(e, &sqrt_price);

        if tick < tick_lower || tick >= tick_upper {
            panic_with_error!(e, Error::TickOutOfBounds);
        }

        (sqrt_price, tick)
    }

    pub(super) fn full_range_ticks(e: &Env) -> (i32, i32) {
        let spacing = get_tick_spacing(e);
        if spacing <= 0 {
            panic_with_error!(e, Error::InvalidTickSpacing);
        }

        let mut tick_lower = MIN_TICK - (MIN_TICK % spacing);
        if tick_lower < MIN_TICK {
            tick_lower = tick_lower.saturating_add(spacing);
        }

        let mut tick_upper = MAX_TICK - (MAX_TICK % spacing);
        if tick_upper > MAX_TICK {
            tick_upper = tick_upper.saturating_sub(spacing);
        }

        Self::check_ticks_internal(e, tick_lower, tick_upper);
        (tick_lower, tick_upper)
    }

    pub(super) fn amounts_for_liquidity(
        e: &Env,
        slot: &Slot0,
        tick_lower: i32,
        tick_upper: i32,
        liquidity: u128,
        round_up: bool,
    ) -> (u128, u128) {
        let sqrt_lower = sqrt_ratio_at_tick(e, tick_lower);
        let sqrt_upper = sqrt_ratio_at_tick(e, tick_upper);

        if slot.sqrt_price_x96 <= sqrt_lower {
            (
                amount0_delta(e, &sqrt_lower, &sqrt_upper, liquidity, round_up),
                0,
            )
        } else if slot.sqrt_price_x96 < sqrt_upper {
            (
                amount0_delta(e, &slot.sqrt_price_x96, &sqrt_upper, liquidity, round_up),
                amount1_delta(e, &sqrt_lower, &slot.sqrt_price_x96, liquidity, round_up),
            )
        } else {
            (
                0,
                amount1_delta(e, &sqrt_lower, &sqrt_upper, liquidity, round_up),
            )
        }
    }

    pub(super) fn max_liquidity_for_amounts(
        e: &Env,
        tick_lower: i32,
        tick_upper: i32,
        desired_amount0: u128,
        desired_amount1: u128,
    ) -> u128 {
        let slot = get_slot0(e);
        Self::max_liquidity_for_amounts_at_slot(
            e,
            &slot,
            tick_lower,
            tick_upper,
            desired_amount0,
            desired_amount1,
        )
    }

    pub(super) fn max_liquidity_for_amounts_at_slot(
        e: &Env,
        slot: &Slot0,
        tick_lower: i32,
        tick_upper: i32,
        desired_amount0: u128,
        desired_amount1: u128,
    ) -> u128 {
        if desired_amount0 == 0 && desired_amount1 == 0 {
            return 0;
        }

        let sqrt_lower = sqrt_ratio_at_tick(e, tick_lower);
        let sqrt_upper = sqrt_ratio_at_tick(e, tick_upper);

        // Analytical formulas (inverse of amount0_delta / amount1_delta):
        // - Below range:  only token0 needed → L = liquidity_for_amount0(sqrtLower, sqrtUpper, amount0)
        // - Above range:  only token1 needed → L = liquidity_for_amount1(sqrtLower, sqrtUpper, amount1)
        // - In range:     L = min(L0_from_current_to_upper, L1_from_lower_to_current)
        if slot.sqrt_price_x96 <= sqrt_lower {
            liquidity_for_amount0(e, &sqrt_lower, &sqrt_upper, desired_amount0)
        } else if slot.sqrt_price_x96 >= sqrt_upper {
            liquidity_for_amount1(e, &sqrt_lower, &sqrt_upper, desired_amount1)
        } else {
            let l0 = liquidity_for_amount0(e, &slot.sqrt_price_x96, &sqrt_upper, desired_amount0);
            let l1 = liquidity_for_amount1(e, &sqrt_lower, &slot.sqrt_price_x96, desired_amount1);
            l0.min(l1)
        }
    }

    /// Convert unsigned swap amounts to signed (amount0, amount1) pair.
    /// Positive = user pays in, negative = user receives out.
    fn swap_amounts_signed(
        zero_for_one: bool,
        exact_input: bool,
        amount_spec_used: u128,
        amount_calculated: u128,
    ) -> (i128, i128) {
        if zero_for_one {
            if exact_input {
                (amount_spec_used as i128, -(amount_calculated as i128))
            } else {
                (amount_calculated as i128, -(amount_spec_used as i128))
            }
        } else if exact_input {
            (-(amount_calculated as i128), amount_spec_used as i128)
        } else {
            (-(amount_spec_used as i128), amount_calculated as i128)
        }
    }

    /// Core swap loop shared by `simulate_swap_amounts` and `swap_internal`.
    ///
    /// When `dry_run == true` (simulation): reads tick data without modifying
    /// fee_growth_outside, skips protocol-fee accounting and storage writes.
    /// When `dry_run == false` (real swap): performs full cross_tick, accumulates
    /// protocol fees / fee growth, and persists state changes.
    ///
    /// Returns (amount_spec_used, amount_calculated, total_fee_amount,
    ///          final_slot, final_liquidity, pf_delta_0, pf_delta_1).
    #[allow(clippy::too_many_arguments)]
    fn swap_loop(
        e: &Env,
        zero_for_one: bool,
        amount_specified: i128,
        sqrt_price_limit_x96: U256,
        dry_run: bool,
    ) -> (u128, u128, u128, Slot0, u128, u128, u128) {
        if amount_specified == 0 {
            panic_with_error!(e, LiquidityPoolValidationError::ZeroAmount);
        }

        let exact_input = amount_specified > 0;

        // Early exit: no positions in the pool — nothing to scan.
        // Always error (matches standard/stableswap EmptyPool behavior).
        if get_total_raw_liquidity(e) == 0 {
            panic_with_error!(e, LiquidityPoolValidationError::EmptyPool);
        }

        let fee = get_fee(e);
        let mut slot = get_slot0(e);
        let price_limit = Self::validate_price_limit(e, &slot, zero_for_one, sqrt_price_limit_x96);
        let mut liquidity = get_liquidity(e);

        let old_protocol_fees = if dry_run {
            ProtocolFees {
                token0: 0,
                token1: 0,
            }
        } else {
            get_protocol_fees(e)
        };
        let mut protocol_fees = old_protocol_fees.clone();

        let mut amount_remaining = amount_specified.unsigned_abs();
        let mut amount_calculated: u128 = 0;
        let mut total_fee_amount: u128 = 0;
        let tick_spacing = get_tick_spacing(e);
        let mut cc = ChunkCache::new(e);

        // Tick bounds: stop swap when past all initialized ticks (anti-griefing).
        let tick_bound = if zero_for_one {
            get_min_init_tick(e)
        } else {
            get_max_init_tick(e)
        };

        while amount_remaining > 0 && slot.sqrt_price_x96 != price_limit {
            // Stop if we've passed all initialized ticks — no more liquidity ahead.
            if zero_for_one && slot.tick < tick_bound {
                break;
            }
            if !zero_for_one && slot.tick >= tick_bound {
                break;
            }
            let (next_tick, next_tick_initialized) = Self::find_initialized_tick_in_word(
                e,
                slot.tick,
                tick_spacing,
                zero_for_one,
                &mut cc,
            );
            let next_tick_price = sqrt_ratio_at_tick(e, next_tick);

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

            let sqrt_price_start = slot.sqrt_price_x96.clone();
            let step = Self::compute_swap_step(
                e,
                &slot.sqrt_price_x96,
                &sqrt_target,
                liquidity,
                amount_remaining,
                fee,
                zero_for_one,
                exact_input,
            );

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

            // Protocol fee split + fee growth (real swap only).
            if !dry_run {
                let protocol_cut =
                    step.fee_amount * get_protocol_fee_fraction(e) as u128 / FEE_DENOMINATOR;
                let fee_for_lp = step.fee_amount.saturating_sub(protocol_cut);
                if zero_for_one {
                    protocol_fees.token0 = protocol_fees.token0.saturating_add(protocol_cut);
                } else {
                    protocol_fees.token1 = protocol_fees.token1.saturating_add(protocol_cut);
                }
                total_fee_amount = total_fee_amount.saturating_add(step.fee_amount);
                Self::add_fee_growth_global(e, zero_for_one, fee_for_lp, liquidity);
            }

            slot.sqrt_price_x96 = step.sqrt_next;

            // Match Uniswap V3 semantics:
            // cross tick only when we reached the actual next tick price,
            // not when we stopped at an arbitrary price limit between ticks.
            if slot.sqrt_price_x96 == next_tick_price {
                if next_tick_initialized {
                    // Real swap: cross_tick flips fee_growth_outside.
                    // Simulation: read liquidity_net without side effects.
                    let mut liquidity_net = if dry_run {
                        cc.get_tick(e, next_tick, tick_spacing).liquidity_net
                    } else {
                        Self::cross_tick(e, next_tick, &mut cc)
                    };
                    if zero_for_one {
                        liquidity_net = -liquidity_net;
                    }
                    if liquidity_net < 0 {
                        let dec = (-liquidity_net) as u128;
                        if liquidity < dec {
                            panic_with_error!(e, Error::LiquidityUnderflow);
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
            } else if slot.sqrt_price_x96 != sqrt_price_start {
                slot.tick = tick_at_sqrt_ratio(e, &slot.sqrt_price_x96);
            }
        }

        if !dry_run {
            cc.flush(e);
            set_protocol_fees(e, &protocol_fees);
            set_liquidity(e, &liquidity);
            set_slot0(e, &slot);
            update_plane(e);
        }

        if !exact_input && amount_remaining > 0 {
            panic_with_error!(e, Error::InsufficientLiquidity);
        }

        let amount_spec_used = amount_specified
            .unsigned_abs()
            .saturating_sub(amount_remaining);
        let pf_delta_0 = protocol_fees.token0 - old_protocol_fees.token0;
        let pf_delta_1 = protocol_fees.token1 - old_protocol_fees.token1;

        (
            amount_spec_used,
            amount_calculated,
            total_fee_amount,
            slot,
            liquidity,
            pf_delta_0,
            pf_delta_1,
        )
    }

    pub(super) fn simulate_swap_amounts(
        e: &Env,
        zero_for_one: bool,
        amount_specified: i128,
        sqrt_price_limit_x96: U256,
    ) -> (i128, i128) {
        let exact_input = amount_specified > 0;
        let (amount_spec_used, amount_calculated, ..) = Self::swap_loop(
            e,
            zero_for_one,
            amount_specified,
            sqrt_price_limit_x96,
            true,
        );
        Self::swap_amounts_signed(
            zero_for_one,
            exact_input,
            amount_spec_used,
            amount_calculated,
        )
    }

    /// `user_max_in`: the user-specified maximum input amount, known at signing time.
    /// For exact-input swaps this is `in_amount`; for exact-output it is `in_max`.
    /// Auth-deterministic: always transfers `user_max_in` from user, refunds excess.
    pub(super) fn swap_internal(
        e: &Env,
        sender: &Address,
        zero_for_one: bool,
        amount_specified: i128,
        sqrt_price_limit_x96: U256,
        user_max_in: u128,
    ) -> SwapResult {
        let exact_input = amount_specified > 0;
        let (
            amount_spec_used,
            amount_calculated,
            total_fee_amount,
            slot,
            liquidity,
            pf_delta_0,
            pf_delta_1,
        ) = Self::swap_loop(
            e,
            zero_for_one,
            amount_specified,
            sqrt_price_limit_x96,
            false,
        );

        let (amount0, amount1) = Self::swap_amounts_signed(
            zero_for_one,
            exact_input,
            amount_spec_used,
            amount_calculated,
        );

        // Token transfers (auth-deterministic: transfer user_max_in, refund excess).
        let token0 = get_token0(e);
        let token1 = get_token1(e);
        let contract = e.current_contract_address();

        let (in_token, out_token) = if zero_for_one {
            (&token0, &token1)
        } else {
            (&token1, &token0)
        };
        let actual_in = if zero_for_one {
            amount0 as u128
        } else {
            amount1 as u128
        };
        let actual_out = if zero_for_one {
            (-amount1) as u128
        } else {
            (-amount0) as u128
        };

        if actual_in > user_max_in {
            panic_with_error!(e, LiquidityPoolValidationError::InMaxNotSatisfied);
        }

        let in_client = SorobanTokenClient::new(e, in_token);
        if user_max_in > 0 {
            in_client.transfer(sender, &contract, &(user_max_in as i128));
        }
        let refund = user_max_in - actual_in;
        if refund > 0 {
            in_client.transfer(&contract, sender, &(refund as i128));
        }
        if actual_out > 0 {
            SorobanTokenClient::new(e, out_token).transfer(
                &contract,
                sender,
                &(actual_out as i128),
            );
        }

        // Reserve tracking: reserves change by net token flow minus protocol fee delta.
        // amount0/amount1: positive = user pays in, negative = user receives out.
        let mut res0 = get_reserve0(e);
        let mut res1 = get_reserve1(e);
        if amount0 > 0 {
            res0 += amount0 as u128 - pf_delta_0;
        } else if amount0 < 0 {
            res0 -= (-amount0) as u128;
        }
        if amount1 > 0 {
            res1 += amount1 as u128 - pf_delta_1;
        } else if amount1 < 0 {
            res1 -= (-amount1) as u128;
        }
        set_reserve0(e, &res0);
        set_reserve1(e, &res1);

        // Event emission.
        let (token_in, token_out, in_amount, out_amount) = if zero_for_one {
            (
                token0.clone(),
                token1.clone(),
                amount0.unsigned_abs(),
                (-amount1).unsigned_abs(),
            )
        } else {
            (
                token1.clone(),
                token0.clone(),
                amount1.unsigned_abs(),
                (-amount0).unsigned_abs(),
            )
        };
        let events = PoolEvents::new(e);
        events.trade(
            sender.clone(),
            token_in,
            token_out,
            in_amount,
            out_amount,
            total_fee_amount,
        );
        events.update_reserves(Vec::from_array(e, [res0, res1]));
        Self::emit_pool_state(e, &slot, liquidity);

        SwapResult {
            amount0,
            amount1,
            liquidity,
            sqrt_price_x96: slot.sqrt_price_x96,
            tick: slot.tick,
        }
    }
}
