use crate::interface::Calculator;
use crate::plane::{parse_stableswap_data, parse_standard_data, PoolPlaneClient};
use crate::storage::{get_plane, set_plane};
use crate::{stableswap_pool, standard_pool};
use access_control::access::{AccessControl, AccessControlTrait};
use access_control::emergency::{get_emergency_mode, set_emergency_mode};
use access_control::errors::AccessControlError;
use access_control::events::Events as AccessControlEvents;
use access_control::interface::TransferableContract;
use access_control::management::SingleAddressManagementTrait;
use access_control::role::{Role, SymbolRepresentation};
use access_control::transfer::TransferOwnershipTrait;
use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, Address, BytesN, Env, Symbol, Vec, U256,
};
use upgrade::events::Events as UpgradeEvents;
use upgrade::interface::UpgradeableContract;
use upgrade::{apply_upgrade, commit_upgrade, revert_upgrade};

#[contract]
pub struct LiquidityPoolLiquidityCalculator;

const POOL_TYPE_STANDARD: Symbol = symbol_short!("standard");
const POOL_TYPE_STABLESWAP: Symbol = symbol_short!("stable");

#[contractimpl]
impl Calculator for LiquidityPoolLiquidityCalculator {
    // Initializes the admin for the contract.
    // If an admin does not exist, it sets the provided account as the admin.
    //
    // # Arguments
    //
    // * `account` - The account to be set as the admin.
    fn init_admin(e: Env, account: Address) {
        let access_control = AccessControl::new(&e);
        if !access_control.get_role_safe(&Role::Admin).is_some() {
            access_control.set_role_address(&Role::Admin, &account)
        }
    }

    // Sets the plane for the pools.
    // It requires the caller to be an admin and checks if the caller is an admin before setting the plane.
    //
    // # Arguments
    //
    // * `admin` - The admin account.
    // * `plane` - The plane to be set for the pools.
    fn set_pools_plane(e: Env, admin: Address, plane: Address) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);

        set_plane(&e, &plane);
    }

    // Returns the plane of the pools.
    //
    // # Returns
    //
    // * The address of the plane of the pools.
    fn get_pools_plane(e: Env) -> Address {
        get_plane(&e)
    }

    // Calculates and returns the liquidity of the provided pools.
    // It interacts with the `PoolPlaneClient` to get the data for the pools
    // and then calculates the liquidity based on the pool type (standard or stableswap).
    //
    // # Arguments
    //
    // * `pools` - A vector of addresses representing the pools.
    //
    // # Returns
    //
    // * A vector of U256 values representing the liquidity of the provided pools.
    fn get_liquidity(e: Env, pools: Vec<Address>) -> Vec<U256> {
        let plane_client = PoolPlaneClient::new(&e, &get_plane(&e));
        let data = plane_client.get(&pools);
        let mut result = Vec::new(&e);
        for pool_idx in 0..pools.len() {
            let (pool_type, init_args, reserves) = data.get(pool_idx).unwrap();

            let mut out = U256::from_u32(&e, 0);
            if pool_type == POOL_TYPE_STANDARD {
                let (fee, reserves) = parse_standard_data(init_args, reserves);
                out = out.add(&U256::from_u128(
                    &e,
                    standard_pool::get_liquidity(&e, fee, &reserves, 0, 1),
                ));
                out = out.add(&U256::from_u128(
                    &e,
                    standard_pool::get_liquidity(&e, fee, &reserves, 1, 0),
                ));
            } else if pool_type == POOL_TYPE_STABLESWAP {
                let data = parse_stableswap_data(init_args, reserves);
                let amp = stableswap_pool::a(
                    &e,
                    data.initial_a,
                    data.initial_a_time,
                    data.future_a,
                    data.future_a_time,
                );
                out = stableswap_pool::get_pool_liquidity(&e, data.fee, amp, &data.xp);
            } else {
                panic!("unknown pool type");
            };

            result.push_back(out);
        }
        result
    }
}

// The `UpgradeableContract` trait provides the interface for upgrading the contract.
#[contractimpl]
impl UpgradeableContract for LiquidityPoolLiquidityCalculator {
    // Returns the version of the contract.
    //
    // # Returns
    //
    // The version of the contract as a u32.
    fn version() -> u32 {
        150
    }

