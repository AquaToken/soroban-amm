use soroban_sdk::{contracterror, contracttype, panic_with_error, Address, Env};
use utils::bump::bump_instance;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AccessControlError {
    AdminNotFound = 101,
    UserNotAdmin = 102,
    AdminAlreadySet = 103,
}

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
        let admin = self.perform_admin_check().expect("Cant check admin");
        if admin != user.clone() {
            panic_with_error!(&self.0, AccessControlError::UserNotAdmin);
        }
    }

    fn require_admin(&self) {
        let admin = self.perform_admin_check().expect("Cant find admin");
        admin.require_auth();
    }

    fn get_future_admin(&self) -> Option<Address> {
        bump_instance(&self.0);
        self.0
            .storage()
            .instance()
            .get(&DataKey::FutureAdmin)
            .expect("Cant find future admin")
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
