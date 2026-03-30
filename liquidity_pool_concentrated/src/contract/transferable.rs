use super::*;

// Role ownership transfer with 3-day delay (same as upgrades).
// commit → wait → apply. Admin only.
#[contractimpl]
impl TransferableContract for ConcentratedLiquidityPool {
    // Stage ownership transfer for a role to a new address.
    fn commit_transfer_ownership(e: Env, admin: Address, role_name: Symbol, new_address: Address) {
        Self::require_admin(&e, &admin);
        let role = Role::from_symbol(&e, role_name);
        let access_control = AccessControl::new(&e);
        access_control.commit_transfer_ownership(&role, &new_address);
        AccessControlEvents::new(&e).commit_transfer_ownership(role, new_address);
    }

    // Finalize staged transfer after delay.
    fn apply_transfer_ownership(e: Env, admin: Address, role_name: Symbol) {
        Self::require_admin(&e, &admin);
        let role = Role::from_symbol(&e, role_name);
        let access_control = AccessControl::new(&e);
        let new_address = access_control.apply_transfer_ownership(&role);
        AccessControlEvents::new(&e).apply_transfer_ownership(role, new_address);
    }

    // Cancel staged transfer.
    fn revert_transfer_ownership(e: Env, admin: Address, role_name: Symbol) {
        Self::require_admin(&e, &admin);
        let role = Role::from_symbol(&e, role_name);
        let access_control = AccessControl::new(&e);
        access_control.revert_transfer_ownership(&role);
        AccessControlEvents::new(&e).revert_transfer_ownership(role);
    }

    // View the staged (pending) new address for a role.
    fn get_future_address(e: Env, role_name: Symbol) -> Address {
        let role = Role::from_symbol(&e, role_name);
        AccessControl::new(&e).get_future_address(&role)
    }
}
