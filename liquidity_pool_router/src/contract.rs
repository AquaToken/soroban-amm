use crate::admin::{has_admin, require_admin, set_admin};
use crate::pool_interface::{LiquidityPoolInterfaceTrait, PoolsManagementTrait};
use crate::router_interface::{AdminInterface, LiquidityPoolRouterTrait, UpgradeableContract};
use crate::{pool_utils, storage};
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env};
use soroban_sdk::{symbol_short, IntoVal, Map, Symbol, Vec};

#[contract]
pub struct LiquidityPoolRouter;

#[contractimpl]
impl LiquidityPoolInterfaceTrait for LiquidityPoolRouter {
    fn get_pool(
        e: Env,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
    ) -> (bool, Address) {
        let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
        (
            storage::has_pool(&e, &salt, pool_index.clone()),
            storage::get_pool(&e, &salt, pool_index),
        )
    }

    fn share_id(e: Env, token_a: Address, token_b: Address, pool_index: BytesN<32>) -> Address {
        let (token_a, token_b) = crate::utils::sort(&token_a, &token_b);
        let (pool_exists, pool_id) =
            Self::get_pool(e.clone(), token_a.clone(), token_b.clone(), pool_index);
        if !pool_exists {
            panic!("pool not exists")
        }

        e.invoke_contract(&pool_id, &Symbol::new(&e, "share_id"), Vec::new(&e))
    }

    fn get_reserves(
        e: Env,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
    ) -> Vec<i128> {
        let (token_a, token_b) = crate::utils::sort(&token_a, &token_b);
        let (pool_exists, pool_id) =
            Self::get_pool(e.clone(), token_a.clone(), token_b.clone(), pool_index);
        if !pool_exists {
            panic!("pool not exists")
        }

        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_reserves"), Vec::new(&e))
    }

    fn get_tokens(
        e: Env,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
    ) -> Vec<Address> {
        let (token_a, token_b) = crate::utils::sort(&token_a, &token_b);
        let (pool_exists, pool_id) =
            Self::get_pool(e.clone(), token_a.clone(), token_b.clone(), pool_index);
        if !pool_exists {
            panic!("pool not exists")
        }

        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_tokens"), Vec::new(&e))
    }

    fn deposit(
        e: Env,
        user: Address,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
        desired_amounts: Vec<i128>,
    ) -> (Vec<i128>, i128) {
        user.require_auth();

        let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
        let pool_id = storage::get_pool(&e, &salt, pool_index);

        let (amounts, share_amount): (Vec<i128>, i128) = e.invoke_contract(
            &pool_id,
            &symbol_short!("deposit"),
            Vec::from_array(
                &e,
                [user.clone().into_val(&e), desired_amounts.into_val(&e)],
            ),
        );

        // e.events().publish(
        //     (Symbol::new(&e, "deposit"), token_a, token_b, account),
        //     vec![&e, pool_id, amount_a, amount_b],
        // );

        (amounts, share_amount)
    }

    fn swap(
        e: Env,
        user: Address,
        token_in: Address,
        token_out: Address,
        pool_index: BytesN<32>,
        in_amount: i128,
        out_min: i128,
    ) -> i128 {
        user.require_auth();
        let (token_a, token_b) = crate::utils::sort(&token_in, &token_out);
        let (pool_exists, pool_id) = Self::get_pool(
            e.clone(),
            token_a.clone(),
            token_b.clone(),
            pool_index.clone(),
        );
        if !pool_exists {
            panic!("pool not exists")
        }
        let tokens: Vec<Address> = Self::get_tokens(
            e.clone(),
            token_a.clone(),
            token_b.clone(),
            pool_index.clone(),
        );

        let out_amt = e.invoke_contract(
            &pool_id,
            &symbol_short!("swap"),
            Vec::from_array(
                &e,
                [
                    user.clone().into_val(&e),
                    tokens
                        .first_index_of(token_in.clone())
                        .unwrap()
                        .into_val(&e),
                    tokens
                        .first_index_of(token_out.clone())
                        .unwrap()
                        .into_val(&e),
                    in_amount.into_val(&e),
                    out_min.into_val(&e),
                ],
            ),
        );

        e.events().publish(
            (Symbol::new(&e, "swap"), token_a, token_b, user),
            (pool_id, token_in, token_out, in_amount, out_amt),
        );

        out_amt
    }

