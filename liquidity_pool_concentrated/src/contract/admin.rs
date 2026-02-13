use super::*;

#[contractimpl]
impl AdminInterfaceTrait for ConcentratedLiquidityPool {
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

    fn set_claim_killed(e: Env, admin: Address, value: bool) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_claim_killed(&e, &value);
    }

    fn get_claim_killed(e: Env) -> bool {
        get_claim_killed(&e)
    }

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

    fn kill_deposit(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_is_killed_deposit(&e, &true);
    }

    fn kill_swap(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_is_killed_swap(&e, &true);
    }

    fn kill_claim(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);
        set_claim_killed(&e, &true);
    }

    fn unkill_deposit(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_admin_or_owner(&e, &admin);
        set_is_killed_deposit(&e, &false);
    }

    fn unkill_swap(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_admin_or_owner(&e, &admin);
        set_is_killed_swap(&e, &false);
    }

    fn unkill_claim(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_admin_or_owner(&e, &admin);
        set_claim_killed(&e, &false);
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

    fn set_protocol_fee_fraction(e: Env, admin: Address, new_fraction: u32) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);
        if new_fraction as u128 > FEE_DENOMINATOR {
            panic_with_error!(&e, Error::InvalidFee);
        }
        set_protocol_fee_fraction(&e, &new_fraction);
    }

    fn get_protocol_fees(e: Env) -> Vec<u128> {
        let fees = get_protocol_fees(&e);
        Vec::from_array(&e, [fees.token0, fees.token1])
    }

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
        if amount0 > 0 {
            SorobanTokenClient::new(&e, &get_token0(&e)).transfer(
                &contract,
                &destination,
                &(amount0 as i128),
            );
            fees.token0 = 0;
        }
        if amount1 > 0 {
            SorobanTokenClient::new(&e, &get_token1(&e)).transfer(
                &contract,
                &destination,
                &(amount1 as i128),
            );
            fees.token1 = 0;
        }
        set_protocol_fees(&e, &fees);
        update_plane(&e);
        Vec::from_array(&e, [amount0, amount1])
    }
}
