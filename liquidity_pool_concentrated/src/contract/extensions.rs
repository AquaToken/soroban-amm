use super::*;

// Concentrated pool extensions — methods specific to tick-based liquidity.
// These are NOT available through the router; called directly on the pool contract.
#[contractimpl]
impl ConcentratedPoolExtensionsTrait for ConcentratedLiquidityPool {
    // Read-only preview for custom-range deposit.
    // Returns (actual_amounts, liquidity) exactly as deposit_position would produce.
    fn estimate_deposit_position(
        e: Env,
        tick_lower: i32,
        tick_upper: i32,
        desired_amounts: Vec<u128>,
    ) -> (Vec<u128>, u128) {
        if desired_amounts.len() != 2 {
            panic_with_error!(&e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        let desired_amount0 = desired_amounts.get_unchecked(0);
        let desired_amount1 = desired_amounts.get_unchecked(1);
        if desired_amount0 == 0 && desired_amount1 == 0 {
            panic_with_error!(&e, LiquidityPoolValidationError::ZeroAmount);
        }

        Self::check_ticks_internal(&e, tick_lower, tick_upper);

        // Match deposit_position behavior on empty pool: find optimal price for the tick range.
        let mut slot = get_slot0(&e);
        if get_total_raw_liquidity(&e) == 0 && desired_amount0 > 0 && desired_amount1 > 0 {
            let (sqrt_price_x96, tick) = Self::init_sqrt_price_for_range(
                &e,
                tick_lower,
                tick_upper,
                desired_amount0,
                desired_amount1,
            );
            slot = Slot0 {
                sqrt_price_x96,
                tick,
            };
        }

        let liquidity = Self::max_liquidity_for_amounts_at_slot(
            &e,
            &slot,
            tick_lower,
            tick_upper,
            desired_amount0,
            desired_amount1,
        );
        if liquidity == 0 {
            panic_with_error!(&e, LiquidityPoolValidationError::ZeroAmount);
        }
        if liquidity > i128::MAX as u128 {
            panic_with_error!(&e, Error::LiquidityAmountTooLarge);
        }

        let (amount0, amount1) =
            Self::amounts_for_liquidity(&e, &slot, tick_lower, tick_upper, liquidity, true);

        (Vec::from_array(&e, [amount0, amount1]), liquidity)
    }

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
        min_liquidity: u128,
    ) -> (Vec<u128>, u128) {
        sender.require_auth();
        if get_is_killed_deposit(&e) {
            panic_with_error!(&e, Error::DepositKilled);
        }
        if desired_amounts.len() != 2 {
            panic_with_error!(&e, LiquidityPoolValidationError::WrongInputVecSize);
        }
        let desired_amount0 = desired_amounts.get_unchecked(0);
        let desired_amount1 = desired_amounts.get_unchecked(1);
        if desired_amount0 == 0 && desired_amount1 == 0 {
            panic_with_error!(&e, LiquidityPoolValidationError::ZeroAmount);
        }

        Self::check_ticks_internal(&e, tick_lower, tick_upper);
        let is_full_range = {
            let (full_range_lower, full_range_upper) = Self::full_range_ticks(&e);
            tick_lower == full_range_lower && tick_upper == full_range_upper
        };

        // Auto-initialize price on empty pool from token amount ratio.
        // First deposit MUST provide both tokens to establish the initial price.
        if get_total_raw_liquidity(&e) == 0 {
            if desired_amount0 > 0 && desired_amount1 > 0 {
                let (sqrt_price_x96, tick) = Self::init_sqrt_price_for_range(
                    &e,
                    tick_lower,
                    tick_upper,
                    desired_amount0,
                    desired_amount1,
                );
                set_slot0(
                    &e,
                    &Slot0 {
                        sqrt_price_x96,
                        tick,
                    },
                );
            } else {
                panic_with_error!(&e, LiquidityPoolValidationError::AllCoinsRequired);
            }
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
        );
        if liquidity == 0 {
            panic_with_error!(&e, LiquidityPoolValidationError::ZeroAmount);
        }
        if liquidity > i128::MAX as u128 {
            panic_with_error!(&e, Error::LiquidityAmountTooLarge);
        }
        if liquidity < min_liquidity {
            panic_with_error!(&e, LiquidityPoolValidationError::OutMinNotSatisfied);
        }

        // Compute actual token amounts for this liquidity (round up for pool safety)
        let slot = get_slot0(&e);
        let sqrt_lower = sqrt_ratio_at_tick(&e, tick_lower);
        let sqrt_upper = sqrt_ratio_at_tick(&e, tick_upper);

        let (amount0, amount1) = if slot.sqrt_price_x96 <= sqrt_lower {
            (
                amount0_delta(&e, &sqrt_lower, &sqrt_upper, liquidity, true),
                0,
            )
        } else if slot.sqrt_price_x96 < sqrt_upper {
            (
                amount0_delta(&e, &slot.sqrt_price_x96, &sqrt_upper, liquidity, true),
                amount1_delta(&e, &sqrt_lower, &slot.sqrt_price_x96, liquidity, true),
            )
        } else {
            (
                0,
                amount1_delta(&e, &sqrt_lower, &sqrt_upper, liquidity, true),
            )
        };

        let token0 = get_token0(&e);
        let token1 = get_token1(&e);
        let contract = e.current_contract_address();
        let token0_client = SorobanTokenClient::new(&e, &token0);
        let token1_client = SorobanTokenClient::new(&e, &token1);

        // Transfer full desired amounts (auth-deterministic: amounts are known at signing time),
        // then refund excess back to the sender.
        if desired_amount0 > 0 {
            token0_client.transfer(&sender, &contract, &(desired_amount0 as i128));
        }
        if desired_amount1 > 0 {
            token1_client.transfer(&sender, &contract, &(desired_amount1 as i128));
        }
        let refund0 = desired_amount0 - amount0;
        let refund1 = desired_amount1 - amount1;
        if refund0 > 0 {
            token0_client.transfer(&contract, &sender, &(refund0 as i128));
        }
        if refund1 > 0 {
            token1_client.transfer(&contract, &sender, &(refund1 as i128));
        }

        set_reserve0(&e, &(get_reserve0(&e) + amount0));
        set_reserve1(&e, &(get_reserve1(&e) + amount1));

        Self::update_tick_liquidity(&e, tick_lower, liquidity as i128, false);
        Self::update_tick_liquidity(&e, tick_upper, liquidity as i128, true);

        let mut position = Self::get_or_create_position(&e, &sender, tick_lower, tick_upper);
        Self::accrue_position_fees(&e, &mut position, tick_lower, tick_upper, slot.tick);
        position.liquidity = position.liquidity.saturating_add(liquidity);
        set_position(&e, &sender, tick_lower, tick_upper, &position);
        Self::ensure_user_range_exists(&e, &sender, tick_lower, tick_upper);

        if slot.tick >= tick_lower && slot.tick < tick_upper {
            set_liquidity(&e, &get_liquidity(&e).saturating_add(liquidity));
        }
        if is_full_range {
            let full_range_liquidity = get_full_range_liquidity(&e);
            let next_full_range_liquidity = match full_range_liquidity.checked_add(liquidity) {
                Some(value) => value,
                None => panic_with_error!(&e, Error::LiquidityOverflow),
            };
            set_full_range_liquidity(&e, &next_full_range_liquidity);
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

        (Vec::from_array(&e, [amount0, amount1]), liquidity)
    }

    // Read-only preview for custom-range withdrawal total.
    // Returns burn principal + fees that will be auto-claimed by withdraw_position.
    fn estimate_withdraw_position(
        e: Env,
        owner: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount: u128,
    ) -> Vec<u128> {
        if amount == 0 {
            panic_with_error!(&e, LiquidityPoolValidationError::ZeroAmount);
        }
        if amount > i128::MAX as u128 {
            panic_with_error!(&e, Error::LiquidityAmountTooLarge);
        }

        Self::check_ticks_internal(&e, tick_lower, tick_upper);

        let mut position = match get_position(&e, &owner, tick_lower, tick_upper) {
            Some(pos) => pos,
            None => panic_with_error!(&e, Error::PositionNotFound),
        };
        if position.liquidity < amount {
            panic_with_error!(&e, Error::InsufficientLiquidity);
        }

        let slot = get_slot0(&e);
        Self::accrue_position_fees(&e, &mut position, tick_lower, tick_upper, slot.tick);

        let (amount0, amount1) =
            Self::amounts_for_liquidity(&e, &slot, tick_lower, tick_upper, amount, false);

        let total_amount0 = match amount0.checked_add(position.tokens_owed_0) {
            Some(v) => v,
            None => panic_with_error!(&e, Error::InvalidAmount),
        };
        let total_amount1 = match amount1.checked_add(position.tokens_owed_1) {
            Some(v) => v,
            None => panic_with_error!(&e, Error::InvalidAmount),
        };

        if get_reserve0(&e) < total_amount0 {
            panic_with_error!(&e, LiquidityPoolValidationError::InsufficientBalance);
        }
        if get_reserve1(&e) < total_amount1 {
            panic_with_error!(&e, LiquidityPoolValidationError::InsufficientBalance);
        }

        Vec::from_array(&e, [total_amount0, total_amount1])
    }

    // Remove liquidity from a position and transfer tokens directly to owner wallet.
    // This call always auto-claims all accrued fees for the position as well.
    // Returns (amount0, amount1) total transferred by this withdraw call (principal + fees).
    fn withdraw_position(
        e: Env,
        owner: Address,
        tick_lower: i32,
        tick_upper: i32,
        amount: u128,
        min_amounts: Vec<u128>,
    ) -> Vec<u128> {
        owner.require_auth();
        if amount == 0 {
            panic_with_error!(&e, LiquidityPoolValidationError::ZeroAmount);
        }
        if amount > i128::MAX as u128 {
            panic_with_error!(&e, Error::LiquidityAmountTooLarge);
        }
        if min_amounts.len() != 2 {
            panic_with_error!(&e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        Self::check_ticks_internal(&e, tick_lower, tick_upper);
        let is_full_range = {
            let (full_range_lower, full_range_upper) = Self::full_range_ticks(&e);
            tick_lower == full_range_lower && tick_upper == full_range_upper
        };

        Self::recompute_user_weighted_liquidity(&e, &owner);
        Self::rewards_checkpoint_user(&e, &owner);

        let mut position = match get_position(&e, &owner, tick_lower, tick_upper) {
            Some(pos) => pos,
            None => panic_with_error!(&e, Error::PositionNotFound),
        };

        let slot = get_slot0(&e);
        Self::accrue_position_fees(&e, &mut position, tick_lower, tick_upper, slot.tick);

        if position.liquidity < amount {
            panic_with_error!(&e, Error::InsufficientLiquidity);
        }

        let sqrt_lower = sqrt_ratio_at_tick(&e, tick_lower);
        let sqrt_upper = sqrt_ratio_at_tick(&e, tick_upper);

        let (amount0, amount1) = if slot.sqrt_price_x96 <= sqrt_lower {
            (
                amount0_delta(&e, &sqrt_lower, &sqrt_upper, amount, false),
                0,
            )
        } else if slot.sqrt_price_x96 < sqrt_upper {
            (
                amount0_delta(&e, &slot.sqrt_price_x96, &sqrt_upper, amount, false),
                amount1_delta(&e, &sqrt_lower, &slot.sqrt_price_x96, amount, false),
            )
        } else {
            (
                0,
                amount1_delta(&e, &sqrt_lower, &sqrt_upper, amount, false),
            )
        };
        let fees0 = position.tokens_owed_0;
        let fees1 = position.tokens_owed_1;
        let total_amount0 = match amount0.checked_add(fees0) {
            Some(v) => v,
            None => panic_with_error!(&e, Error::InvalidAmount),
        };
        let total_amount1 = match amount1.checked_add(fees1) {
            Some(v) => v,
            None => panic_with_error!(&e, Error::InvalidAmount),
        };

        if total_amount0 < min_amounts.get_unchecked(0)
            || total_amount1 < min_amounts.get_unchecked(1)
        {
            panic_with_error!(&e, LiquidityPoolValidationError::OutMinNotSatisfied);
        }

        let reserve0_before = get_reserve0(&e);
        let reserve1_before = get_reserve1(&e);
        if reserve0_before < total_amount0 {
            panic_with_error!(&e, LiquidityPoolValidationError::InsufficientBalance);
        }
        if reserve1_before < total_amount1 {
            panic_with_error!(&e, LiquidityPoolValidationError::InsufficientBalance);
        }

        position.liquidity -= amount;
        position.tokens_owed_0 = 0;
        position.tokens_owed_1 = 0;

        if position.liquidity == 0 {
            remove_position(&e, &owner, tick_lower, tick_upper);
            Self::remove_user_range_if_empty(&e, &owner, tick_lower, tick_upper);
        } else {
            set_position(&e, &owner, tick_lower, tick_upper, &position);
        }

        Self::update_tick_liquidity(&e, tick_lower, -(amount as i128), false);
        Self::update_tick_liquidity(&e, tick_upper, -(amount as i128), true);

        if slot.tick >= tick_lower && slot.tick < tick_upper {
            let active = get_liquidity(&e);
            if active < amount {
                panic_with_error!(&e, Error::LiquidityUnderflow);
            }
            set_liquidity(&e, &(active - amount));
        }
        if is_full_range {
            let full_range_liquidity = get_full_range_liquidity(&e);
            if full_range_liquidity < amount {
                panic_with_error!(&e, Error::LiquidityUnderflow);
            }
            set_full_range_liquidity(&e, &(full_range_liquidity - amount));
        }

        Self::update_user_raw_liquidity(&e, &owner, -(amount as i128));
        Self::recompute_user_weighted_liquidity(&e, &owner);
        Self::rewards_refresh_working_balance(&e, &owner);

        let token0 = get_token0(&e);
        let token1 = get_token1(&e);
        let contract = e.current_contract_address();
        if total_amount0 > 0 {
            SorobanTokenClient::new(&e, &token0).transfer(
                &contract,
                &owner,
                &(total_amount0 as i128),
            );
        }
        if total_amount1 > 0 {
            SorobanTokenClient::new(&e, &token1).transfer(
                &contract,
                &owner,
                &(total_amount1 as i128),
            );
        }

        let reserve0_after = reserve0_before - total_amount0;
        let reserve1_after = reserve1_before - total_amount1;
        set_reserve0(&e, &reserve0_after);
        set_reserve1(&e, &reserve1_after);
        update_plane(&e);

        let tokens = Vec::from_array(&e, [get_token0(&e), get_token1(&e)]);
        let amounts = Vec::from_array(&e, [total_amount0, total_amount1]);
        let events = PoolEvents::new(&e);
        events.withdraw_liquidity(tokens, amounts, amount);
        events.update_reserves(Vec::from_array(&e, [reserve0_after, reserve1_after]));
        Self::emit_position_update(&e, &owner, tick_lower, tick_upper, -(amount as i128));
        Self::emit_pool_state(&e, &slot, get_liquidity(&e));

        Vec::from_array(&e, [total_amount0, total_amount1])
    }

    // Read-only preview for currently claimable swap fees on a single position.
    // Returns current tokens_owed values after fee accrual at current tick.
    fn get_position_fees(e: Env, owner: Address, tick_lower: i32, tick_upper: i32) -> Vec<u128> {
        Self::check_ticks_internal(&e, tick_lower, tick_upper);

        let mut position = match get_position(&e, &owner, tick_lower, tick_upper) {
            Some(pos) => pos,
            None => panic_with_error!(&e, Error::PositionNotFound),
        };
        let tick_current = get_slot0(&e).tick;
        Self::accrue_position_fees(&e, &mut position, tick_lower, tick_upper, tick_current);

        Vec::from_array(&e, [position.tokens_owed_0, position.tokens_owed_1])
    }

    // Collect accrued swap fees from a position. Transfers up to amount0/1_requested
    // of owed tokens to owner. Fees accumulate from swaps that occur while the
    // position's range contains the active price. Returns (amount0, amount1) collected.
    fn claim_position_fees(e: Env, owner: Address, tick_lower: i32, tick_upper: i32) -> Vec<u128> {
        Self::collect_internal(
            &e,
            &owner,
            tick_lower,
            tick_upper,
            Vec::from_array(&e, [u128::MAX, u128::MAX]),
            true,
        )
    }

    // Read-only preview of total claimable fees/tokens_owed across all user positions.
    fn get_all_position_fees(e: Env, owner: Address) -> Vec<u128> {
        let ranges = get_user_state(&e, &owner).positions;
        if ranges.len() == 0 {
            return Vec::from_array(&e, [0u128, 0u128]);
        }

        let tick_current = get_slot0(&e).tick;
        let mut total0 = 0u128;
        let mut total1 = 0u128;

        for i in 0..ranges.len() {
            let range = ranges.get_unchecked(i);
            if let Some(mut position) = get_position(&e, &owner, range.tick_lower, range.tick_upper)
            {
                Self::accrue_position_fees(
                    &e,
                    &mut position,
                    range.tick_lower,
                    range.tick_upper,
                    tick_current,
                );
                total0 = total0.saturating_add(position.tokens_owed_0);
                total1 = total1.saturating_add(position.tokens_owed_1);
            }
        }

        Vec::from_array(&e, [total0, total1])
    }

    // Collect all currently claimable fees/tokens_owed across all user positions.
    // Useful for one-click "claim all fees" UX.
    fn claim_all_position_fees(e: Env, owner: Address) -> Vec<u128> {
        owner.require_auth();

        let ranges = get_user_state(&e, &owner).positions;
        if ranges.len() == 0 {
            return Vec::from_array(&e, [0u128, 0u128]);
        }

        let tick_current = get_slot0(&e).tick;
        let mut total0 = 0u128;
        let mut total1 = 0u128;

        for i in 0..ranges.len() {
            let range = ranges.get_unchecked(i);
            let mut position = match get_position(&e, &owner, range.tick_lower, range.tick_upper) {
                Some(pos) => pos,
                None => continue,
            };

            Self::accrue_position_fees(
                &e,
                &mut position,
                range.tick_lower,
                range.tick_upper,
                tick_current,
            );

            total0 = total0.saturating_add(position.tokens_owed_0);
            total1 = total1.saturating_add(position.tokens_owed_1);

            position.tokens_owed_0 = 0;
            position.tokens_owed_1 = 0;
            if position.liquidity == 0 {
                remove_position(&e, &owner, range.tick_lower, range.tick_upper);
                Self::remove_user_range_if_empty(&e, &owner, range.tick_lower, range.tick_upper);
            } else {
                set_position(&e, &owner, range.tick_lower, range.tick_upper, &position);
            }
        }

        let reserve0 = get_reserve0(&e);
        let reserve1 = get_reserve1(&e);
        if reserve0 < total0 {
            panic_with_error!(&e, LiquidityPoolValidationError::InsufficientBalance);
        }
        if reserve1 < total1 {
            panic_with_error!(&e, LiquidityPoolValidationError::InsufficientBalance);
        }

        let token0 = get_token0(&e);
        let token1 = get_token1(&e);
        let contract = e.current_contract_address();

        if total0 > 0 {
            SorobanTokenClient::new(&e, &token0).transfer(&contract, &owner, &(total0 as i128));
        }
        if total1 > 0 {
            SorobanTokenClient::new(&e, &token1).transfer(&contract, &owner, &(total1 as i128));
        }

        set_reserve0(&e, &(reserve0 - total0));
        set_reserve1(&e, &(reserve1 - total1));
        PoolEvents::new(&e)
            .update_reserves(Vec::from_array(&e, [reserve0 - total0, reserve1 - total1]));
        update_plane(&e);

        Vec::from_array(&e, [total0, total1])
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

    // Compute the tick for a given token amount ratio.
    // tick = tick_at_sqrt_ratio(sqrt(amount1 / amount0) * 2^96)
    // Useful for frontends to determine the initial price tick before the first deposit.
    fn tick_from_amounts(e: Env, amount0: u128, amount1: u128) -> i32 {
        let sqrt_price = sqrt_price_from_amounts(&e, amount0, amount1);
        tick_at_sqrt_ratio(&e, &sqrt_price)
    }
}
