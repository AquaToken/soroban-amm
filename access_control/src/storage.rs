use crate::access::AccessControl;
use crate::errors::AccessControlError;
use crate::role::Role;
use soroban_sdk::{contracttype, panic_with_error};

#[derive(Clone)]
#[contracttype]
pub(crate) enum DataKey {
    Admin,           // owner - upgrade, set privileged roles
    EmergencyAdmin,  // emergency admin - put system into emergency mode, allowing instant upgrade
    Operator,        // rewards admin - configure rewards. legacy name cannot be changed
    OperationsAdmin, // operations admin - add/remove pools, ramp A, set fees, etc
    PauseAdmin,      // pause admin - pause/unpause pools
    EmPauseAdmins,   // emergency pause admin - pause pools in emergency

    // transfer ownership - pending values
    FutureAdmin,
    FutureEmergencyAdmin,

    // transfer ownership - deadlines
    TransferOwnershipDeadline,
    EmAdminTransferOwnershipDeadline,

    // emergency mode
    EmergencyMode,
}

pub(crate) trait StorageTrait {
    fn get_key(&self, role: &Role) -> DataKey;
    fn get_future_key(&self, role: &Role) -> DataKey;
    fn get_future_deadline_key(&self, role: &Role) -> DataKey;
}

impl StorageTrait for AccessControl {
    fn get_key(&self, role: &Role) -> DataKey {
        match role {
            Role::Admin => DataKey::Admin,
            Role::EmergencyAdmin => DataKey::EmergencyAdmin,
            Role::RewardsAdmin => DataKey::Operator,
            Role::OperationsAdmin => DataKey::OperationsAdmin,
            Role::PauseAdmin => DataKey::PauseAdmin,
            Role::EmergencyPauseAdmin => DataKey::EmPauseAdmins,
        }
    }

    fn get_future_key(&self, role: &Role) -> DataKey {
        match role {
            Role::Admin => DataKey::FutureAdmin,
            Role::EmergencyAdmin => DataKey::FutureEmergencyAdmin,
            _ => panic_with_error!(&self.0, AccessControlError::BadRoleUsage),
        }
    }

    fn get_future_deadline_key(&self, role: &Role) -> DataKey {
        match role {
            Role::Admin => DataKey::TransferOwnershipDeadline,
            Role::EmergencyAdmin => DataKey::EmAdminTransferOwnershipDeadline,
            _ => panic_with_error!(&self.0, AccessControlError::BadRoleUsage),
        }
    }
}
