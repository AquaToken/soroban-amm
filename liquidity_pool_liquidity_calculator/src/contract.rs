use crate::interface::{Calculator, UpgradeableContract};
use crate::plane::{parse_stableswap_data, parse_standard_data, PoolPlaneClient};
use crate::storage::{get_plane, set_plane};
use crate::{stableswap_pool, standard_pool};
use access_control::access::{AccessControl, AccessControlTrait, Role, TransferOwnershipTrait};
use access_control::events::Events as AccessControlEvents;
use access_control::interface::TransferableContract;
use soroban_sdk::{contract, contractimpl, symbol_short, Address, BytesN, Env, Symbol, Vec, U256};
use access_control::emergency::{get_emergency_mode, set_emergency_mode};
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
        if !access_control.get_role_safe(Role::Admin).is_some() {
            access_control.set_role_address(Role::Admin, &account)
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
        AccessControl::new(&e).assert_address_has_role(&admin, Role::Admin);

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
                out = stableswap_pool::get_pool_liquidity(&e, data.fee, amp, &data.reserves);
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
impl TransferableContract for LiquidityPoolLiquidityCalculator {
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
