use super::*;

#[contractimpl]
impl UpgradeableContract for ConcentratedLiquidityPool {
    fn version() -> u32 {
        180
    }

    fn contract_name(e: Env) -> Symbol {
        Symbol::new(&e, "ConcentratedLiquidityPool")
    }

    fn commit_upgrade(
        e: Env,
        admin: Address,
        new_wasm_hash: BytesN<32>,
        token_new_wasm_hash: BytesN<32>,
        gauges_new_wasm_hash: BytesN<32>,
    ) {
        Self::require_admin(&e, &admin);
        commit_upgrade(&e, &new_wasm_hash);
        set_token_future_wasm(&e, &token_new_wasm_hash);
        set_gauge_future_wasm(&e, &gauges_new_wasm_hash);
        UpgradeEvents::new(&e).commit_upgrade(Vec::from_array(
            &e,
            [new_wasm_hash, token_new_wasm_hash, gauges_new_wasm_hash],
        ));
    }

    fn apply_upgrade(e: Env, admin: Address) -> (BytesN<32>, BytesN<32>) {
        Self::require_admin(&e, &admin);
        let new_wasm_hash = apply_upgrade(&e);
        let token_new_wasm_hash = get_token_future_wasm(&e);
        rewards_gauge::operations::upgrade(&e, &get_gauge_future_wasm(&e));
        UpgradeEvents::new(&e).apply_upgrade(Vec::from_array(
            &e,
            [new_wasm_hash.clone(), token_new_wasm_hash.clone()],
        ));
        (new_wasm_hash, token_new_wasm_hash)
    }

    fn revert_upgrade(e: Env, admin: Address) {
        Self::require_admin(&e, &admin);
        revert_upgrade(&e);
        UpgradeEvents::new(&e).revert_upgrade();
    }

    fn set_emergency_mode(e: Env, emergency_admin: Address, value: bool) {
        emergency_admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&emergency_admin, &Role::EmergencyAdmin);
        set_emergency_mode(&e, &value);
        AccessControlEvents::new(&e).set_emergency_mode(value);
    }

    fn get_emergency_mode(e: Env) -> bool {
        get_emergency_mode(&e)
    }
}
