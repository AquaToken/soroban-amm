use super::*;

// Concentrated pool extensions — methods specific to tick-based liquidity.
// These are NOT available through the router; called directly on the pool contract.
#[contractimpl]
impl ConcentratedPoolExtensionsTrait for ConcentratedLiquidityPool {
    // Add liquidity to a specific tick range [tick_lower, tick_upper).
    // `desired_amounts` is [amount0, amount1] — the maximum tokens the sender is willing
    // to spend. The contract computes the maximum liquidity mintable from these amounts
    // at the current price and transfers only the actual tokens needed.
    // Returns (actual_amounts: Vec<u128>, liquidity: u128).
    // On an empty pool (zero total liquidity), the price is auto-initialized from the
    // token ratio — no separate initialize_price call needed.
    fn deposit_position(
        e: Env,
        sender: Address,
        tick_lower: i32,
        tick_upper: i32,
        desired_amounts: Vec<u128>,
    ) -> Result<(Vec<u128>, u128), Error> {
        sender.require_auth();
        if get_is_killed_deposit(&e) {
            return Err(Error::DepositKilled);
        }
        if desired_amounts.len() != 2 {
            return Err(Error::InvalidAmount);
        }
        let desired_amount0 = desired_amounts.get_unchecked(0);
        let desired_amount1 = desired_amounts.get_unchecked(1);
        if desired_amount0 == 0 && desired_amount1 == 0 {
            return Err(Error::AmountShouldBeGreaterThanZero);
        }

        Self::check_ticks_internal(&e, tick_lower, tick_upper)?;

        // Auto-initialize price on empty pool from token ratio
        if get_total_raw_liquidity(&e) == 0 && desired_amount0 > 0 && desired_amount1 > 0 {
            let sqrt_price_x96 = sqrt_price_from_amounts(&e, desired_amount0, desired_amount1)?;
            let tick = tick_at_sqrt_ratio(&e, &sqrt_price_x96)?;
            set_slot0(
                &e,
                &Slot0 {
                    sqrt_price_x96,
                    tick,
                },
            );
        }

        Self::recompute_user_weighted_liquidity(&e, &sender);
        Self::rewards_checkpoint_user(&e, &sender);

        // Compute max liquidity from desired token amounts at current price
        let liquidity = Self::max_liquidity_for_amounts(
            &e,
            tick_lower,
            tick_upper,
            desired_amount0,
            desired_amount1,
        )?;
        if liquidity == 0 {
            return Err(Error::AmountShouldBeGreaterThanZero);
        }
        if liquidity > i128::MAX as u128 {
            return Err(Error::LiquidityAmountTooLarge);
        }

        // Compute actual token amounts for this liquidity (round up for pool safety)
        let slot = get_slot0(&e);
        let sqrt_lower = sqrt_ratio_at_tick(&e, tick_lower)?;
        let sqrt_upper = sqrt_ratio_at_tick(&e, tick_upper)?;

        let (amount0, amount1) = if slot.sqrt_price_x96 <= sqrt_lower {
            (
                amount0_delta(&e, &sqrt_lower, &sqrt_upper, liquidity, true)?,
                0,
            )
        } else if slot.sqrt_price_x96 < sqrt_upper {
            (
                amount0_delta(&e, &slot.sqrt_price_x96, &sqrt_upper, liquidity, true)?,
                amount1_delta(&e, &sqrt_lower, &slot.sqrt_price_x96, liquidity, true)?,
            )
        } else {
            (
                0,
                amount1_delta(&e, &sqrt_lower, &sqrt_upper, liquidity, true)?,
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

        set_reserve0(&e, &(get_reserve0(&e) + amount0));
        set_reserve1(&e, &(get_reserve1(&e) + amount1));

        let mut position = Self::get_or_create_position(&e, &sender, tick_lower, tick_upper);
        Self::accrue_position_fees(&e, &mut position, tick_lower, tick_upper, slot.tick)?;
        position.liquidity = position.liquidity.saturating_add(liquidity);
        set_position(&e, &sender, tick_lower, tick_upper, &position);
        Self::ensure_user_range_exists(&e, &sender, tick_lower, tick_upper)?;

        Self::update_tick_liquidity(&e, tick_lower, liquidity as i128, false)?;
        Self::update_tick_liquidity(&e, tick_upper, liquidity as i128, true)?;

        if slot.tick >= tick_lower && slot.tick < tick_upper {
            set_liquidity(&e, &get_liquidity(&e).saturating_add(liquidity));
        }

        Self::update_user_raw_liquidity(&e, &sender, liquidity as i128);
        Self::recompute_user_weighted_liquidity(&e, &sender);
        Self::rewards_refresh_working_balance(&e, &sender);
        update_plane(&e);

        let tokens = Vec::from_array(&e, [token0, token1]);
        let amounts = Vec::from_array(&e, [amount0, amount1]);
        let events = PoolEvents::new(&e);
        events.deposit_liquidity(tokens, amounts, liquidity);
        events.update_reserves(Vec::from_array(&e, [get_reserve0(&e), get_reserve1(&e)]));
        Self::emit_position_update(&e, &sender, tick_lower, tick_upper, liquidity as i128);
        Self::emit_pool_state(&e, &get_slot0(&e), get_liquidity(&e));

        Ok((Vec::from_array(&e, [amount0, amount1]), liquidity))
    }

    // Remove liquidity from a position. Withdrawn tokens + accrued fees are credited
    // to position's tokens_owed fields — call claim_position_fees to actually transfer.
    // Returns (amount0, amount1) that were credited. If position is fully withdrawn
    // and has no owed tokens, it is deleted.
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
        if amount > i128::MAX as u128 {
            return Err(Error::LiquidityAmountTooLarge);
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

        let tokens = Vec::from_array(&e, [get_token0(&e), get_token1(&e)]);
        let amounts = Vec::from_array(&e, [amount0, amount1]);
        let events = PoolEvents::new(&e);
        events.withdraw_liquidity(tokens, amounts, amount);
        Self::emit_position_update(&e, &owner, tick_lower, tick_upper, -(amount as i128));
        Self::emit_pool_state(&e, &slot, get_liquidity(&e));

        Ok((amount0, amount1))
    }

    // Collect accrued swap fees from a position. Transfers up to amount0/1_requested
    // of owed tokens to owner. Fees accumulate from swaps that occur while the
    // position's range contains the active price. Returns (amount0, amount1) collected.
    fn claim_position_fees(
        e: Env,
        owner: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount0_requested: u128,
        amount1_requested: u128,
    ) -> Result<(u128, u128), Error> {
        Self::collect_internal(
            &e,
            &owner,
            tick_lower,
            tick_upper,
            amount0_requested,
            amount1_requested,
            true,
        )
    }

    // Current price state: sqrt_price_x96 (Q64.96) and tick index.
    fn get_slot0(e: Env) -> Slot0 {
        get_slot0(&e)
    }

    // Minimum distance between initialized ticks. Derived from fee tier.
    fn get_tick_spacing(e: Env) -> i32 {
        get_tick_spacing(&e)
    }

    // Chunk bitmap word. Each bit represents a chunk that has at least one initialized tick.
    // word_pos = chunk_pos >> 8.
    fn get_chunk_bitmap(e: Env, word_pos: i32) -> U256 {
        get_chunk_bitmap_word(&e, word_pos)
    }

    // Active liquidity — sum of all positions whose range contains current tick.
    // This is the liquidity used for swap math at the current price.
    fn get_active_liquidity(e: Env) -> u128 {
        get_liquidity(&e)
    }

    // Global cumulative fee growth per unit of liquidity for token0, in Q128 format.
    fn get_fee_growth_global_0_x128(e: Env) -> U256 {
        get_fee_growth_global_0_x128(&e)
    }

    // Global cumulative fee growth per unit of liquidity for token1, in Q128 format.
    fn get_fee_growth_global_1_x128(e: Env) -> U256 {
        get_fee_growth_global_1_x128(&e)
    }

    // Tick state (stored in chunks, converted to TickInfo at accessor boundary).
    fn get_tick(e: Env, tick: i32) -> TickInfo {
        get_tick(&e, tick, get_tick_spacing(&e))
    }

    // Returns position data for a specific owner + tick range.
    // Panics with PositionNotFound if position doesn't exist.
    fn get_position(e: Env, recipient: Address, tick_lower: i32, tick_upper: i32) -> PositionData {
        match get_position(&e, &recipient, tick_lower, tick_upper) {
            Some(pos) => pos,
            None => panic_with_error!(&e, Error::PositionNotFound),
        }
    }

    // User's position ranges, raw liquidity, and weighted liquidity (for rewards).
    fn get_user_position_snapshot(e: Env, user: Address) -> UserPositionSnapshot {
        let state = get_user_state(&e, &user);
        UserPositionSnapshot {
            ranges: state.positions,
            raw_liquidity: state.raw_liquidity,
            weighted_liquidity: state.weighted_liquidity,
        }
    }

    // Total weighted liquidity across all users (used for rewards distribution).
    // Weighted = raw * distance_multiplier, where narrower ranges near price get higher weight.
    fn get_total_weighted_liquidity(e: Env) -> u128 {
        get_total_weighted_liquidity(&e)
    }

    // Total raw (unweighted) liquidity across all users.
    fn get_total_raw_liquidity(e: Env) -> u128 {
        get_total_raw_liquidity(&e)
    }

    // Batch-fetch chunk bitmap words for frontend scanning.
    fn get_chunk_bitmap_batch(e: Env, start_word: i32, count: u32) -> Vec<U256> {
        let mut result = Vec::new(&e);
        for i in 0..count {
            result.push_back(get_chunk_bitmap_word(&e, start_word + i as i32));
        }
        result
    }

    // Batch-fetch tick data for multiple tick indexes.
    fn get_ticks_batch(e: Env, ticks: Vec<i32>) -> Vec<TickInfo> {
        let spacing = get_tick_spacing(&e);
        let mut result = Vec::new(&e);
        for i in 0..ticks.len() {
            result.push_back(get_tick(&e, ticks.get(i).unwrap(), spacing));
        }
        result
    }
}
