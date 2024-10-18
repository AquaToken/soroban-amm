use crate::constants::ADMIN_ACTIONS_DELAY;
use crate::errors::AccessControlError;
use crate::storage::{get_transfer_ownership_deadline, put_transfer_ownership_deadline, DataKey};
use soroban_sdk::{panic_with_error, Address, Env, Symbol, Vec};
use utils::bump::bump_instance;
use utils::storage_errors::StorageError;

pub enum Role {
    Admin,
    FutureAdmin,
    RewardsAdmin,
    OperationsAdmin,
    PauseAdmin,
    EmergencyPauseAdmin,
}

pub trait SymbolRepresentation {
    fn as_symbol(&self, e: &Env) -> Symbol;
}

impl SymbolRepresentation for Role {
    fn as_symbol(&self, e: &Env) -> Symbol {
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
            Role::EmergencyPauseAdmin => DataKey::EmPauseAdmins,
        }
    }
}

fn role_has_many_users(role: &Role) -> bool {
    match role {
        Role::Admin => false,
        Role::FutureAdmin => false,
        Role::RewardsAdmin => false,
        Role::OperationsAdmin => false,
        Role::PauseAdmin => false,
        Role::EmergencyPauseAdmin => true,
    }
}

pub trait AccessControlTrait {
    // single address
    fn get_role_safe(&self, role: Role) -> Option<Address>;
    fn get_role(&self, role: Role) -> Address;
    fn set_role_address(&self, role: Role, address: &Address);

    // multiple addresses
    fn get_role_addresses(&self, role: Role) -> Vec<Address>;
    fn set_role_addresses(&self, role: Role, addresses: &Vec<Address>);

    // check role
    fn address_has_role(&self, address: &Address, role: Role) -> bool;
    fn assert_address_has_role(&self, address: &Address, role: Role);
}

pub trait TransferOwnershipTrait {
    fn commit_transfer_ownership(&self, new_admin: Address);
    fn apply_transfer_ownership(&self) -> Address;
    fn revert_transfer_ownership(&self);
}

impl AccessControlTrait for AccessControl {
    fn get_role_safe(&self, role: Role) -> Option<Address> {
        if role_has_many_users(&role) {
            panic_with_error!(&self.0, AccessControlError::BadRoleUsage);
        }

        let key = self.get_key(role);
        bump_instance(&self.0);
        self.0.storage().instance().get(&key)
    }

    fn get_role(&self, role: Role) -> Address {
        match role {
            Role::Admin => {}
            _ => {
                // only admin is guaranteed, use `get_role_safe` for other roles
                panic_with_error!(&self.0, AccessControlError::BadRoleUsage);
            }
        }

        if role_has_many_users(&role) {
            panic_with_error!(&self.0, AccessControlError::BadRoleUsage);
        }

        match self.get_role_safe(role) {
            Some(address) => address,
            None => panic_with_error!(&self.0, AccessControlError::RoleNotFound),
        }
    }

    fn set_role_address(&self, role: Role, address: &Address) {
        if role_has_many_users(&role) {
            panic_with_error!(&self.0, AccessControlError::BadRoleUsage);
        }

        let key = self.get_key(role);
        bump_instance(&self.0);
        self.0.storage().instance().set(&key, address);
    }

    fn get_role_addresses(&self, role: Role) -> Vec<Address> {
        if !role_has_many_users(&role) {
            panic_with_error!(&self.0, AccessControlError::BadRoleUsage);
        }

        let key = self.get_key(role);
        bump_instance(&self.0);
        self.0
            .storage()
            .instance()
            .get(&key)
            .unwrap_or(Vec::new(&self.0))
    }

    fn set_role_addresses(&self, role: Role, addresses: &Vec<Address>) {
        if !role_has_many_users(&role) {
            panic_with_error!(&self.0, AccessControlError::BadRoleUsage);
        }

        let key = self.get_key(role);
        bump_instance(&self.0);
        self.0.storage().instance().set(&key, addresses);
    }

    fn address_has_role(&self, address: &Address, role: Role) -> bool {
        if role_has_many_users(&role) {
            self.get_role_addresses(role).contains(address)
        } else {
            match self.get_role_safe(role) {
                Some(role_address) => address == &role_address,
                None => false,
            }
        }
    }

    fn assert_address_has_role(&self, address: &Address, role: Role) {
        if !self.address_has_role(address, role) {
            panic_with_error!(&self.0, AccessControlError::Unauthorized);
        }
    }
}

impl TransferOwnershipTrait for AccessControl {
    fn commit_transfer_ownership(&self, new_admin: Address) {
        if get_transfer_ownership_deadline(&self.0) != 0 {
            panic_with_error!(&self.0, AccessControlError::AnotherActionActive);
        }

        let deadline = self.0.ledger().timestamp() + ADMIN_ACTIONS_DELAY;
        put_transfer_ownership_deadline(&self.0, &deadline);
        self.set_role_address(Role::FutureAdmin, &new_admin);
    }

    fn apply_transfer_ownership(&self) -> Address {
        if self.0.ledger().timestamp() < get_transfer_ownership_deadline(&self.0) {
            panic_with_error!(&self.0, AccessControlError::ActionNotReadyYet);
        }
        if get_transfer_ownership_deadline(&self.0) == 0 {
            panic_with_error!(&self.0, AccessControlError::NoActionActive);
        }

        put_transfer_ownership_deadline(&self.0, &0);
        let future_admin = match self.get_role_safe(Role::FutureAdmin) {
            Some(v) => v,
            None => panic_with_error!(&self.0, StorageError::ValueNotInitialized),
        };
        self.set_role_address(Role::Admin, &future_admin);
        future_admin
    }

    fn revert_transfer_ownership(&self) {
        put_transfer_ownership_deadline(&self.0, &0);
    }
}
