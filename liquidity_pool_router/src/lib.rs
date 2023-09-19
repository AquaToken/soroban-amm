#![no_std]

mod admin;
mod pool_contract;
mod storage;
mod test;
pub mod testutils;
mod storage_types;

use crate::admin::{get_admin, has_admin, require_admin, set_admin};
use crate::storage::{
    add_pool_to_list, get_pool_hash, get_pool_id, get_pools_list, get_token_hash, has_pool,
    put_pool, set_pool_hash, set_token_hash,
};
use pool_contract::LiquidityPoolClient;
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{contract, contractimpl, contracttype, Address, Bytes, BytesN, Env, Vec};

mod token {
    // soroban_sdk::contractimport!(file = "../soroban_token_spec.wasm");

    soroban_sdk::contractimport!(
        file = "../token/target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
}

pub trait LiquidityPoolRouterTrait {
    fn init_pool(e: Env, token_a: Address, token_b: Address) -> Address;

    fn get_pools_list(e: Env) -> Vec<Address>;

    fn get_pool_hash(e: Env) -> BytesN<32>;
    fn set_pool_hash(e: Env, new_hash: BytesN<32>);

    fn get_token_hash(e: Env) -> BytesN<32>;
    fn set_token_hash(e: Env, new_hash: BytesN<32>);

    fn deposit(
        e: Env,
        account: Address,
        token_a: Address,
        token_b: Address,
        desired_a: i128,
        min_a: i128,
        desired_b: i128,
        min_b: i128,
    ) -> (i128, i128);

    // swaps out an exact amount of "buy", in exchange for "sell" that this contract has an
    // allowance for from "to". "sell" amount swapped in must not be greater than "in_max"
    fn swap_out(
        e: Env,
        account: Address,
        sell: Address,
        buy: Address,
        out: i128,
        in_max: i128,
    ) -> i128;
    fn estimate_swap_out(e: Env, sell: Address, buy: Address, out: i128) -> i128;

    fn withdraw(
        e: Env,
        account: Address,
        token_a: Address,
        token_b: Address,
        share_amount: i128,
        min_a: i128,
        min_b: i128,
    ) -> (i128, i128);

    // returns the contract address for the specified token_a/token_b combo
    fn get_pool(e: Env, token_a: Address, token_b: Address) -> (bool, Address);

    // get pool reserves amount. it may differ from pool balance
    fn get_reserves(e: Env, token_a: Address, token_b: Address) -> (i128, i128);
}

fn sort(a: &Address, b: &Address) -> (Address, Address) {
    if a < b {
        return (a.clone(), b.clone());
    } else if a > b {
        return (b.clone(), a.clone());
    }
    panic!("a and b can't be the same")
}

pub fn pool_salt(e: &Env, token_a: &Address, token_b: &Address) -> BytesN<32> {
    if token_a >= token_b {
        panic!("token_a must be less t&han token_b");
    }

    let mut salt = Bytes::new(e);
    salt.append(&token_a.to_xdr(e));
    salt.append(&token_b.to_xdr(e));
    e.crypto().sha256(&salt)
}

pub trait UpgradeableContract {
    fn init_admin(e: Env, account: Address);
    fn version() -> u32;
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
}

#[contract]
struct LiquidityPoolRouter;

#[contractimpl]
impl LiquidityPoolRouterTrait for LiquidityPoolRouter {
    fn init_pool(e: Env, token_a: Address, token_b: Address) -> Address {
        let salt = pool_salt(&e, &token_a, &token_b);
        if !has_pool(&e, &salt) {
            let liquidity_pool_wasm_hash = get_pool_hash(&e);
            let token_wasm_hash = get_token_hash(&e);
            let admin = get_admin(&e);

            let pool_contract_id = e
                .deployer()
                .with_current_contract(salt.clone())
                .deploy(liquidity_pool_wasm_hash);

            put_pool(&e, &salt, &pool_contract_id);

            // TODO: NOT FOR PRODUCTION
            //  this is unsafe as we can store limited amount of records
            add_pool_to_list(&e, &get_pool_id(&e, &salt));

            LiquidityPoolClient::new(&e, &pool_contract_id).initialize(
                &admin,
                &token_wasm_hash,
                &token_a,
                &token_b,
            );
        }
        let (_pool_exists, pool_id) = Self::get_pool(e.clone(), token_a, token_b);
        pool_id
    }

