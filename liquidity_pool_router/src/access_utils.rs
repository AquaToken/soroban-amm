use access_control::access::{AccessControl, AccessControlTrait, Role};
use access_control::errors::AccessControlError;
use soroban_sdk::{panic_with_error, Address, Env};

pub(crate) fn require_admin_or_rewards_admin(e: &Env, user: Address) {
    // both admin and operator are authorized
    let access_control = AccessControl::new(&e);
    if access_control.address_has_role(Role::Admin, &user)
        || access_control.address_has_role(Role::RewardsAdmin, &user)
    {
        return;
    }

    panic_with_error!(e, AccessControlError::Unauthorized);
}
