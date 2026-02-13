use super::*;

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
