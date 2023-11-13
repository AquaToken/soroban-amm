use crate::admin::{has_admin, require_admin, set_admin};
use crate::constants::CONSTANT_PRODUCT_FEE_AVAILABLE;
use crate::pool_interface::{
    LiquidityPoolInterfaceTrait, PoolsManagementTrait, RewardsInterfaceTrait,
};
use crate::router_interface::{AdminInterface, UpgradeableContract};
use crate::{pool_utils, storage};
use soroban_sdk::{contract, contractimpl, Address, BytesN, Env, Val};
use soroban_sdk::{symbol_short, IntoVal, Map, Symbol, Vec};

#[contract]
pub struct LiquidityPoolRouter;

#[contractimpl]
impl LiquidityPoolInterfaceTrait for LiquidityPoolRouter {
    fn pool_type(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Symbol {
        let pool_id = storage::get_pool(&e, tokens, pool_index);
        e.invoke_contract(&pool_id, &Symbol::new(&e, "pool_type"), Vec::new(&e))
    }

    fn get_pool(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> (bool, Address) {
        let salt = crate::utils::pool_salt(&e, tokens);
        (
            storage::has_pool(&e, &salt, pool_index.clone()),
            storage::get_pool_safe(&e, &salt, pool_index),
        )
    }

    fn share_id(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address {
        let pool_id = storage::get_pool(&e, tokens, pool_index);
        e.invoke_contract(&pool_id, &Symbol::new(&e, "share_id"), Vec::new(&e))
    }

    fn get_reserves(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Vec<u128> {
        let pool_id = storage::get_pool(&e, tokens, pool_index);
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_reserves"), Vec::new(&e))
    }

    fn get_tokens(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Vec<Address> {
        let pool_id = storage::get_pool(&e, tokens, pool_index);
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_tokens"), Vec::new(&e))
    }

    fn deposit(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
        desired_amounts: Vec<u128>,
    ) -> (Vec<u128>, u128) {
        user.require_auth();

        let salt = crate::utils::pool_salt(&e, tokens.clone());
        let pool_id = storage::get_pool_safe(&e, &salt, pool_index);

        let (amounts, share_amount): (Vec<u128>, u128) = e.invoke_contract(
            &pool_id,
            &symbol_short!("deposit"),
            Vec::from_array(
                &e,
                [user.clone().into_val(&e), desired_amounts.into_val(&e)],
            ),
        );

        e.events().publish(
            (Symbol::new(&e, "deposit"), tokens, user),
            (pool_id, amounts.clone(), share_amount),
        );

        (amounts, share_amount)
    }

    fn swap(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        token_in: Address,
        token_out: Address,
        pool_index: BytesN<32>,
        in_amount: u128,
        out_min: u128,
    ) -> u128 {
        user.require_auth();

        let pool_id = storage::get_pool(&e, tokens.clone(), pool_index.clone());
        let tokens: Vec<Address> = Self::get_tokens(e.clone(), tokens.clone(), pool_index.clone());

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
            (Symbol::new(&e, "swap"), tokens, user),
            (pool_id, token_in, token_out, in_amount, out_amt),
        );

        out_amt
    }

    fn estimate_swap(
        e: Env,
        tokens: Vec<Address>,
        token_in: Address,
        token_out: Address,
        pool_index: BytesN<32>,
        in_amount: u128,
    ) -> u128 {
        let pool_id = storage::get_pool(&e, tokens.clone(), pool_index.clone());
        let tokens: Vec<Address> = Self::get_tokens(e.clone(), tokens.clone(), pool_index.clone());

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
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
        share_amount: u128,
        min_amounts: Vec<u128>,
    ) -> Vec<u128> {
        user.require_auth();

        let pool_id = storage::get_pool(&e, tokens.clone(), pool_index.clone());

        let amounts: Vec<u128> = e.invoke_contract(
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
            (Symbol::new(&e, "withdraw"), tokens, user),
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

    fn set_stableswap_pool_hash(e: Env, num_tokens: u32, new_hash: BytesN<32>) {
        require_admin(&e);
        storage::set_stableswap_pool_hash(&e, num_tokens, &new_hash);
    }

    fn set_reward_token(e: Env, reward_token: Address) {
        require_admin(&e);
        storage::set_reward_token(&e, &reward_token);
    }
}

#[contractimpl]
impl RewardsInterfaceTrait for LiquidityPoolRouter {
    fn set_rewards_config(
        e: Env,
        admin: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
        expired_at: u64,
        tps: u128,
    ) -> bool {
        admin.require_auth();
        require_admin(&e);

        let pool_id = storage::get_pool(&e, tokens.clone(), pool_index.clone());

        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "set_rewards_config"),
            Vec::from_array(
                &e,
                [
                    admin.into_val(&e),
                    expired_at.into_val(&e),
                    tps.into_val(&e),
                ],
            ),
        )
    }

    fn get_rewards_info(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
    ) -> Map<Symbol, i128> {
        let pool_id = storage::get_pool(&e, tokens.clone(), pool_index.clone());

        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_rewards_info"),
            Vec::from_array(&e, [user.clone().into_val(&e)]),
        )
    }

    fn get_user_reward(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
    ) -> u128 {
        let pool_id = storage::get_pool(&e, tokens.clone(), pool_index.clone());

        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_user_reward"),
            Vec::from_array(&e, [user.clone().into_val(&e)]),
        )
    }

    fn claim(e: Env, user: Address, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        user.require_auth();
        let pool_id = storage::get_pool(&e, tokens.clone(), pool_index.clone());

        e.invoke_contract(
            &pool_id,
            &symbol_short!("claim"),
            Vec::from_array(&e, [user.clone().into_val(&e)]),
        )
    }
}

#[contractimpl]
impl PoolsManagementTrait for LiquidityPoolRouter {
    fn init_pool(e: Env, tokens: Vec<Address>) -> (BytesN<32>, Address) {
        let salt = crate::utils::pool_salt(&e, tokens.clone());
        let pools = storage::get_pools(&e, &salt);
        if pools.is_empty() {
            pool_utils::deploy_standard_pool(&e, tokens, 30)
        } else {
            let pool_hash = pools.keys().first().unwrap();
            (pool_hash.clone(), pools.get(pool_hash).unwrap())
        }
    }

    fn init_standard_pool(
        e: Env,
        tokens: Vec<Address>,
        fee_fraction: u32,
    ) -> (BytesN<32>, Address) {
        if !CONSTANT_PRODUCT_FEE_AVAILABLE.contains(&fee_fraction) {
            panic!("non-standard fee");
        }

        let salt = crate::utils::pool_salt(&e, tokens.clone());
        let pools = storage::get_pools(&e, &salt);
        let pool_index = pool_utils::get_standard_pool_salt(&e, fee_fraction);

        match pools.get(pool_index.clone()) {
            Some(pool_address) => (pool_index, pool_address),
            None => pool_utils::deploy_standard_pool(&e, tokens, fee_fraction),
        }
    }

    fn init_stableswap_pool(
        e: Env,
        tokens: Vec<Address>,
        a: u128,
        fee_fraction: u32,
        admin_fee: u32,
    ) -> (BytesN<32>, Address) {
        require_admin(&e);

        let salt = crate::utils::pool_salt(&e, tokens.clone());
        let pools = storage::get_pools(&e, &salt);
        let pool_index = pool_utils::get_stableswap_pool_salt(&e, a, fee_fraction, admin_fee);

        match pools.get(pool_index.clone()) {
            Some(pool_address) => (pool_index, pool_address),
            None => pool_utils::deploy_stableswap_pool(&e, tokens, a, fee_fraction, admin_fee),
        }
    }

    fn get_pools(e: Env, tokens: Vec<Address>) -> Map<BytesN<32>, Address> {
        let salt = crate::utils::pool_salt(&e, tokens);
        storage::get_pools(&e, &salt)
    }

    fn add_custom_pool(
        e: Env,
        tokens: Vec<Address>,
        pool_address: Address,
        pool_type: Symbol,
        init_args: Vec<Val>,
    ) -> BytesN<32> {
        require_admin(&e);
        let salt = crate::utils::pool_salt(&e, tokens.clone());
        let subpool_salt = pool_utils::get_custom_salt(&e, &pool_type, &init_args);

        if storage::has_pool(&e, &salt, subpool_salt.clone()) {
            panic!("pool already exists")
        }

        storage::add_pool(&e, &salt, subpool_salt.clone(), pool_address.clone());

        e.events().publish(
            (Symbol::new(&e, "add_pool"), tokens),
            (pool_address, pool_type, subpool_salt.clone(), init_args),
        );

        subpool_salt
    }

    fn remove_pool(e: Env, tokens: Vec<Address>, pool_hash: BytesN<32>) {
        require_admin(&e);
        let salt = crate::utils::pool_salt(&e, tokens.clone());
        if storage::has_pool(&e, &salt, pool_hash.clone()) {
            storage::remove_pool(&e, &salt, pool_hash)
        }
    }
}
