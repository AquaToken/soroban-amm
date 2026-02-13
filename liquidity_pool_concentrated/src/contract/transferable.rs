use super::*;

#[contractimpl]
impl TransferableContract for ConcentratedLiquidityPool {
    fn commit_transfer_ownership(e: Env, admin: Address, role_name: Symbol, new_address: Address) {
        Self::require_admin(&e, &admin);
        let role = Role::from_symbol(&e, role_name);
        let access_control = AccessControl::new(&e);
        access_control.commit_transfer_ownership(&role, &new_address);
        AccessControlEvents::new(&e).commit_transfer_ownership(role, new_address);
    }

    fn apply_transfer_ownership(e: Env, admin: Address, role_name: Symbol) {
        Self::require_admin(&e, &admin);
        let role = Role::from_symbol(&e, role_name);
        let access_control = AccessControl::new(&e);
        let new_address = access_control.apply_transfer_ownership(&role);
        AccessControlEvents::new(&e).apply_transfer_ownership(role, new_address);
    }

    fn revert_transfer_ownership(e: Env, admin: Address, role_name: Symbol) {
        Self::require_admin(&e, &admin);
        let role = Role::from_symbol(&e, role_name);
        let access_control = AccessControl::new(&e);
        access_control.revert_transfer_ownership(&role);
        AccessControlEvents::new(&e).revert_transfer_ownership(role);
    }

    fn get_future_address(e: Env, role_name: Symbol) -> Address {
        let role = Role::from_symbol(&e, role_name);
        AccessControl::new(&e).get_future_address(&role)
    }
}