    // Commits a new wasm hash for a future upgrade.
    // The upgrade will be available through `apply_upgrade` after the standard upgrade delay
    // unless the system is in emergency mode.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `new_wasm_hash` - The new wasm hash to commit.
    fn commit_upgrade(e: Env, admin: Address, new_wasm_hash: BytesN<32>) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        commit_upgrade(&e, &new_wasm_hash);
        UpgradeEvents::new(&e).commit_upgrade(Vec::from_array(&e, [new_wasm_hash.clone()]));
    }

    // Applies the committed upgrade.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn apply_upgrade(e: Env, admin: Address) -> BytesN<32> {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        let new_wasm_hash = apply_upgrade(&e);
        UpgradeEvents::new(&e).apply_upgrade(Vec::from_array(&e, [new_wasm_hash.clone()]));
        new_wasm_hash
    }

    // Reverts the committed upgrade.
    // This can be used to cancel a previously committed upgrade.
    // The upgrade will be canceled only if it has not been applied yet.
    // If the upgrade has already been applied, it cannot be reverted.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn revert_upgrade(e: Env, admin: Address) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        revert_upgrade(&e);
        UpgradeEvents::new(&e).revert_upgrade();
    }

    // Sets the emergency mode.
    // When the emergency mode is set to true, the contract will allow instant upgrades without the delay.
    // This is useful in case of critical issues that need to be fixed immediately.
    // When the emergency mode is set to false, the contract will require the standard upgrade delay.
    // The emergency mode can only be set by the emergency admin.
    //
    // # Arguments
    //
    // * `emergency_admin` - The address of the emergency admin.
    // * `value` - The value to set the emergency mode to.
    fn set_emergency_mode(e: Env, emergency_admin: Address, value: bool) {
        emergency_admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&emergency_admin, &Role::EmergencyAdmin);
        set_emergency_mode(&e, &value);
        AccessControlEvents::new(&e).set_emergency_mode(value);
    }

    // Returns the emergency mode flag value.
    fn get_emergency_mode(e: Env) -> bool {
        get_emergency_mode(&e)
    }
}

// The `TransferableContract` trait provides the interface for transferring ownership of the contract.
#[contractimpl]
impl TransferableContract for LiquidityPoolLiquidityCalculator {
    // Commits an ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `role_name` - The name of the role to transfer ownership of. The role must be one of the following:
    //     * `Admin`
    //     * `EmergencyAdmin`
    // * `new_address` - New address for the role
    fn commit_transfer_ownership(e: Env, admin: Address, role_name: Symbol, new_address: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, &Role::Admin);

        let role = Role::from_symbol(&e, role_name);
        access_control.commit_transfer_ownership(&role, &new_address);
        AccessControlEvents::new(&e).commit_transfer_ownership(role, new_address);
    }

    // Applies the committed ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `role_name` - The name of the role to transfer ownership of. The role must be one of the following:
    //     * `Admin`
    //     * `EmergencyAdmin`
    fn apply_transfer_ownership(e: Env, admin: Address, role_name: Symbol) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, &Role::Admin);

        let role = Role::from_symbol(&e, role_name);
        let new_address = access_control.apply_transfer_ownership(&role);
        AccessControlEvents::new(&e).apply_transfer_ownership(role, new_address);
    }

    // Reverts the committed ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `role_name` - The name of the role to transfer ownership of. The role must be one of the following:
    //     * `Admin`
    //     * `EmergencyAdmin`
    fn revert_transfer_ownership(e: Env, admin: Address, role_name: Symbol) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, &Role::Admin);

        let role = Role::from_symbol(&e, role_name);
        access_control.revert_transfer_ownership(&role);
        AccessControlEvents::new(&e).revert_transfer_ownership(role);
    }

    // Returns the future address for the role.
    // The future address is the address that the ownership of the role will be transferred to.
    // The future address is set using the `commit_transfer_ownership` function.
    // The address will be defaulted to the current address if the transfer is not committed.
    //
    // # Arguments
    //
    // * `role_name` - The name of the role to get the future address for. The role must be one of the following:
    //    * `Admin`
    //    * `EmergencyAdmin`
    fn get_future_address(e: Env, role_name: Symbol) -> Address {
        let access_control = AccessControl::new(&e);
        let role = Role::from_symbol(&e, role_name);
        match access_control.get_transfer_ownership_deadline(&role) {
            0 => match access_control.get_role_safe(&role) {
                Some(address) => address,
                None => panic_with_error!(&e, AccessControlError::RoleNotFound),
            },
            _ => access_control.get_future_address(&role),
        }
    }
}
