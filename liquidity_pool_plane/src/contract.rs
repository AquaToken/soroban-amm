use crate::interface::PlaneInterface;
use crate::storage::{get, update, PoolPlane};
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

#[contract]
pub struct LiquidityPoolPlane;

#[contractimpl]
impl PlaneInterface for LiquidityPoolPlane {
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
