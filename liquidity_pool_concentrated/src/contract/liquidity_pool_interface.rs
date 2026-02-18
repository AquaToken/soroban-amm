use super::*;

// Standard pool interface — router-compatible methods shared across all pool types.
// deposit/withdraw use full-range positions for compatibility with the router's
// uniform LP token model. For custom ranges, use deposit_position/withdraw_position.
#[contractimpl]
impl LiquidityPoolInterfaceTrait for ConcentratedLiquidityPool {
    // Returns "concentrated" — used by router for pool type dispatch.
    fn pool_type(e: Env) -> Symbol {
        Symbol::new(&e, "concentrated")
    }

    // One-time pool setup. Called by router during pool creation.
    // Sets tokens, fee, tick spacing, access roles, default protocol fee (50%),
    // and initial price at tick 0 (1:1). Price is auto-set on first deposit from token ratio.
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
        if token0 >= token1 {
            panic_with_error!(&e, Error::TokensNotSorted);
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
        set_reserve0(&e, &0);
        set_reserve1(&e, &0);

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

    // Pool fee in basis points (e.g. 30 = 0.3%).
    fn get_fee_fraction(e: Env) -> u32 {
        get_fee(&e)
    }

    // Protocol's share of collected fees, in parts per FEE_DENOMINATOR (1_000_000).
    fn get_protocol_fee_fraction(e: Env) -> u32 {
        get_protocol_fee_fraction(&e)
    }

    // Returns pool contract address — concentrated pools don't mint LP tokens,
    // liquidity is tracked per-position internally.
    fn share_id(e: Env) -> Address {
        e.current_contract_address()
    }

    // Total raw liquidity across all positions (sum of all deposit amounts).
    // Used by rewards system as equivalent of "total shares" in standard pools.
    fn get_total_shares(e: Env) -> u128 {
        get_total_raw_liquidity(&e)
    }

    // User's total raw liquidity across all their positions.
    fn get_user_shares(e: Env, user: Address) -> u128 {
        get_user_raw_liquidity(&e, &user)
    }

    // Tracked LP reserves (excludes protocol fees). Updated by deposit/withdraw/swap/collect.
    fn get_reserves(e: Env) -> Vec<u128> {
        Vec::from_array(&e, [get_reserve0(&e), get_reserve1(&e)])
    }

    // Returns [token0, token1] sorted addresses.
    fn get_tokens(e: Env) -> Vec<Address> {
        Vec::from_array(&e, [get_token0(&e), get_token1(&e)])
    }

    // Router-compatible deposit: opens a full-range position [MIN_TICK, MAX_TICK].
    // Delegates to deposit_position which computes maximum liquidity from desired_amounts
    // at current price, transfers required tokens, returns (actual_amounts, minted_liquidity).
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

        let (actual_amounts, liquidity) = match Self::deposit_position(
            e.clone(),
            user.clone(),
            tick_lower,
            tick_upper,
            desired_amounts,
        ) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        if liquidity < min_shares {
            panic_with_error!(&e, Error::InvalidAmount);
        }

        (actual_amounts, liquidity)
    }

    // Estimates liquidity for a full-range deposit without executing it.
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

    // Exact-input swap via token indexes (0 or 1). Swaps in_amount of token[in_idx]
    // for at least out_min of token[out_idx]. No price limit — swaps until input
    // is consumed or liquidity runs out. Returns amount received.
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

    // Simulates exact-input swap without executing. Returns expected output amount.
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

    // Exact-output swap: receive exactly out_amount of token[out_idx], paying at most
    // in_max of token[in_idx]. Returns actual amount spent.
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

    // Simulates exact-output swap without executing. Returns expected input amount.
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

    // Router-compatible withdraw: removes share_amount liquidity from user's full-range
    // position, then collects all owed tokens (withdrawn + accrued fees).
    // Returns [amount0, amount1] received. Reverts if below min_amounts.
    fn withdraw(e: Env, user: Address, share_amount: u128, min_amounts: Vec<u128>) -> Vec<u128> {
        if min_amounts.len() != 2 {
            panic_with_error!(&e, Error::InvalidAmount);
        }

        let (tick_lower, tick_upper) = match Self::full_range_ticks(&e) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let (_burn_amount0, _burn_amount1) = match Self::withdraw_position(
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
            tick_lower,
            tick_upper,
            u128::MAX,
            u128::MAX,
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

    // Returns pool metadata: pool_type, fee, tick_spacing.
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

    // Shares excluded from rewards distribution (e.g. users who opted out).
    fn get_total_excluded_shares(e: Env) -> u128 {
        Self::rewards_manager(&e)
            .storage()
            .get_total_excluded_shares()
    }
}
