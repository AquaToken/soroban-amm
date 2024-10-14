use crate::errors::AccessControlError;
use soroban_sdk::{contracttype, panic_with_error, Address, Env};
use utils::bump::bump_instance;
use utils::storage_errors::StorageError;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Admin,
    FutureAdmin,
    Operator,
}

#[derive(Clone)]
pub struct AccessControl(Env);

impl AccessControl {
    pub fn new(env: &Env) -> AccessControl {
        AccessControl(env.clone())
    }
}

pub trait AccessControlTrait {
    fn has_admin(&self) -> bool;
    fn get_admin(&self) -> Option<Address>;
    fn set_admin(&self, admin: &Address);
    fn check_admin(&self, user: &Address);
    fn require_admin(&self);
    fn get_future_admin(&self) -> Option<Address>;
    fn set_future_admin(&self, admin: &Address);
    fn perform_admin_check(&self) -> Result<Address, AccessControlError>;
}

pub trait OperatorAccessTrait {
    fn has_operator(&self) -> bool;
    fn get_operator(&self) -> Option<Address>;
    fn set_operator(&self, admin: &Address);
    fn check_operator(&self, user: &Address);
}

impl AccessControlTrait for AccessControl {
    fn has_admin(&self) -> bool {
        bump_instance(&self.0);
        self.0.storage().instance().has(&DataKey::Admin)
    }

    fn get_admin(&self) -> Option<Address> {
        bump_instance(&self.0);
        self.0.storage().instance().get(&DataKey::Admin)
    }

    fn set_admin(&self, admin: &Address) {
        bump_instance(&self.0);
        self.0.storage().instance().set(&DataKey::Admin, admin);
    }

    fn check_admin(&self, user: &Address) {
        let admin = match self.perform_admin_check() {
            Ok(v) => v,
            Err(err) => panic_with_error!(self.0, err),
        };
        if admin != user.clone() {
            panic_with_error!(&self.0, AccessControlError::Unauthorized);
        }
    }

    fn require_admin(&self) {
        let admin = match self.perform_admin_check() {
            Ok(v) => v,
            Err(err) => panic_with_error!(self.0, err),
        };
        admin.require_auth();
    }

    fn get_future_admin(&self) -> Option<Address> {
        bump_instance(&self.0);
        match self.0.storage().instance().get(&DataKey::FutureAdmin) {
            Some(v) => v,
            None => panic_with_error!(&self.0, StorageError::ValueNotInitialized),
        }
    }

    fn set_future_admin(&self, admin: &Address) {
        bump_instance(&self.0);
        self.0
            .storage()
            .instance()
            .set(&DataKey::FutureAdmin, admin)
    }

    fn perform_admin_check(&self) -> Result<Address, AccessControlError> {
        if !self.has_admin() {
            panic_with_error!(&self.0, AccessControlError::RoleNotFound);
        }
        self.get_admin().ok_or(AccessControlError::RoleNotFound)
    }
}

impl OperatorAccessTrait for AccessControl {
    fn has_operator(&self) -> bool {
        bump_instance(&self.0);
        self.0.storage().instance().has(&DataKey::Operator)
    }

    fn get_operator(&self) -> Option<Address> {
        bump_instance(&self.0);
        self.0.storage().instance().get(&DataKey::Operator)
    }

    fn set_operator(&self, operator: &Address) {
        bump_instance(&self.0);
        self.0
            .storage()
            .instance()
            .set(&DataKey::Operator, operator)
    }

    fn check_operator(&self, user: &Address) {
        let operator = match self.get_operator() {
            Some(address) => address,
            None => panic_with_error!(self.0, AccessControlError::RoleNotFound),
        };
        if operator != user.clone() {
            panic_with_error!(&self.0, AccessControlError::Unauthorized);
        }
    }
}
