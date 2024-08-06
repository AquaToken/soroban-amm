use crate::storage::{get_operator, has_operator};
use access_control::access::{AccessControl, AccessControlTrait};
use access_control::errors::AccessControlError;
use soroban_sdk::{panic_with_error, Address, Env};

pub(crate) fn require_admin_or_operator(e: &Env, user: Address) {
    // both admin and operator are authorized
    let access_control = AccessControl::new(&e);
    let admin = match access_control.perform_admin_check() {
        Ok(v) => v,
        Err(err) => panic_with_error!(&e, err),
    };

    if user != admin {
        // the user is not an admin, so let's check if it's operator
        if !has_operator(&e) {
            panic_with_error!(&e, AccessControlError::Unauthorized);
        }
        if user != get_operator(&e) {
            panic_with_error!(&e, AccessControlError::Unauthorized);
        }
    }
}
