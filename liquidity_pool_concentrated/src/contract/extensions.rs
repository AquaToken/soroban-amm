use super::*;

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

        let tokens = Vec::from_array(&e, [get_token0(&e), get_token1(&e)]);
        let amounts = Vec::from_array(&e, [amount0, amount1]);
        PoolEvents::new(&e).withdraw_liquidity(tokens, amounts, amount);

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

    fn get_tick_bitmap_batch(e: Env, start_word: i32, count: u32) -> Vec<U256> {
        let count = count.min(100);
        let mut result = Vec::new(&e);
        for i in 0..count {
            result.push_back(get_tick_bitmap_word(&e, start_word + i as i32));
        }
        result
    }

    fn get_ticks_batch(e: Env, ticks: Vec<i32>) -> Vec<TickInfo> {
        let max_ticks = ticks.len().min(100);
        let mut result = Vec::new(&e);
        for i in 0..max_ticks {
            result.push_back(get_tick(&e, ticks.get(i).unwrap()));
        }
        result
    }
}
