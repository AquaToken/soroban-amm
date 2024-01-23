use crate::storage::{get, update, Pool};
use soroban_sdk::{contract, contractimpl, Address, Env, Symbol, Vec};

#[contract]
pub struct LiquidityPoolPlane;

pub trait Plane {
    // update pool details. any contract can use it to store it's basic information
    fn update(
        e: Env,
        contract: Address,
        pool_type: Symbol,
        init_args: Vec<u128>,
        reserves: Vec<u128>,
    );

    // get pool details
    fn get(e: Env, contracts: Vec<Address>) -> Vec<(Symbol, Vec<u128>, Vec<u128>)>;
}

#[contractimpl]
impl Plane for LiquidityPoolPlane {
    fn update(
        e: Env,
        contract: Address,
        pool_type: Symbol,
        init_args: Vec<u128>,
        reserves: Vec<u128>,
    ) {
        contract.require_auth();
        update(
            &e,
            contract,
            &Pool {
                pool_type,
                init_args,
                reserves,
            },
        );
    }

    // pool_type, init_args, reserves
    fn get(e: Env, contracts: Vec<Address>) -> Vec<(Symbol, Vec<u128>, Vec<u128>)> {
        let mut result = Vec::new(&e);
        for i in 0..contracts.len() {
            let contract = contracts.get(i).unwrap();
            let data = get(&e, contract);
            result.push_back((data.pool_type, data.init_args, data.reserves));
        }
        result
    }
}
