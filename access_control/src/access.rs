use crate::errors::AccessControlError;
use soroban_sdk::{contracttype, panic_with_error, Address, Env};
use utils::bump::bump_instance;
use utils::storage_errors::StorageError;

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Admin,
    FutureAdmin,
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
            panic_with_error!(&self.0, AccessControlError::UserNotAdmin);
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
            panic_with_error!(&self.0, AccessControlError::AdminNotFound);
        }
        self.get_admin().ok_or(AccessControlError::AdminNotFound)
    }
}
