use crate::interface::{RouterInterface, UpgradeableContract};
use crate::plane::{parse_stableswap_data, parse_standard_data, PoolPlaneClient};
use crate::storage::{get_plane, set_plane};
use crate::{stableswap_pool, standard_pool};
use access_control::access::{AccessControl, AccessControlTrait};
use access_control::errors::AccessControlError;
use liquidity_pool_validation_errors::LiquidityPoolValidationError;
use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, Address, BytesN, Env, Symbol, Vec,
};

#[contract]
pub struct LiquidityPoolSwapRouter;

pub const POOL_TYPE_STANDARD: Symbol = symbol_short!("standard");
pub const POOL_TYPE_STABLESWAP: Symbol = symbol_short!("stable");

#[contractimpl]
impl RouterInterface for LiquidityPoolSwapRouter {
    fn init_admin(e: Env, account: Address) {
        let access_control = AccessControl::new(&e);
        if access_control.has_admin() {
            panic_with_error!(&e, AccessControlError::AdminAlreadySet);
        }
        access_control.set_admin(&account);
    }

    fn set_pools_plane(e: Env, admin: Address, plane: Address) {
        let access_control = AccessControl::new(&e);
        admin.require_auth();
        access_control.check_admin(&admin);

        set_plane(&e, &plane);
    }
    fn get_pools_plane(e: Env) -> Address {
        get_plane(&e)
    }

    fn estimate_swap(
        e: Env,
        pools: Vec<Address>,
        in_idx: u32,
        out_idx: u32,
        in_amount: u128,
    ) -> (Address, u128) {
        if in_idx == out_idx {
            panic_with_error!(&e, LiquidityPoolValidationError::CannotSwapSameToken);
        }

        if in_idx > 1 {
            panic_with_error!(&e, LiquidityPoolValidationError::InTokenOutOfBounds);
        }

        if out_idx > 1 {
            panic_with_error!(&e, LiquidityPoolValidationError::OutTokenOutOfBounds);
        }

        let plane_client = PoolPlaneClient::new(&e, &get_plane(&e));
        let data = plane_client.get(&pools);
        let mut best_result = 0;
        let mut best_pool = pools.get(0).unwrap();
        for i in 0..pools.len() {
            let (pool_type, init_args, reserves) = data.get(i).unwrap();

            let out;
            if pool_type == POOL_TYPE_STANDARD {
                let data = parse_standard_data(init_args, reserves);
                out = standard_pool::estimate_swap(
                    &e,
                    data.fee,
                    data.reserves,
                    in_idx,
                    out_idx,
                    in_amount,
                );
            } else if pool_type == POOL_TYPE_STABLESWAP {
                let data = parse_stableswap_data(init_args, reserves);
                out = stableswap_pool::estimate_swap(
                    &e,
                    data.fee,
                    data.initial_a,
                    data.initial_a_time,
                    data.future_a,
                    data.future_a_time,
                    data.reserves,
                    in_idx,
                    out_idx,
                    in_amount,
                );
            } else {
                panic_with_error!(&e, LiquidityPoolValidationError::UnknownPoolType);
            };

            if best_result == 0 {
                best_result = out;
            } else if out > best_result {
                best_pool = pools.get(i).unwrap();
                best_result = out;
            }
        }
        (best_pool, best_result)
    }
}

#[contractimpl]
impl UpgradeableContract for LiquidityPoolSwapRouter {
    fn version() -> u32 {
        100
    }

    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}
