use crate::interface::{PlaneInterface, UpgradeableContract};
use crate::storage::{get, update, PoolPlane};
use access_control::access::{AccessControl, AccessControlTrait, Role};
use access_control::errors::AccessControlError;
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, Symbol, Vec};

#[contract]
pub struct LiquidityPoolPlane;

#[contractimpl]
impl PlaneInterface for LiquidityPoolPlane {
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

    // Updates the pool stored data. Any pool can use it to store its information.
    //
    // # Arguments
    //
    // * `pool` - The address of the pool.
    // * `pool_type` - The type of the pool.
    // * `init_args` - The initialization arguments for the pool.
    // * `reserves` - The reserves of the pool.
    fn update(e: Env, pool: Address, pool_type: Symbol, init_args: Vec<u128>, reserves: Vec<u128>) {
        pool.require_auth();
        update(
            &e,
            pool,
            &PoolPlane {
                pool_type,
                init_args,
                reserves,
            },
        );
    }

    // Gets details for many pools: type string representation, pool parameters and reserves amount.
    //
    // # Arguments
    //
    // * `pools` - A vector of addresses representing the pools.
    //
    // # Returns
    //
    // * A vector of tuples, each containing the type of the pool, the initialization arguments, and the reserves of the pool.
    fn get(e: Env, pools: Vec<Address>) -> Vec<(Symbol, Vec<u128>, Vec<u128>)> {
        let mut result = Vec::new(&e);
        for i in 0..pools.len() {
            let pool = pools.get(i).unwrap();
            let data = get(&e, pool);
            result.push_back((data.pool_type, data.init_args, data.reserves));
        }
        result
    }
}

// The `UpgradeableContract` trait provides the interface for upgrading the contract.
#[contractimpl]
impl UpgradeableContract for LiquidityPoolPlane {
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
