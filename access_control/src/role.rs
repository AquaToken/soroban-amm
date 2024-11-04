use crate::errors::AccessControlError;
use soroban_sdk::{panic_with_error, Env, Symbol};

#[derive(Clone)]
pub enum Role {
    Admin,
    EmergencyAdmin,
    RewardsAdmin,
    OperationsAdmin,
    PauseAdmin,
    EmergencyPauseAdmin,
}

impl Role {
    pub(crate) fn has_many_users(&self) -> bool {
        match self {
            Role::Admin => false,
            Role::EmergencyAdmin => false,
            Role::RewardsAdmin => false,
            Role::OperationsAdmin => false,
            Role::PauseAdmin => false,
            Role::EmergencyPauseAdmin => true,
        }
    }

    pub(crate) fn is_transfer_delayed(&self) -> bool {
        match self {
            Role::Admin => true,
            Role::EmergencyAdmin => true,
            Role::RewardsAdmin => false,
            Role::OperationsAdmin => false,
            Role::PauseAdmin => false,
            Role::EmergencyPauseAdmin => false,
        }
    }
}

pub trait SymbolRepresentation {
    fn as_symbol(&self, e: &Env) -> Symbol;
    fn from_symbol(e: &Env, value: Symbol) -> Self;
}

impl SymbolRepresentation for Role {
    fn as_symbol(&self, e: &Env) -> Symbol {
        match self {
            Role::Admin => Symbol::new(&e, "Admin"),
            Role::EmergencyAdmin => Symbol::new(&e, "EmergencyAdmin"),
            Role::RewardsAdmin => Symbol::new(&e, "RewardsAdmin"),
            Role::OperationsAdmin => Symbol::new(&e, "OperationsAdmin"),
            Role::PauseAdmin => Symbol::new(&e, "PauseAdmin"),
            Role::EmergencyPauseAdmin => Symbol::new(&e, "EmergencyPauseAdmin"),
        }
    }

    fn from_symbol(e: &Env, value: Symbol) -> Self {
        if value == Symbol::new(e, "Admin") {
            return Role::Admin;
        } else if value == Symbol::new(e, "EmergencyAdmin") {
            return Role::EmergencyAdmin;
        } else if value == Symbol::new(e, "RewardsAdmin") {
            return Role::RewardsAdmin;
        } else if value == Symbol::new(e, "OperationsAdmin") {
            return Role::OperationsAdmin;
        } else if value == Symbol::new(e, "PauseAdmin") {
            return Role::PauseAdmin;
        } else if value == Symbol::new(e, "EmergencyPauseAdmin") {
            return Role::EmergencyPauseAdmin;
        }
        panic_with_error!(e, AccessControlError::BadRoleUsage);
    }
}