    fn get_pools_list(e: Env) -> Vec<Address> {
        get_pools_list(&e)
    }

    fn get_pool_hash(e: Env) -> BytesN<32> {
        get_pool_hash(&e)
    }

    fn set_pool_hash(e: Env, new_hash: BytesN<32>) {
        require_admin(&e);
        set_pool_hash(&e, &new_hash);
    }

    fn get_token_hash(e: Env) -> BytesN<32> {
        get_token_hash(&e)
    }

    fn set_token_hash(e: Env, new_hash: BytesN<32>) {
        require_admin(&e);
        set_token_hash(&e, &new_hash);
    }

    fn deposit(
        e: Env,
        account: Address,
        token_a: Address,
        token_b: Address,
        desired_a: i128,
        min_a: i128,
        desired_b: i128,
        min_b: i128,
    ) -> (i128, i128) {
        account.require_auth();

        let salt = pool_salt(&e, &token_a, &token_b);
        let pool_id = get_pool_id(&e, &salt);

        LiquidityPoolClient::new(&e, &pool_id)
            .deposit(&account, &desired_a, &min_a, &desired_b, &min_b)
    }

    fn swap_out(
        e: Env,
        account: Address,
        sell: Address,
        buy: Address,
        out: i128,
        in_max: i128,
    ) -> i128 {
        account.require_auth();
        let (token_a, token_b) = sort(&sell, &buy);
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), token_a.clone(), token_b);
        if !pool_exists {
            panic!("pool not exists")
        }

        LiquidityPoolClient::new(&e, &pool_id).swap(&account, &(buy == token_a), &out, &in_max)
    }

    fn estimate_swap_out(e: Env, sell: Address, buy: Address, out: i128) -> i128 {
        let (token_a, token_b) = sort(&sell, &buy);
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), token_a.clone(), token_b);
        if !pool_exists {
            panic!("pool not exists")
        }

        LiquidityPoolClient::new(&e, &pool_id).estimate_swap_out(&(buy == token_a), &out)
    }

    fn withdraw(
        e: Env,
        account: Address,
        token_a: Address,
        token_b: Address,
        share_amount: i128,
        min_a: i128,
        min_b: i128,
    ) -> (i128, i128) {
        account.require_auth();
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), token_a, token_b);
        if !pool_exists {
            panic!("pool not exists")
        }

        let pool_client = LiquidityPoolClient::new(&e, &pool_id);

        let (amount_a, amount_b) = pool_client.withdraw(&account, &share_amount, &min_a, &min_b);

        if amount_a < min_a || amount_b < min_b {
            panic!("min not satisfied");
        }
        (amount_a, amount_b)
    }

    fn get_pool(e: Env, token_a: Address, token_b: Address) -> (bool, Address) {
        let salt = pool_salt(&e, &token_a, &token_b);
        (has_pool(&e, &salt), get_pool_id(&e, &salt))
    }

    fn get_reserves(e: Env, token_a: Address, token_b: Address) -> (i128, i128) {
        let (a, b) = sort(&token_a, &token_b);
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), a, b);
        if !pool_exists {
            panic!("pool not exists")
        }
        LiquidityPoolClient::new(&e, &pool_id).get_rsrvs()
    }
}

#[contractimpl]
impl UpgradeableContract for LiquidityPoolRouter {
    fn init_admin(e: Env, account: Address) {
        if !has_admin(&e) {
            set_admin(&e, &account)
        }
    }

    fn version() -> u32 {
        5
    }

    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        require_admin(&e);
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}
