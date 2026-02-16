use super::*;

// Admin operations — access-controlled pool management.
#[contractimpl]
impl AdminInterfaceTrait for ConcentratedLiquidityPool {
    // Configure rewards distance weighting. Positions closer to current price
    // get higher rewards multiplier. max_distance_ticks = range where multiplier
    // starts decaying; min_multiplier_bps = floor multiplier for far positions (0 = no rewards).
    fn set_distance_weighting(
        e: Env,
        admin: Address,
        max_distance_ticks: u32,
        min_multiplier_bps: u32,
    ) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);
        set_distance_weight_config(
            &e,
            &DistanceWeightConfig {
                max_distance_ticks,
                min_multiplier_bps,
            },
        );
    }

    fn get_distance_weighting(e: Env) -> DistanceWeightConfig {
        get_distance_weight_config(&e)
    }

    // Kill switch for reward claims. Requires pause or emergency pause admin.
    fn set_claim_killed(e: Env, admin: Address, value: bool) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_claim_killed(&e, &value);
    }

    fn get_claim_killed(e: Env) -> bool {
        get_claim_killed(&e)
    }

    // Assign privileged roles: rewards_admin, operations_admin, pause_admin,
    // emergency_pause_admins (multiple), system_fee_admin. Admin only.
    fn set_privileged_addrs(
        e: Env,
        admin: Address,
        rewards_admin: Address,
        operations_admin: Address,
        pause_admin: Address,
        emergency_pause_admins: Vec<Address>,
        system_fee_admin: Address,
    ) {
        Self::require_admin(&e, &admin);
        let access_control = AccessControl::new(&e);
        access_control.set_role_address(&Role::RewardsAdmin, &rewards_admin);
        access_control.set_role_address(&Role::OperationsAdmin, &operations_admin);
        access_control.set_role_address(&Role::PauseAdmin, &pause_admin);
        access_control.set_role_addresses(&Role::EmergencyPauseAdmin, &emergency_pause_admins);
        access_control.set_role_address(&Role::SystemFeeAdmin, &system_fee_admin);
        AccessControlEvents::new(&e).set_privileged_addrs(
            rewards_admin,
            operations_admin,
            pause_admin,
            emergency_pause_admins,
            system_fee_admin,
        );
    }

    // Returns map of role_name → [addresses] for all privileged roles.
    fn get_privileged_addrs(e: Env) -> Map<Symbol, Vec<Address>> {
        let access_control = AccessControl::new(&e);
        let mut result: Map<Symbol, Vec<Address>> = Map::new(&e);
        for role in [
            Role::Admin,
            Role::EmergencyAdmin,
            Role::RewardsAdmin,
            Role::OperationsAdmin,
            Role::PauseAdmin,
            Role::SystemFeeAdmin,
        ] {
            result.set(
                role.as_symbol(&e),
                match access_control.get_role_safe(&role) {
                    Some(v) => Vec::from_array(&e, [v]),
                    None => Vec::new(&e),
                },
            );
        }
        result.set(
            Role::EmergencyPauseAdmin.as_symbol(&e),
            access_control.get_role_addresses(&Role::EmergencyPauseAdmin),
        );
        result
    }

    // Kill switches: independently pause deposits, swaps, or fee claims.
    // kill_* requires pause or emergency pause admin; unkill_* requires pause admin only.
    fn kill_deposit(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_is_killed_deposit(&e, &true);
        PoolEvents::new(&e).kill_deposit();
    }

    fn kill_swap(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_is_killed_swap(&e, &true);
        PoolEvents::new(&e).kill_swap();
    }

    fn kill_claim(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_claim_killed(&e, &true);
        PoolEvents::new(&e).kill_claim();
    }

    fn unkill_deposit(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_admin_or_owner(&e, &admin);
        set_is_killed_deposit(&e, &false);
        PoolEvents::new(&e).unkill_deposit();
    }

    fn unkill_swap(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_admin_or_owner(&e, &admin);
        set_is_killed_swap(&e, &false);
        PoolEvents::new(&e).unkill_swap();
    }

    fn unkill_claim(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_admin_or_owner(&e, &admin);
        set_claim_killed(&e, &false);
        PoolEvents::new(&e).unkill_claim();
    }

    fn get_is_killed_deposit(e: Env) -> bool {
        get_is_killed_deposit(&e)
    }

    fn get_is_killed_swap(e: Env) -> bool {
        get_is_killed_swap(&e)
    }

    fn get_is_killed_claim(e: Env) -> bool {
        get_claim_killed(&e)
    }

    // Set protocol's share of swap fees, in parts per FEE_DENOMINATOR (1_000_000).
    // E.g. 5_000 = 0.5%. Operations admin or owner only.
    fn set_protocol_fee_fraction(e: Env, admin: Address, new_fraction: u32) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);
        if new_fraction as u128 > FEE_DENOMINATOR {
            panic_with_error!(&e, Error::InvalidFee);
        }
        set_protocol_fee_fraction(&e, &new_fraction);
        PoolEvents::new(&e).set_protocol_fee_fraction(new_fraction);
    }

    // Returns [token0_fees, token1_fees] accumulated for the protocol.
    fn get_protocol_fees(e: Env) -> Vec<u128> {
        let fees = get_protocol_fees(&e);
        Vec::from_array(&e, [fees.token0, fees.token1])
    }

    // Transfer accumulated protocol fees to destination. System fee admin or owner only.
    // Returns [amount0, amount1] transferred. Resets counters to zero.
    fn claim_protocol_fees(e: Env, admin: Address, destination: Address) -> Vec<u128> {
        admin.require_auth();
        require_system_fee_admin_or_owner(&e, &admin);

        let mut fees = get_protocol_fees(&e);
        let amount0 = fees.token0;
        let amount1 = fees.token1;
        if amount0 == 0 && amount1 == 0 {
            return Vec::from_array(&e, [0, 0]);
        }

        let contract = e.current_contract_address();
        let token0 = get_token0(&e);
        let token1 = get_token1(&e);
        if amount0 > 0 {
            SorobanTokenClient::new(&e, &token0).transfer(
                &contract,
                &destination,
                &(amount0 as i128),
            );
            fees.token0 = 0;
            PoolEvents::new(&e).claim_protocol_fee(
                token0,
                destination.clone(),
                amount0,
            );
        }
        if amount1 > 0 {
            SorobanTokenClient::new(&e, &token1).transfer(
                &contract,
                &destination,
                &(amount1 as i128),
            );
            fees.token1 = 0;
            PoolEvents::new(&e).claim_protocol_fee(token1, destination, amount1);
        }
        set_protocol_fees(&e, &fees);
        update_plane(&e);
        Vec::from_array(&e, [amount0, amount1])
    }
}