    fn estimate_swap(
        e: Env,
        token_in: Address,
        token_out: Address,
        pool_index: BytesN<32>,
        in_amount: i128,
    ) -> i128 {
        let (token_a, token_b) = crate::utils::sort(&token_in, &token_out);
        let (pool_exists, pool_id) = Self::get_pool(
            e.clone(),
            token_a.clone(),
            token_b.clone(),
            pool_index.clone(),
        );
        if !pool_exists {
            panic!("pool not exists")
        }
        let tokens: Vec<Address> = Self::get_tokens(
            e.clone(),
            token_a.clone(),
            token_b.clone(),
            pool_index.clone(),
        );

        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "estimate_swap"),
            Vec::from_array(
                &e,
                [
                    tokens
                        .first_index_of(token_in.clone())
                        .unwrap()
                        .into_val(&e),
                    tokens
                        .first_index_of(token_out.clone())
                        .unwrap()
                        .into_val(&e),
                    in_amount.into_val(&e),
                ],
            ),
        )
    }

    fn withdraw(
        e: Env,
        user: Address,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
        share_amount: i128,
        min_amounts: Vec<i128>,
    ) -> Vec<i128> {
        user.require_auth();
        let (token_a, token_b) = crate::utils::sort(&token_a, &token_b);
        let (pool_exists, pool_id) = Self::get_pool(
            e.clone(),
            token_a.clone(),
            token_b.clone(),
            pool_index.clone(),
        );
        if !pool_exists {
            panic!("pool not exists")
        }
        let tokens: Vec<Address> = Self::get_tokens(
            e.clone(),
            token_a.clone(),
            token_b.clone(),
            pool_index.clone(),
        );

        let amounts: Vec<i128> = e.invoke_contract(
            &pool_id,
            &symbol_short!("withdraw"),
            Vec::from_array(
                &e,
                [
                    user.clone().into_val(&e),
                    share_amount.into_val(&e),
                    min_amounts.into_val(&e),
                ],
            ),
        );

        e.events().publish(
            (Symbol::new(&e, "withdraw"), token_a, token_b, user),
            (pool_id, share_amount, amounts.clone()),
        );

        amounts
    }
}

#[contractimpl]
impl UpgradeableContract for LiquidityPoolRouter {
    fn version() -> u32 {
        7
    }

    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        require_admin(&e);
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}

#[contractimpl]
impl AdminInterface for LiquidityPoolRouter {
    fn init_admin(e: Env, account: Address) {
        if !has_admin(&e) {
            set_admin(&e, &account)
        }
    }

    fn set_token_hash(e: Env, new_hash: BytesN<32>) {
        require_admin(&e);
        storage::set_token_hash(&e, &new_hash);
    }

    fn set_pool_hash(e: Env, new_hash: BytesN<32>) {
        require_admin(&e);
        storage::set_constant_product_pool_hash(&e, &new_hash);
    }

    fn set_reward_token(e: Env, reward_token: Address) {
        require_admin(&e);
        storage::set_reward_token(&e, &reward_token);
    }
}

#[contractimpl]
impl PoolsManagementTrait for LiquidityPoolRouter {
    fn init_pool(e: Env, token_a: Address, token_b: Address) -> (BytesN<32>, Address) {
        let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
        let pools = storage::get_pools(&e, &salt);
        if pools.is_empty() {
            pool_utils::deploy_standard_pool(&e, &token_a, &token_b, 30)
        } else {
            let pool_hash = pools.keys().first().unwrap();
            (pool_hash.clone(), pools.get(pool_hash).unwrap())
        }
    }

    // fn init_standard_pool(
    //     e: Env,
    //     token_a: Address,
    //     token_b: Address,
    //     fee_fraction: u32,
    // ) -> Address {
    //     let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
    //     if !has_pool(&e, &salt) {
    //         pool_utils::deploy_standard_pool(&e, &token_a, &token_b, PoolType::Standard, fee_fraction);
    //     }
    //     let (_pool_exists, pool_id) = Self::get_pool(e.clone(), token_a, token_b);
    //     pool_id
    // }
    //
    // fn init_stableswap_pool(
    //     e: Env,
    //     token_a: Address,
    //     token_b: Address,
    //     a: u128,
    //     fee_fraction: u32,
    //     admin_fee: u32,
    // ) -> Address {
    //     let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
    //     if !has_pool(&e, &salt) {
    //         pool_utils::deploy_stableswap_pool(&e, &token_a, &token_b, PoolType::StableSwap, a, fee_fraction, admin_fee);
    //     }
    //     let (_pool_exists, pool_id) = Self::get_pool(e.clone(), token_a, token_b);
    //     pool_id
    // }
    //
    fn get_pools(e: Env, token_a: Address, token_b: Address) -> Map<BytesN<32>, Address> {
        let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
        storage::get_pools(&e, &salt)
    }

    // fn add_pool(e: Env, token_a: Address, token_b: Address, pool_address: Address, pool_description: Symbol) {
    //     todo!()
    // }
    //
    // fn remove_pool(e: Env, token_a: Address, token_b: Address, pool_hash: BytesN<32>) {
    //     todo!()
    // }
}
