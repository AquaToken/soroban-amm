use super::*;

// External rewards gauges — third-party reward token distribution.
// Each gauge is a separate contract that distributes its own reward token
// proportionally to users' weighted liquidity in this pool.
#[contractimpl]
impl RewardsGaugeInterfaceTrait for ConcentratedLiquidityPool {
    // Register a new rewards gauge contract. Operations admin, owner, or router.
    fn gauge_add(e: Env, admin: Address, gauge_address: Address) {
        admin.require_auth();
        if admin != get_router(&e) {
            require_operations_admin_or_owner(&e, &admin);
        }
        rewards_gauge::operations::add(&e, gauge_address);
    }

    // Remove a gauge. Admin only.
    fn gauge_remove(e: Env, admin: Address, reward_token: Address) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        rewards_gauge::operations::remove(&e, reward_token);
    }

    // Schedule reward distribution on a gauge. Router + distributor auth required.
    fn gauge_schedule_reward(
        e: Env,
        router: Address,
        distributor: Address,
        gauge: Address,
        start_at: Option<u64>,
        duration: u64,
        tps: u128,
    ) {
        router.require_auth();
        distributor.require_auth();
        if router != get_router(&e) {
            panic_with_error!(&e, Error::Unauthorized);
        }

        let rewards = Self::rewards_manager(&e);
        let total_weighted = get_total_weighted_liquidity(&e);
        let manager = rewards.manager();

        rewards_gauge::operations::schedule_rewards_config(
            &e,
            gauge,
            distributor,
            start_at,
            duration,
            tps,
            manager.get_working_supply(total_weighted),
        );
    }

    // Kill switch for gauge reward claims.
    fn kill_gauges_claim(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        rewards_gauge::operations::kill_claim(&e);
    }

    fn unkill_gauges_claim(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_admin_or_owner(&e, &admin);
        rewards_gauge::operations::unkill_claim(&e);
    }

    // Returns map of gauge_address → reward_token_address.
    fn get_gauges(e: Env) -> Map<Address, Address> {
        rewards_gauge::operations::list(&e)
    }

    // Claim rewards from all registered gauges. Returns map of reward_token → amount.
    fn gauges_claim(e: Env, user: Address) -> Map<Address, u128> {
        user.require_auth();
        Self::recompute_user_weighted_liquidity(&e, &user);

        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);
        let manager = Self::rewards_manager(&e).manager();

        rewards_gauge::operations::claim(
            &e,
            &user,
            manager.get_working_balance(&user, user_weighted),
            manager.get_working_supply(total_weighted),
        )
    }

    // Query pending gauge rewards for a user without claiming.
    fn gauges_get_reward_info(e: Env, user: Address) -> Map<Address, Map<Symbol, i128>> {
        Self::recompute_user_weighted_liquidity(&e, &user);

        let total_weighted = get_total_weighted_liquidity(&e);
        let user_weighted = get_user_weighted_liquidity(&e, &user);
        let manager = Self::rewards_manager(&e).manager();

        rewards_gauge::operations::get_rewards_info(
            &e,
            &user,
            manager.get_working_balance(&user, user_weighted),
            manager.get_working_supply(total_weighted),
        )
    }
}
