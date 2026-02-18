use super::*;

// Native AQUA rewards — distributed proportionally to weighted liquidity.
// Weighted liquidity = raw_liquidity * distance_multiplier * boost_multiplier.
// Distance multiplier rewards positions closer to current price.
// Boost multiplier rewards users who lock AQUA in the locker.
#[contractimpl]
impl RewardsTrait for ConcentratedLiquidityPool {
    // Set the reward token address (typically AQUA). One-time setup, panics if already set.
    fn initialize_rewards_config(e: Env, reward_token: Address) {
        let rewards = Self::rewards_manager(&e);
        if rewards.storage().has_reward_token() {
            panic_with_error!(&e, Error::RewardsAlreadyInitialized);
        }
        rewards.storage().put_reward_token(reward_token);
    }

    // Set boost config: token to lock for boosted rewards + price feed for valuation.
    fn initialize_boost_config(e: Env, reward_boost_token: Address, reward_boost_feed: Address) {
        let rewards = Self::rewards_manager(&e);
        if rewards.storage().has_reward_boost_token() {
            panic_with_error!(&e, Error::RewardsAlreadyInitialized);
        }
        rewards.storage().put_reward_boost_token(reward_boost_token);
        rewards.storage().put_reward_boost_feed(reward_boost_feed);
    }

