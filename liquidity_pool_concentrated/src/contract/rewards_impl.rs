use super::*;

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

    fn set_rewards_config(e: Env, admin: Address, expired_at: u64, tps: u128) {
        admin.require_auth();
        if admin != get_router(&e) {
            require_rewards_admin_or_owner(&e, &admin);
        }
        let mut manager = Self::rewards_manager(&e).manager();
        manager.set_reward_config(get_total_weighted_liquidity(&e), expired_at, tps);
    }

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

    fn estimate_working_balance(e: Env, user: Address, new_user_shares: u128) -> (u128, u128) {
        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);

        let new_total_weighted = if new_user_shares >= user_weighted {
            total_weighted + (new_user_shares - user_weighted)
        } else {
            total_weighted - (user_weighted - new_user_shares)
        };

        let manager = Self::rewards_manager(&e).manager();
        let prev_working_balance = manager.get_working_balance(&user, user_weighted);
        let prev_working_supply = manager.get_working_supply(total_weighted);
        let new_working_balance =
            manager.calculate_effective_balance(&user, new_user_shares, new_total_weighted);
        let new_working_supply = prev_working_supply + new_working_balance - prev_working_balance;
        (new_working_balance, new_working_supply)
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

    fn adjust_total_accumulated_reward(e: Env, admin: Address, diff: i128) {
        admin.require_auth();
        require_rewards_admin_or_owner(&e, &admin);
        Self::rewards_manager(&e)
            .manager()
            .adjust_total_accumulated_reward(get_total_weighted_liquidity(&e), diff);
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

    fn get_rewards_state(e: Env, user: Address) -> bool {
        Self::rewards_manager(&e)
            .manager()
            .get_user_rewards_state(&user)
    }

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
