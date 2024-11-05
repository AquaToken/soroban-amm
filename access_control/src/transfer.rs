use crate::access::AccessControl;
use crate::constants::ADMIN_ACTIONS_DELAY;
use crate::errors::AccessControlError;
use crate::role::Role;
use crate::storage::StorageTrait;
use soroban_sdk::{panic_with_error, Address};
use utils::bump::bump_instance;
use utils::storage_errors::StorageError;

pub trait TransferOwnershipTrait {
    fn get_transfer_ownership_deadline(&self, role: &Role) -> u64;
    fn put_transfer_ownership_deadline(&self, role: &Role, value: u64);
    fn get_future_address(&self, role: &Role) -> Address;
    fn commit_transfer_ownership(&self, role: &Role, future_address: &Address);
    fn apply_transfer_ownership(&self, role: &Role) -> Address;
    fn revert_transfer_ownership(&self, role: &Role);
}

impl TransferOwnershipTrait for AccessControl {
    fn get_transfer_ownership_deadline(&self, role: &Role) -> u64 {
        let key = self.get_future_deadline_key(role);
        bump_instance(&self.0);
        self.0.storage().instance().get(&key).unwrap_or(0)
    }

    fn put_transfer_ownership_deadline(&self, role: &Role, value: u64) {
        let key = self.get_future_deadline_key(role);
        bump_instance(&self.0);
        self.0.storage().instance().set(&key, &value);
    }

    fn get_future_address(&self, role: &Role) -> Address {
        if !role.is_transfer_delayed() || role.has_many_users() {
            panic_with_error!(&self.0, AccessControlError::BadRoleUsage);
        }

        match self.0.storage().instance().get(&self.get_future_key(role)) {
            Some(v) => v,
            None => panic_with_error!(&self.0, AccessControlError::NoActionActive),
        }
    }

    fn commit_transfer_ownership(&self, role: &Role, future_address: &Address) {
        if !role.is_transfer_delayed() || role.has_many_users() {
            panic_with_error!(&self.0, AccessControlError::BadRoleUsage);
        }

        if self.get_transfer_ownership_deadline(role) != 0 {
            panic_with_error!(&self.0, AccessControlError::AnotherActionActive);
        }

        let deadline = self.0.ledger().timestamp() + ADMIN_ACTIONS_DELAY;
        self.put_transfer_ownership_deadline(role, deadline);

        bump_instance(&self.0);
        self.0
            .storage()
            .instance()
            .set(&self.get_future_key(role), future_address);
    }

    fn apply_transfer_ownership(&self, role: &Role) -> Address {
        let storage = self.0.storage().instance();
        let role_key = self.get_key(role);
        let has_value = storage.has(&role_key);
        if has_value && self.0.ledger().timestamp() < self.get_transfer_ownership_deadline(role) {
            panic_with_error!(&self.0, AccessControlError::ActionNotReadyYet);
        }
        if self.get_transfer_ownership_deadline(role) == 0 {
            panic_with_error!(&self.0, AccessControlError::NoActionActive);
        }

        self.put_transfer_ownership_deadline(role, 0);
        let future_address: Address =
            match self.0.storage().instance().get(&self.get_future_key(role)) {
                Some(v) => v,
                None => panic_with_error!(&self.0, StorageError::ValueNotInitialized),
            };

        bump_instance(&self.0);
        storage.set(&self.get_key(role), &future_address);

        future_address
    }

    fn revert_transfer_ownership(&self, role: &Role) {
        self.put_transfer_ownership_deadline(role, 0);
    }
}
