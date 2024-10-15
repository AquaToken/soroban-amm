use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env};

use access_control::access::{AccessControl, AccessControlTrait, Role};
use access_control::errors::AccessControlError;

use crate::interface::{AdminInterface, UpgradeableContract};

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
        120
    }

    // Upgrades the contract to a new version.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `new_wasm_hash` - The hash of the new contract version.
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.get_role(Role::Admin).require_auth();
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}
