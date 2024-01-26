use crate::plane::{parse_stableswap_data, parse_standard_data, PoolPlaneClient};
use crate::storage::{get_plane, set_plane};
use crate::{stableswap_pool, standard_pool};
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol, Vec};
use access_control::access::{AccessControl, AccessControlTrait};
use crate::interface::Calculator;

#[contract]
pub struct LiquidityPoolLiquidityCalculator;

const POOL_TYPE_STANDARD: Symbol = symbol_short!("standard");
const POOL_TYPE_STABLESWAP: Symbol = symbol_short!("stable");

#[contractimpl]
impl Calculator for LiquidityPoolLiquidityCalculator {
    fn init_admin(e: Env, account: Address) {
        let access_control = AccessControl::new(&e);
        if !access_control.has_admin() {
            access_control.set_admin(&account)
        }
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

    fn get_liquidity(
        e: Env,
        pools: Vec<Address>,
    ) -> Vec<u128> {
        let plane_client = PoolPlaneClient::new(&e, &get_plane(&e));
        let data = plane_client.get(&pools);
        let mut result = Vec::new(&e);
        for i in 0..pools.len() {
            let (pool_type, init_args, reserves) = data.get(i).unwrap();

            let out;
            if pool_type == POOL_TYPE_STANDARD {
                let (fee, reserves) = parse_standard_data(init_args, reserves);
                out = standard_pool::get_liquidity(&reserves, 0, 1, fee) + standard_pool::get_liquidity(&reserves, 1, 0, fee);
            } else if pool_type == POOL_TYPE_STABLESWAP {
                let data = parse_stableswap_data(init_args, reserves);
                out = stableswap_pool::get_liquidity(
                    &e,
                    data.fee,
                    data.initial_a,
                    data.initial_a_time,
                    data.future_a,
                    data.future_a_time,
                    &data.reserves,
                    0,
                    1,
                ) + stableswap_pool::get_liquidity(
                    &e,
                    data.fee,
                    data.initial_a,
                    data.initial_a_time,
                    data.future_a,
                    data.future_a_time,
                    &data.reserves,
                    1,
                    0,
                );
            } else {
                panic!("unknown pool type");
            };

            result.push_back(out);
        }
        result
    }
}