    // Update boost config after initialization. Admin only.
    fn set_reward_boost_config(
        e: Env,
        admin: Address,
        reward_boost_token: Address,
        reward_boost_feed: Address,
    ) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);

        let storage = Self::rewards_manager(&e).storage();
        storage.put_reward_boost_token(reward_boost_token);
        storage.put_reward_boost_feed(reward_boost_feed);
    }

    // Configure reward emission rate: tps = tokens per second, expired_at = end timestamp.
    // Rewards admin, owner, or router.
    fn set_rewards_config(e: Env, admin: Address, expired_at: u64, tps: u128) {
        admin.require_auth();
        if admin != get_router(&e) {
            require_rewards_admin_or_owner(&e, &admin);
        }
        let mut manager = Self::rewards_manager(&e).manager();
        manager.set_reward_config(get_total_weighted_liquidity(&e), expired_at, tps);
    }

    // Reward tokens held by pool that exceed what's owed to LPs. Can be reclaimed.
    fn get_unused_reward(e: Env) -> u128 {
        let rewards = Self::rewards_manager(&e);
        let mut manager = rewards.manager();
        let total_weighted = get_total_weighted_liquidity(&e);

        let mut reward_balance_to_keep = manager
            .get_total_configured_reward(total_weighted)
            .saturating_sub(manager.get_total_claimed_reward(total_weighted));

        let reward_token = rewards.storage().get_reward_token();
        let reward_balance = SorobanTokenClient::new(&e, &reward_token)
            .balance(&e.current_contract_address()) as u128;

        if let Some(idx) = Self::get_tokens(e.clone()).first_index_of(reward_token) {
            reward_balance_to_keep = reward_balance_to_keep
                .saturating_add(Self::get_reserves(e.clone()).get(idx).unwrap());
        }

        reward_balance.saturating_sub(reward_balance_to_keep)
    }

    // Transfer unused reward tokens back to router. Rewards admin or owner.
    fn return_unused_reward(e: Env, admin: Address) -> u128 {
        admin.require_auth();
        require_rewards_admin_or_owner(&e, &admin);

        let unused_reward = Self::get_unused_reward(e.clone());
        if unused_reward == 0 {
            return 0;
        }

        let reward_token = Self::rewards_manager(&e).storage().get_reward_token();
        SorobanTokenClient::new(&e, &reward_token).transfer(
            &e.current_contract_address(),
            &get_router(&e),
            &(unused_reward as i128),
        );
        unused_reward
    }

    // Full rewards state for a user — aligned with standard/stableswap pools.
    fn get_rewards_info(e: Env, user: Address) -> Map<Symbol, i128> {
        let rewards = Self::rewards_manager(&e);
        let mut manager = rewards.manager();
        let storage = rewards.storage();
        let config = storage.get_pool_reward_config();

        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);

        // pre-fill result dict with stored values
        // or values won't be affected by checkpoint in any way
        let mut result = Map::from_array(
            &e,
            [
                (symbol_short!("tps"), config.tps as i128),
                (symbol_short!("exp_at"), config.expired_at as i128),
                (
                    symbol_short!("state"),
                    manager.get_user_rewards_state(&user) as i128,
                ),
                (symbol_short!("supply"), total_weighted as i128),
                (
                    Symbol::new(&e, "working_balance"),
                    manager.get_working_balance(&user, user_weighted) as i128,
                ),
                (
                    Symbol::new(&e, "working_supply"),
                    manager.get_working_supply(total_weighted) as i128,
                ),
                (
                    Symbol::new(&e, "boost_balance"),
                    manager.get_user_boost_balance(&user) as i128,
                ),
                (
                    Symbol::new(&e, "boost_supply"),
                    manager.get_total_locked() as i128,
                ),
            ],
        );

        // gauge checkpoint before pool checkpoint
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            manager.get_working_balance(&user, user_weighted),
            manager.get_working_supply(total_weighted),
        );

        // display actual values
        let user_data = manager.checkpoint_user(&user, total_weighted, user_weighted);
        let pool_data = storage.get_pool_reward_data();

        result.set(symbol_short!("acc"), pool_data.accumulated as i128);
        result.set(symbol_short!("last_time"), pool_data.last_time as i128);
        result.set(
            symbol_short!("pool_acc"),
            user_data.pool_accumulated as i128,
        );
        result.set(symbol_short!("block"), pool_data.block as i128);
        result.set(symbol_short!("usr_block"), user_data.last_block as i128);
        result.set(symbol_short!("to_claim"), user_data.to_claim as i128);

        // provide updated working balance information. if working_balance_new is bigger
        // than working_balance, it means that user has locked some tokens
        // and needs to checkpoint itself for more rewards
        result.set(
            Symbol::new(&e, "new_working_balance"),
            manager.get_working_balance(&user, user_weighted) as i128,
        );
        result.set(
            Symbol::new(&e, "new_working_supply"),
            manager.get_working_supply(total_weighted) as i128,
        );
        result
    }

    // Pending reward amount for a user (recomputes weighted liquidity first).
    fn get_user_reward(e: Env, user: Address) -> u128 {
        Self::recompute_user_weighted_liquidity(&e, &user);

        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);
        let mut manager = Self::rewards_manager(&e).manager();

        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            manager.get_working_balance(&user, user_weighted),
            manager.get_working_supply(total_weighted),
        );

        manager.get_amount_to_claim(&user, total_weighted, user_weighted)
    }

    // Preview working_balance and working_supply after a hypothetical position change.
    // new_liquidity = absolute liquidity value for position (tick_lower, tick_upper).
    // If tick range matches an existing position — simulates modification.
    // If new range — simulates deposit. new_liquidity=0 simulates full withdrawal.
    fn estimate_working_balance(
        e: Env,
        user: Address,
        tick_lower: i32,
        tick_upper: i32,
        new_liquidity: u128,
    ) -> (u128, u128) {
        let new_user_weighted =
            Self::compute_user_weighted_liquidity(&e, &user, tick_lower, tick_upper, new_liquidity);

        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);

        let new_total_weighted = if new_user_weighted >= user_weighted {
            total_weighted.saturating_add(new_user_weighted - user_weighted)
        } else {
            total_weighted.saturating_sub(user_weighted - new_user_weighted)
        };

        let manager = Self::rewards_manager(&e).manager();
        let prev_working_balance = manager.get_working_balance(&user, user_weighted);
        let prev_working_supply = manager.get_working_supply(total_weighted);
        let new_working_balance =
            manager.calculate_effective_balance(&user, new_user_weighted, new_total_weighted);
        let new_working_supply = prev_working_supply + new_working_balance - prev_working_balance;
        (new_working_balance, new_working_supply)
    }

    // Total rewards emitted since pool creation.
    fn get_total_accumulated_reward(e: Env) -> u128 {
        Self::rewards_manager(&e)
            .manager()
            .get_total_accumulated_reward(get_total_weighted_liquidity(&e))
    }

    // Total rewards that will be emitted by current config expiry.
    fn get_total_configured_reward(e: Env) -> u128 {
        Self::rewards_manager(&e)
            .manager()
            .get_total_configured_reward(get_total_weighted_liquidity(&e))
    }

    // Admin correction for accumulated rewards counter. Rewards admin or owner.
    fn adjust_total_accumulated_reward(e: Env, admin: Address, diff: i128) {
        admin.require_auth();
        require_rewards_admin_or_owner(&e, &admin);
        Self::rewards_manager(&e)
            .manager()
            .adjust_total_accumulated_reward(get_total_weighted_liquidity(&e), diff);
    }

    // Total rewards already claimed by all users.
    fn get_total_claimed_reward(e: Env) -> u128 {
        Self::rewards_manager(&e)
            .manager()
            .get_total_claimed_reward(get_total_weighted_liquidity(&e))
    }

    // Claim pending AQUA rewards. Recomputes weighted liquidity, checkpoints gauges,
    // transfers reward tokens to user. Validates that claiming doesn't drain pool reserves
    // (relevant when reward_token == one of the pool tokens).
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

        // Post-claim reserve validation: ensure the reward transfer did not drain
        // below reserves + protocol fees. Stored reserves are independent of balance
        // (updated only by deposit/withdraw/swap/collect), so this check is meaningful.
        let reward_token = rewards.storage().get_reward_token();
        let token0 = get_token0(&e);
        let token1 = get_token1(&e);
        let contract = e.current_contract_address();
        let protocol_fees = get_protocol_fees(&e);

        if reward_token == token0 {
            let balance = SorobanTokenClient::new(&e, &token0).balance(&contract) as u128;
            let reserve = get_reserve0(&e);
            if reserve + protocol_fees.token0 > balance {
                panic_with_error!(&e, Error::InsufficientToken0);
            }
        } else if reward_token == token1 {
            let balance = SorobanTokenClient::new(&e, &token1).balance(&contract) as u128;
            let reserve = get_reserve1(&e);
            if reserve + protocol_fees.token1 > balance {
                panic_with_error!(&e, Error::InsufficientToken1);
            }
        }

        RewardEvents::new(&e).claim(user.clone(), reward_token, reward);

        let manager_after = rewards.manager();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            manager_after.get_working_balance(&user, user_weighted),
            manager_after.get_working_supply(total_weighted),
        );

        reward
    }

    // Whether user has opted into rewards (true = active, false = excluded).
    fn get_rewards_state(e: Env, user: Address) -> bool {
        Self::rewards_manager(&e)
            .manager()
            .get_user_rewards_state(&user)
    }

    // Opt in/out of rewards. Checkpoints gauges and rewards before changing state.
    fn set_rewards_state(e: Env, user: Address, state: bool) {
        user.require_auth();

        Self::recompute_user_weighted_liquidity(&e, &user);

        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);
        let mut manager = Self::rewards_manager(&e).manager();

        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            manager.get_working_balance(&user, user_weighted),
            manager.get_working_supply(total_weighted),
        );
        manager.set_user_rewards_state(&user, user_weighted, state);
        manager.checkpoint_user(&user, total_weighted, user_weighted);
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            manager.get_working_balance(&user, user_weighted),
            manager.get_working_supply(total_weighted),
        );

        RewardEvents::new(&e).set_rewards_state(user, state);
    }

    // Admin override for user's rewards opt-in state. Operations admin or owner.
    fn admin_set_rewards_state(e: Env, admin: Address, user: Address, state: bool) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);

        Self::recompute_user_weighted_liquidity(&e, &user);

        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);
        let mut manager = Self::rewards_manager(&e).manager();

        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            manager.get_working_balance(&user, user_weighted),
            manager.get_working_supply(total_weighted),
        );
        manager.set_user_rewards_state(&user, user_weighted, state);
        manager.checkpoint_user(&user, total_weighted, user_weighted);
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            manager.get_working_balance(&user, user_weighted),
            manager.get_working_supply(total_weighted),
        );
    }
}
