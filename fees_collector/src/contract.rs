use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env};

use crate::interface::{AdminInterface, UpgradeableContract};
use access_control::access::{AccessControl, AccessControlTrait, Role, TransferOwnershipTrait};
use access_control::emergency::{get_emergency_mode, set_emergency_mode};
use access_control::errors::AccessControlError;
use access_control::events::Events as AccessControlEvents;
use access_control::interface::TransferableContract;
use upgrade::{apply_upgrade, commit_upgrade, revert_upgrade};

#[contract]
pub struct FeesCollector;

#[contractimpl]
impl AdminInterface for FeesCollector {
    // Initializes the admin user.
    //
    // # Arguments
    //
    // * `account` - The address of the admin user.
    fn init_admin(e: Env, account: Address) {
        let access_control = AccessControl::new(&e);
        if access_control.get_role_safe(Role::Admin).is_some() {
            panic_with_error!(&e, AccessControlError::AdminAlreadySet);
        }
        access_control.set_role_address(Role::Admin, &account);
    }
}

// The `UpgradeableContract` trait provides the interface for upgrading the contract.
#[contractimpl]
impl UpgradeableContract for FeesCollector {
    // Returns the version of the contract.
    //
    // # Returns
    //
    // The version of the contract as a u32.
    fn version() -> u32 {
        130
    }

    fn set_emergency_admin(e: Env, admin: Address, emergency_admin: Address) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, Role::Admin);
        AccessControl::new(&e).set_role_address(Role::EmergencyAdmin, &emergency_admin);
    }

    fn set_emergency_mode(e: Env, admin: Address, value: bool) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, Role::EmergencyAdmin);
        set_emergency_mode(&e, &value);
    }

    fn get_emergency_mode(e: Env) -> bool {
        get_emergency_mode(&e)
    }

    fn commit_upgrade(e: Env, admin: Address, new_wasm_hash: BytesN<32>) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, Role::Admin);
        commit_upgrade(&e, &new_wasm_hash);
    }

    fn apply_upgrade(e: Env, admin: Address) -> BytesN<32> {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, Role::Admin);
        apply_upgrade(&e)
    }

    fn revert_upgrade(e: Env, admin: Address) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, Role::Admin);
        revert_upgrade(&e);
    }
}

// The `TransferableContract` trait provides the interface for transferring ownership of the contract.
#[contractimpl]
impl TransferableContract for FeesCollector {
    // Commits an ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `new_admin` - The address of the new admin.
    fn commit_transfer_ownership(e: Env, admin: Address, new_admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, Role::Admin);
        access_control.commit_transfer_ownership(new_admin.clone());
        AccessControlEvents::new(&e).commit_transfer_ownership(new_admin);
    }

    // Applies the committed ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn apply_transfer_ownership(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, Role::Admin);
        let new_admin = access_control.apply_transfer_ownership();
        AccessControlEvents::new(&e).apply_transfer_ownership(new_admin);
    }

    // Reverts the committed ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn revert_transfer_ownership(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, Role::Admin);
        access_control.revert_transfer_ownership();
        AccessControlEvents::new(&e).revert_transfer_ownership();
    }
}
