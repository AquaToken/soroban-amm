use super::*;

// Concentrated pool extensions — methods specific to tick-based liquidity.
// These are NOT available through the router; called directly on the pool contract.
#[contractimpl]
impl ConcentratedPoolExtensionsTrait for ConcentratedLiquidityPool {
    // Validates tick range: lower < upper, both within [MIN_TICK, MAX_TICK],
    // both aligned to tick_spacing.
    fn check_ticks(e: Env, tick_lower: i32, tick_upper: i32) -> Result<(), Error> {
        Self::check_ticks_internal(&e, tick_lower, tick_upper)
    }

    // Returns current ledger timestamp (seconds since epoch).
    fn block_timestamp(e: Env) -> u64 {
        e.ledger().timestamp()
    }

    // Sets the pool's initial price as sqrt(price) in Q64.96 format.
    // Can only be called when pool has zero liquidity (before first deposit
    // or after all positions are withdrawn). Operations admin or owner only.
    fn initialize_price(e: Env, admin: Address, sqrt_price_x96: U256) -> Result<(), Error> {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);

        if sqrt_price_x96 == U256::from_u32(&e, 0) {
            return Err(Error::InvalidSqrtPrice);
        }

        // Prevent price change when pool has active liquidity — would corrupt
        // tick accounting, fee growth, and active liquidity tracking.
        if get_total_raw_liquidity(&e) > 0 {
            return Err(Error::PoolAlreadyInitialized);
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

    // Advanced swap: specify tokens by address, signed amount (positive=exact_input,
    // negative=exact_output), and optional sqrt price limit. Returns signed amounts
    // (positive=paid by user, negative=received by user).
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

    // Add liquidity to a specific tick range [tick_lower, tick_upper).
    // `amount` is liquidity units (not token amounts). Transfers required tokens from
    // sender, creates/updates position for recipient. Returns (amount0, amount1) spent.
    // If range contains current price, both tokens needed; otherwise only one.
    // Accrues pending fees on existing position before adding.
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
        if amount > i128::MAX as u128 {
            return Err(Error::LiquidityAmountTooLarge);
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
        Self::ensure_user_range_exists(&e, &recipient, tick_lower, tick_upper)?;

        Self::update_tick_liquidity(&e, tick_lower, amount as i128, false)?;
        Self::update_tick_liquidity(&e, tick_upper, amount as i128, true)?;

        if slot.tick >= tick_lower && slot.tick < tick_upper {
            set_liquidity(&e, &get_liquidity(&e).saturating_add(amount));
        }

        Self::update_user_raw_liquidity(&e, &recipient, amount as i128);
        Self::recompute_user_weighted_liquidity(&e, &recipient);
        Self::rewards_refresh_working_balance(&e, &recipient);
        update_plane(&e);

        let tokens = Vec::from_array(&e, [token0, token1]);
        let amounts = Vec::from_array(&e, [amount0, amount1]);
        PoolEvents::new(&e).deposit_liquidity(tokens, amounts, amount);

        Ok((amount0, amount1))
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
        PoolEvents::new(&e).withdraw_liquidity(tokens, amounts, amount);

        Ok((amount0, amount1))
    }

    // Collect accrued swap fees from a position. Transfers up to amount0/1_requested
    // of owed tokens to recipient. Fees accumulate from swaps that occur while the
    // position's range contains the active price. Returns (amount0, amount1) collected.
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

    // Current price state: sqrt_price_x96 (Q64.96) and tick index.
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

    // Fee tier in basis points (e.g. 10 = 0.1%).
    fn fee(e: Env) -> u32 {
        get_fee(&e)
    }

    // Minimum distance between initialized ticks. Derived from fee tier.
    fn tick_spacing(e: Env) -> i32 {
        get_tick_spacing(&e)
    }

    // Bitmap word for tick scanning. Each bit represents an initialized tick.
    // word_pos = tick / (tick_spacing * 256).
    fn tick_bitmap(e: Env, word_pos: i32) -> U256 {
        get_tick_bitmap_word(&e, word_pos)
    }

    // Active liquidity — sum of all positions whose range contains current tick.
    // This is the liquidity used for swap math at the current price.
    fn liquidity(e: Env) -> u128 {
        get_liquidity(&e)
    }

    // Global cumulative fee growth per unit of liquidity for token0, in Q128 format.
    fn fee_growth_global_0_x128(e: Env) -> U256 {
        get_fee_growth_global_0_x128(&e)
    }

    // Global cumulative fee growth per unit of liquidity for token1, in Q128 format.
    fn fee_growth_global_1_x128(e: Env) -> U256 {
        get_fee_growth_global_1_x128(&e)
    }

    // Uncollected protocol fees (admin's cut of swap fees).
    fn protocol_fees(e: Env) -> ProtocolFees {
        get_protocol_fees(&e)
    }

    // Tick state (storage uses tuple encoding, converted to TickInfo at accessor boundary).
    fn ticks(e: Env, tick: i32) -> TickInfo {
        get_tick(&e, tick)
    }

    // Returns position data for a specific owner + tick range.
    // Panics with PositionNotFound if position doesn't exist.
    fn get_position(e: Env, recipient: Address, tick_lower: i32, tick_upper: i32) -> PositionData {
        match get_position(&e, &recipient, tick_lower, tick_upper) {
            Some(pos) => pos,
            None => panic_with_error!(&e, Error::PositionNotFound),
        }
    }

    // Full pool state in a single call: fee, liquidity, price, tick, spacing, tokens.
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

    // Pool state + actual token balances held by the contract.
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

    // Batch-fetch bitmap words for frontend tick scanning.
    fn get_tick_bitmap_batch(e: Env, start_word: i32, count: u32) -> Vec<U256> {
        let mut result = Vec::new(&e);
        for i in 0..count {
            result.push_back(get_tick_bitmap_word(&e, start_word + i as i32));
        }
        result
    }

    // Batch-fetch tick data for multiple tick indexes.
    fn get_ticks_batch(e: Env, ticks: Vec<i32>) -> Vec<TickInfo> {
        let mut result = Vec::new(&e);
        for i in 0..ticks.len() {
            result.push_back(get_tick(&e, ticks.get(i).unwrap()));
        }
        result
    }
}
