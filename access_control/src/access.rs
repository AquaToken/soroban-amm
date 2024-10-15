use crate::errors::AccessControlError;
use soroban_sdk::{contracttype, panic_with_error, Address, Env, Symbol};
use utils::bump::bump_instance;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Admin,           // owner - upgrade, set privileged roles
    FutureAdmin,     // pending owner
    Operator,        // rewards admin - configure rewards. legacy name cannot be changed
    OperationsAdmin, // operations admin - add/remove pools, ramp A, set fees, etc
    PauseAdmin,      // pause admin - pause/unpause pools
    EmPauseAdmin,    // emergency pause admin - pause pools in emergency
}

pub enum Role {
    Admin,
    FutureAdmin,
    RewardsAdmin,
    OperationsAdmin,
    PauseAdmin,
    EmergencyPauseAdmin,
}

impl Role {
    pub fn as_symbol(&self, e: &Env) -> Symbol {
        match self {
            Role::Admin => Symbol::new(&e, "Admin"),
            Role::FutureAdmin => Symbol::new(&e, "FutureAdmin"),
            Role::RewardsAdmin => Symbol::new(&e, "RewardsAdmin"),
            Role::OperationsAdmin => Symbol::new(&e, "OperationsAdmin"),
            Role::PauseAdmin => Symbol::new(&e, "PauseAdmin"),
            Role::EmergencyPauseAdmin => Symbol::new(&e, "EmergencyPauseAdmin"),
        }
    }
}

#[derive(Clone)]
pub struct AccessControl(Env);

impl AccessControl {
    pub fn new(env: &Env) -> AccessControl {
        AccessControl(env.clone())
    }

    fn get_key(&self, role: Role) -> DataKey {
        match role {
            Role::Admin => DataKey::Admin,
            Role::FutureAdmin => DataKey::FutureAdmin,
            Role::RewardsAdmin => DataKey::Operator,
            Role::OperationsAdmin => DataKey::OperationsAdmin,
            Role::PauseAdmin => DataKey::PauseAdmin,
            Role::EmergencyPauseAdmin => DataKey::EmPauseAdmin,
        }
    }
}

pub trait AccessControlTrait {
    fn get_role_safe(&self, role: Role) -> Option<Address>;
    fn get_role(&self, role: Role) -> Address;
    fn set_role_address(&self, role: Role, address: &Address);
    fn address_has_role(&self, role: Role, address: &Address) -> bool;
    fn assert_address_has_role(&self, address: &Address, role: Role);
}

impl AccessControlTrait for AccessControl {
    fn get_role_safe(&self, role: Role) -> Option<Address> {
        let key = self.get_key(role);
        bump_instance(&self.0);
        self.0.storage().instance().get(&key)
    }

    fn get_role(&self, role: Role) -> Address {
        match self.get_role_safe(role) {
            Some(address) => address,
            None => panic_with_error!(&self.0, AccessControlError::RoleNotFound),
        }
    }

    fn set_role_address(&self, role: Role, address: &Address) {
        let key = self.get_key(role);
        bump_instance(&self.0);
        self.0.storage().instance().set(&key, address);
    }

    fn address_has_role(&self, role: Role, address: &Address) -> bool {
        match self.get_role_safe(role) {
            Some(role_address) => address == &role_address,
            None => false,
        }
    }

    fn assert_address_has_role(&self, address: &Address, role: Role) {
        if !self.address_has_role(role, address) {
            panic_with_error!(&self.0, AccessControlError::Unauthorized);
        }
    }
}
