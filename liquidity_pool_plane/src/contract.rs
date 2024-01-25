use crate::interface::PlaneInterface;
use crate::storage::{get, update, PoolPlane};
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

#[contract]
pub struct LiquidityPoolPlane;

#[contractimpl]
impl PlaneInterface for LiquidityPoolPlane {
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

    // pool_type, init_args, reserves
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
