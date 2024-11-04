use crate::errors::AccessControlError;
use crate::management::{MultipleAddressesManagementTrait, SingleAddressManagementTrait};
use crate::role::Role;
use soroban_sdk::{panic_with_error, Address, Env};

#[derive(Clone)]
pub struct AccessControl(pub(crate) Env);

impl AccessControl {
    pub fn new(env: &Env) -> AccessControl {
        AccessControl(env.clone())
    }
}

pub trait AccessControlTrait {
    fn address_has_role(&self, address: &Address, role: &Role) -> bool;
    fn assert_address_has_role(&self, address: &Address, role: &Role);
}

impl AccessControlTrait for AccessControl {
    fn address_has_role(&self, address: &Address, role: &Role) -> bool {
        if role.has_many_users() {
            self.get_role_addresses(role).contains(address)
        } else {
            match self.get_role_safe(role) {
                Some(role_address) => address == &role_address,
                None => false,
            }
        }
    }

    fn assert_address_has_role(&self, address: &Address, role: &Role) {
        if !self.address_has_role(address, role) {
            panic_with_error!(&self.0, AccessControlError::Unauthorized);
        }
    }
}
