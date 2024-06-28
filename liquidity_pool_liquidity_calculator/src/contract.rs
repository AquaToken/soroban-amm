use crate::interface::{Calculator, UpgradeableContractTrait};
use crate::plane::{parse_stableswap_data, parse_standard_data, PoolPlaneClient};
use crate::storage::{get_plane, set_plane};
use crate::{stableswap_pool, standard_pool};
use access_control::access::{AccessControl, AccessControlTrait};
use soroban_sdk::{contract, contractimpl, symbol_short, Address, BytesN, Env, Symbol, Vec, U256};

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
        if !access_control.has_admin() {
            access_control.set_admin(&account)
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
        let access_control = AccessControl::new(&e);
        admin.require_auth();
        access_control.check_admin(&admin);

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
                // calculate liquidity for all non-duplicate permutations
                for i in 0..data.reserves.len() {
                    for j in 0..data.reserves.len() {
                        let in_idx = i;
                        let out_idx = data.reserves.len() - j - 1;
                        if in_idx == out_idx {
                            continue;
                        }

                        out = out.add(&U256::from_u128(
                            &e,
                            stableswap_pool::get_liquidity(
                                &e,
                                data.fee,
                                data.initial_a,
                                data.initial_a_time,
                                data.future_a,
                                data.future_a_time,
                                &data.reserves,
                                in_idx,
                                out_idx,
                            ),
                        ));
                    }
                }
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
impl UpgradeableContractTrait for LiquidityPoolLiquidityCalculator {
    // Returns the version of the contract.
    //
    // # Returns
    //
    // The version of the contract as a u32.
    fn version() -> u32 {
        104
    }

    // Upgrades the contract to a new version.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `new_wasm_hash` - The hash of the new contract version.
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}
