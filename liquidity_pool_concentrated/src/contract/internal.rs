use super::*;

impl ConcentratedLiquidityPool {
    pub(super) fn has_admin_role(e: &Env) -> bool {
        AccessControl::new(e).get_role_safe(&Role::Admin).is_some()
    }

    pub(super) fn require_admin(e: &Env, admin: &Address) {
        admin.require_auth();
        AccessControl::new(e).assert_address_has_role(admin, &Role::Admin);
    }

    pub(super) fn check_ticks_internal(
        e: &Env,
        tick_lower: i32,
        tick_upper: i32,
    ) -> Result<(), Error> {
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

    pub(super) fn abs_i128(v: i128) -> u128 {
        if v < 0 {
            (-v) as u128
        } else {
            v as u128
        }
    }

    pub(super) fn u128_to_i128(v: u128) -> Result<i128, Error> {
        if v > i128::MAX as u128 {
            return Err(Error::LiquidityOverflow);
        }
        Ok(v as i128)
    }

    pub(super) fn u256_to_array(v: &U256) -> [u8; 32] {
        let bytes = v.to_be_bytes();
        let mut out = [0u8; 32];
        bytes.copy_into_slice(&mut out);
        out
    }

    pub(super) fn u256_from_array(e: &Env, bytes: &[u8; 32]) -> U256 {
        U256::from_be_bytes(e, &Bytes::from_array(e, bytes))
    }

    pub(super) fn bit_is_set(word: &[u8; 32], bit_pos: u32) -> bool {
        if bit_pos >= 256 {
            return false;
        }

        let byte_idx = 31usize - (bit_pos / 8) as usize;
        let bit_idx = (bit_pos % 8) as u8;
        (word[byte_idx] & (1u8 << bit_idx)) != 0
    }

    pub(super) fn set_bit(word: &mut [u8; 32], bit_pos: u32, value: bool) {
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

    pub(super) fn find_prev_set_bit(word: &[u8; 32], from_bit: u32) -> Option<u32> {
        let from_bit = from_bit.min(255);
        // Big-endian: byte 0 = bits 255..248, byte 31 = bits 7..0
        let start_byte = (255 - from_bit) / 8;
        let start_bit_in_byte = from_bit % 8;

        // Check the first (partial) byte — mask off bits above from_bit
        let mask = ((1u16 << (start_bit_in_byte + 1)) - 1) as u8;
        let masked = word[start_byte as usize] & mask;
        if masked != 0 {
            let top_bit = 7 - masked.leading_zeros();
            return Some((31 - start_byte) * 8 + top_bit);
        }

        // Scan remaining bytes downward (higher byte index = lower bits)
        for byte_idx in (start_byte + 1)..32 {
            if word[byte_idx as usize] != 0 {
                let top_bit = 7 - word[byte_idx as usize].leading_zeros();
                return Some((31 - byte_idx) * 8 + top_bit);
            }
        }

        None
    }

    pub(super) fn find_next_set_bit(word: &[u8; 32], from_bit: u32) -> Option<u32> {
        let from_bit = from_bit.min(255);
        let start_byte = (255 - from_bit) / 8;
        let start_bit_in_byte = from_bit % 8;

        // Check the first (partial) byte — mask off bits below from_bit
        let mask = !((1u8 << start_bit_in_byte).wrapping_sub(1));
        let masked = word[start_byte as usize] & mask;
        if masked != 0 {
            let low_bit = masked.trailing_zeros();
            return Some((31 - start_byte) * 8 + low_bit);
        }

        // Scan remaining bytes upward (lower byte index = higher bits)
        if start_byte > 0 {
            for byte_idx in (0..start_byte).rev() {
                if word[byte_idx as usize] != 0 {
                    let low_bit = word[byte_idx as usize].trailing_zeros();
                    return Some((31 - byte_idx) * 8 + low_bit);
                }
            }
        }

        None
    }

    pub(super) fn compress_tick(tick: i32, spacing: i32) -> i32 {
        let mut compressed = tick / spacing;
        if tick < 0 && tick % spacing != 0 {
            compressed -= 1;
        }
        compressed
    }

    pub(super) fn position(compressed_tick: i32) -> (i32, u32) {
        let word_pos = compressed_tick >> 8;
        let bit_pos = (compressed_tick & 255) as u32;
        (word_pos, bit_pos)
    }

    pub(super) fn set_tick_bitmap_bit(e: &Env, tick_idx: i32, spacing: i32, initialized: bool) {
        let compressed = Self::compress_tick(tick_idx, spacing);
        let (word_pos, bit_pos) = Self::position(compressed);

        let mut word = Self::u256_to_array(&get_tick_bitmap_word(e, word_pos));
        Self::set_bit(&mut word, bit_pos, initialized);
        set_tick_bitmap_word(e, word_pos, &Self::u256_from_array(e, &word));
    }

    pub(super) fn find_initialized_tick_in_word(
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

    pub(super) fn update_tick_liquidity(
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
            Self::set_tick_bitmap_bit(e, tick_idx, get_tick_spacing(e), true);
        } else if prev_initialized && !tick.initialized {
            Self::set_tick_bitmap_bit(e, tick_idx, get_tick_spacing(e), false);
        }

        set_tick(e, tick_idx, &tick);
        Ok(())
    }

    pub(super) fn ensure_user_range_exists(
        e: &Env,
        user: &Address,
        tick_lower: i32,
        tick_upper: i32,
    ) -> Result<(), Error> {
        let mut ranges = get_user_positions(e, user);
        for range in ranges.iter() {
            if range.tick_lower == tick_lower && range.tick_upper == tick_upper {
                return Ok(());
            }
        }

        if ranges.len() >= MAX_USER_POSITIONS {
            return Err(Error::TooManyPositions);
        }

        ranges.push_back(PositionRange {
            tick_lower,
            tick_upper,
        });
        set_user_positions(e, user, &ranges);
        Ok(())
    }

    pub(super) fn remove_user_range_if_empty(
        e: &Env,
        user: &Address,
        tick_lower: i32,
        tick_upper: i32,
    ) {
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

    pub(super) fn recompute_user_weighted_liquidity(e: &Env, user: &Address) -> u128 {
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

    pub(super) fn update_user_raw_liquidity(e: &Env, user: &Address, delta: i128) {
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
            manager.get_working_balance(user, 0),
            manager.get_working_supply(0),
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

        let lower = get_tick(e, tick_lower);
        let upper = get_tick(e, tick_upper);

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
    ) -> Result<(), Error> {
        let (inside_0, inside_1) =
            Self::compute_fee_growth_inside(e, tick_lower, tick_upper, tick_current);

        let delta_0 = wrapping_sub_u256(e, &inside_0, &position.fee_growth_inside_0_last_x128);
        let delta_1 = wrapping_sub_u256(e, &inside_1, &position.fee_growth_inside_1_last_x128);

        let owed_0 = mul_div_fee_growth(e, &delta_0, position.liquidity)?;
        let owed_1 = mul_div_fee_growth(e, &delta_1, position.liquidity)?;

        position.tokens_owed_0 = position.tokens_owed_0.saturating_add(owed_0);
        position.tokens_owed_1 = position.tokens_owed_1.saturating_add(owed_1);
        position.fee_growth_inside_0_last_x128 = inside_0;
        position.fee_growth_inside_1_last_x128 = inside_1;

        Ok(())
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

    pub(super) fn collect_internal(
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

    pub(super) fn cross_tick(e: &Env, tick_idx: i32) -> i128 {
        let mut tick = get_tick(e, tick_idx);
        let fee_growth_global_0 = get_fee_growth_global_0_x128(e);
        let fee_growth_global_1 = get_fee_growth_global_1_x128(e);

        tick.fee_growth_outside_0_x128 =
            wrapping_sub_u256(e, &fee_growth_global_0, &tick.fee_growth_outside_0_x128);
        tick.fee_growth_outside_1_x128 =
            wrapping_sub_u256(e, &fee_growth_global_1, &tick.fee_growth_outside_1_x128);

        let liquidity_net = tick.liquidity_net;
        set_tick(e, tick_idx, &tick);
        liquidity_net
    }

    pub(super) fn add_fee_growth_global(
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
            let next = wrapping_add_u256(e, &get_fee_growth_global_0_x128(e), &growth_delta);
            set_fee_growth_global_0_x128(e, &next);
        } else {
            let next = wrapping_add_u256(e, &get_fee_growth_global_1_x128(e), &growth_delta);
            set_fee_growth_global_1_x128(e, &next);
        }

        Ok(())
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
                let computed = get_next_sqrt_price_from_input(
                    e,
                    sqrt_current,
                    liquidity,
                    amount_remaining_less_fee,
                    zero_for_one,
                )?;
                // Clamp to [target, current] range
                if zero_for_one {
                    computed.max(sqrt_target.clone())
                } else {
                    computed.min(sqrt_target.clone())
                }
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
                let computed = get_next_sqrt_price_from_output(
                    e,
                    sqrt_current,
                    liquidity,
                    amount_remaining,
                    zero_for_one,
                )?;
                // Clamp to [target, current] range
                if zero_for_one {
                    computed.max(sqrt_target.clone())
                } else {
                    computed.min(sqrt_target.clone())
                }
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

    pub(super) fn validate_price_limit(
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

    pub(super) fn direction_from_indexes(in_idx: u32, out_idx: u32) -> Result<bool, Error> {
        if in_idx > 1 || out_idx > 1 || in_idx == out_idx {
            return Err(Error::InvalidAmount);
        }
        Ok(in_idx == 0 && out_idx == 1)
    }

    pub(super) fn direction_from_tokens(
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

    pub(super) fn full_range_ticks(e: &Env) -> Result<(i32, i32), Error> {
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

    pub(super) fn amounts_for_liquidity(
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

    pub(super) fn max_liquidity_for_amounts(
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
        let sqrt_lower = sqrt_ratio_at_tick(e, tick_lower)?;
        let sqrt_upper = sqrt_ratio_at_tick(e, tick_upper)?;

        // Analytical formulas (inverse of amount0_delta / amount1_delta):
        // - Below range:  only token0 needed → L = liquidity_for_amount0(sqrtLower, sqrtUpper, amount0)
        // - Above range:  only token1 needed → L = liquidity_for_amount1(sqrtLower, sqrtUpper, amount1)
        // - In range:     L = min(L0_from_current_to_upper, L1_from_lower_to_current)
        if slot.sqrt_price_x96 <= sqrt_lower {
            liquidity_for_amount0(e, &sqrt_lower, &sqrt_upper, desired_amount0)
        } else if slot.sqrt_price_x96 >= sqrt_upper {
            liquidity_for_amount1(e, &sqrt_lower, &sqrt_upper, desired_amount1)
        } else {
            let l0 =
                liquidity_for_amount0(e, &slot.sqrt_price_x96, &sqrt_upper, desired_amount0)?;
            let l1 =
                liquidity_for_amount1(e, &sqrt_lower, &slot.sqrt_price_x96, desired_amount1)?;
            Ok(l0.min(l1))
        }
    }

    pub(super) fn simulate_swap_amounts(
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

            let (next_tick, next_tick_initialized) =
                Self::find_initialized_tick_in_word(e, slot.tick, tick_spacing, zero_for_one);
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

    pub(super) fn swap_internal(
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
        let mut total_fee_amount: u128 = 0;
        let tick_spacing = get_tick_spacing(e);

        while amount_remaining > 0 && slot.sqrt_price_x96 != price_limit {
            if liquidity == 0 {
                break;
            }

            let (next_tick, next_tick_initialized) =
                Self::find_initialized_tick_in_word(e, slot.tick, tick_spacing, zero_for_one);
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

            total_fee_amount = total_fee_amount.saturating_add(step.fee_amount);
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

        let (token_in, token_out, in_amount, out_amount) = if zero_for_one {
            (
                token0.clone(),
                token1.clone(),
                amount0.unsigned_abs() as u128,
                (-amount1).unsigned_abs() as u128,
            )
        } else {
            (
                token1.clone(),
                token0.clone(),
                amount1.unsigned_abs() as u128,
                (-amount0).unsigned_abs() as u128,
            )
        };
        PoolEvents::new(e).trade(
            sender.clone(),
            token_in,
            token_out,
            in_amount,
            out_amount,
            total_fee_amount,
        );

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
