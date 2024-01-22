use crate::plane::{parse_stableswap_data, parse_standard_data, PoolPlaneClient};
use crate::storage::{get_plane, set_plane};
use crate::{stableswap_pool, standard_pool};
use soroban_sdk::{contract, contractimpl, symbol_short, Address, Env, Symbol, Vec};

#[contract]
pub struct LiquidityPoolSwapRouter;

pub trait Router {
    fn initialize_plane(e: Env, plane: Address);

    fn estimate_swap(
        e: Env,
        pools: Vec<Address>,
        in_idx: u32,
        out_idx: u32,
        in_amount: u128,
    ) -> (Address, u128);
}

const POOL_TYPE_STANDARD: Symbol = symbol_short!("standard");
const POOL_TYPE_STABLESWAP: Symbol = symbol_short!("stable");

#[contractimpl]
impl Router for LiquidityPoolSwapRouter {
    fn initialize_plane(e: Env, plane: Address) {
        set_plane(&e, &plane);
    }

    fn estimate_swap(
        e: Env,
        pools: Vec<Address>,
        in_idx: u32,
        out_idx: u32,
        in_amount: u128,
    ) -> (Address, u128) {
        if in_idx == out_idx {
            panic!("cannot swap token to same one")
        }

        if in_idx > 1 {
            panic!("in_idx out of bounds");
        }

        if out_idx > 1 {
            panic!("out_idx out of bounds");
        }

        let plane_client = PoolPlaneClient::new(&e, &get_plane(&e));
        let data = plane_client.get(&pools);
        let mut best_result = 0;
        let mut best_pool = pools.get(0).unwrap();
        for i in 0..pools.len() {
            let (pool_type, init_args, reserves) = data.get(i).unwrap();

            let out;
            if pool_type == POOL_TYPE_STANDARD {
                let (fee, reserves) = parse_standard_data(init_args, reserves);
                out = standard_pool::estimate_swap(reserves, fee, in_idx, out_idx, in_amount);
            } else if pool_type == POOL_TYPE_STABLESWAP {
                let (fee, a, reserves) = parse_stableswap_data(init_args, reserves);
                out = stableswap_pool::estimate_swap(reserves, fee, a, in_idx, out_idx, in_amount);
            } else {
                panic!("unknown pool type");
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
