use crate::constants::CONSTANT_PRODUCT_FEE_AVAILABLE;
use crate::errors::LiquidityPoolRouterError;
use crate::events::{Events, LiquidityPoolRouterEvents};
use crate::liquidity_calculator::LiquidityCalculatorClient;
use crate::pool_interface::{
    LiquidityPoolInterfaceTrait, PoolPlaneInterface, PoolsManagementTrait, RewardsInterfaceTrait,
    SwapRouterInterface,
};
use crate::pool_utils::{
    deploy_stableswap_pool, deploy_standard_pool, get_stableswap_pool_salt, get_standard_pool_salt,
    get_tokens_salt, get_total_liquidity, pool_salt,
};
use crate::rewards::get_rewards_manager;
use crate::router_interface::{AdminInterface, UpgradeableContract};
use crate::storage::{
    get_init_pool_payment_address, get_init_pool_payment_amount, get_init_pool_payment_token,
    get_liquidity_calculator, get_pool, get_pool_plane, get_pools_plain, get_reward_tokens,
    get_reward_tokens_detailed, get_rewards_config, get_swap_router, has_pool, remove_pool,
    set_constant_product_pool_hash, set_init_pool_payment_address, set_init_pool_payment_amount,
    set_init_pool_payment_token, set_liquidity_calculator, set_pool_plane, set_reward_tokens,
    set_reward_tokens_detailed, set_rewards_config, set_stableswap_pool_hash, set_swap_router,
    set_token_hash, GlobalRewardsConfig, LiquidityPoolRewardInfo,
};
use crate::swap_router::SwapRouterClient;
use access_control::access::{AccessControl, AccessControlTrait};
use access_control::errors::AccessControlError;
use liquidity_pool_validation_errors::LiquidityPoolValidationError;
use rewards::storage::RewardsStorageTrait;
use soroban_sdk::token::Client as SorobanTokenClient;
use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, Address, BytesN, Env, IntoVal, Map,
    Symbol, Val, Vec, U256,
};
use utils::token_utils::check_vec_ordered;

#[contract]
pub struct LiquidityPoolRouter;

#[contractimpl]
impl LiquidityPoolInterfaceTrait for LiquidityPoolRouter {
    fn pool_type(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Symbol {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        e.invoke_contract(&pool_id, &Symbol::new(&e, "pool_type"), Vec::new(&e))
    }

    fn get_info(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Map<Symbol, Val> {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_info"), Vec::new(&e))
    }

    fn get_pool(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address {
        match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        }
    }

    fn share_id(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        e.invoke_contract(&pool_id, &Symbol::new(&e, "share_id"), Vec::new(&e))
    }

    fn get_total_shares(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_total_shares"), Vec::new(&e))
    }

    fn get_reserves(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Vec<u128> {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_reserves"), Vec::new(&e))
    }

    fn get_tokens(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Vec<Address> {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_tokens"), Vec::new(&e))
    }

    fn deposit(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
        desired_amounts: Vec<u128>,
        min_shares: u128,
    ) -> (Vec<u128>, u128) {
        user.require_auth();

        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let (amounts, share_amount): (Vec<u128>, u128) = e.invoke_contract(
            &pool_id,
            &symbol_short!("deposit"),
            Vec::from_array(
                &e,
                [
                    user.clone().into_val(&e),
                    desired_amounts.into_val(&e),
                    min_shares.into_val(&e),
                ],
            ),
        );
        Events::new(&e).deposit(tokens, user, pool_id, amounts.clone(), share_amount);
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
        if !check_vec_ordered(&tokens) {
            panic_with_error!(&e, LiquidityPoolValidationError::TokensNotSorted);
        }
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
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

        Events::new(&e).swap(
            tokens, user, pool_id, token_in, token_out, in_amount, out_amt,
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
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
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

        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

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

        Events::new(&e).withdraw(tokens, user, pool_id, amounts.clone(), share_amount);
        amounts
    }

    fn get_liquidity(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> U256 {
        let pool_id = get_pool(&e, tokens, pool_index).expect("Pool doesn't exist");

        let calculator = get_liquidity_calculator(&e);
        LiquidityCalculatorClient::new(&e, &calculator)
            .get_liquidity(&Vec::from_array(&e, [pool_id]))
            .get(0)
            .expect("unable to get liquidity for the pool")
    }

    fn get_liquidity_calculator(e: Env) -> Address {
        get_liquidity_calculator(&e)
    }

    fn set_liquidity_calculator(e: Env, admin: Address, calculator: Address) {
        let access_control = AccessControl::new(&e);
        admin.require_auth();
        access_control.check_admin(&admin);

        set_liquidity_calculator(&e, &calculator);
    }
}

#[contractimpl]
impl UpgradeableContract for LiquidityPoolRouter {
    fn version() -> u32 {
        100
    }

    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}

#[contractimpl]
impl AdminInterface for LiquidityPoolRouter {
    fn init_admin(e: Env, account: Address) {
        let access_control = AccessControl::new(&e);
        if access_control.has_admin() {
            panic_with_error!(&e, AccessControlError::AdminAlreadySet);
        }
        access_control.set_admin(&account);
    }

    fn set_token_hash(e: Env, new_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_token_hash(&e, &new_hash);
    }

    fn set_pool_hash(e: Env, new_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_constant_product_pool_hash(&e, &new_hash);
    }

    fn set_stableswap_pool_hash(e: Env, num_tokens: u32, new_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_stableswap_pool_hash(&e, num_tokens, &new_hash);
    }

    fn configure_init_pool_payment(e: Env, token: Address, amount: u128, to: Address) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_init_pool_payment_token(&e, &token);
        set_init_pool_payment_amount(&e, &amount);
        set_init_pool_payment_address(&e, &to);
    }

    fn set_reward_token(e: Env, reward_token: Address) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        let rewards = get_rewards_manager(&e);
        rewards.storage().put_reward_token(reward_token);
    }
}

#[contractimpl]
impl RewardsInterfaceTrait for LiquidityPoolRouter {
    fn get_rewards_config(e: Env) -> Map<Symbol, Val> {
        let rewards_config = get_rewards_config(&e);
        let mut result = Map::new(&e);
        result.set(Symbol::new(&e, "tps"), rewards_config.tps.into_val(&e));
        result.set(
            Symbol::new(&e, "expired_at"),
            rewards_config.expired_at.into_val(&e),
        );
        result.set(
            Symbol::new(&e, "current_block"),
            rewards_config.current_block.into_val(&e),
        );
        result
    }

    fn get_total_liquidity(e: Env, tokens: Vec<Address>) -> U256 {
        if !check_vec_ordered(&tokens) {
            panic!("tokens are not sorted")
        }
        let salt = pool_salt(&e, tokens.clone());
        let pools = get_pools_plain(&e, &salt);

        let calculator = get_liquidity_calculator(&e);
        let mut pools_vec: Vec<Address> = Vec::new(&e);
        for (_key, value) in pools {
            pools_vec.push_back(value.clone());
        }

        let pools_liquidity =
            LiquidityCalculatorClient::new(&e, &calculator).get_liquidity(&pools_vec);
        let mut result = U256::from_u32(&e, 0);
        for liquidity in pools_liquidity {
            result = result.add(&liquidity);
        }
        result
    }

    fn config_global_rewards(
        e: Env,
        admin: Address,
        reward_tps: u128, // value with 7 decimal places. example: 600_0000000
        expired_at: u64,  // timestamp
        tokens: Vec<(Vec<Address>, u32)>, // {[token1, token2]: voting_percentage}, voting percentage 0_0000000 .. 1_0000000
    ) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.require_admin();

        let rewards_config = get_rewards_config(&e);
        let new_rewards_block = rewards_config.current_block + 1;

        let mut tokens_with_liquidity = Map::new(&e);
        for token in tokens {
            if !check_vec_ordered(&token.0) {
                panic!("tokens are not sorted")
            }

            tokens_with_liquidity.set(
                token.0,
                LiquidityPoolRewardInfo {
                    voting_share: token.1,
                    processed: false,
                    total_liquidity: U256::from_u32(&e, 0),
                },
            );
        }
        let mut sum = 0;
        for token in tokens_with_liquidity.iter() {
            sum += token.1.voting_share;
        }
        if sum > 1_0000000 {
            panic!("total voting share exceeds 100%")
        }

        set_reward_tokens(&e, new_rewards_block, &tokens_with_liquidity);
        set_rewards_config(
            &e,
            &GlobalRewardsConfig {
                tps: reward_tps,
                expired_at,
                current_block: new_rewards_block,
            },
        )
    }

    // todo: deactivate no more active tokens

    fn fill_liquidity(e: Env, admin: Address, tokens: Vec<Address>) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.require_admin();

        let rewards_config = get_rewards_config(&e);
        let salt = pool_salt(&e, tokens.clone());
        let calculator = get_liquidity_calculator(&e);
        let (pools, total_liquidity) = get_total_liquidity(&e, tokens.clone(), calculator);
        let mut tokens_with_liquidity = get_reward_tokens(&e, rewards_config.current_block);
        let mut token_data = tokens_with_liquidity
            .get(tokens.clone())
            .expect("unable to find tokens in reward map");
        token_data.processed = true;
        token_data.total_liquidity = total_liquidity;
        tokens_with_liquidity.set(tokens, token_data);
        set_reward_tokens(&e, rewards_config.current_block, &tokens_with_liquidity);
        set_reward_tokens_detailed(&e, rewards_config.current_block, salt, &pools);
    }

    fn config_pool_rewards(
        e: Env,
        admin: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
    ) -> u128 {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.require_admin();

        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let rewards_config = get_rewards_config(&e);
        let salt = pool_salt(&e, tokens.clone());
        let tokens_detailed = get_reward_tokens_detailed(&e, rewards_config.current_block, salt);
        let tokens_reward = get_reward_tokens(&e, rewards_config.current_block);
        let tokens_reward_info = tokens_reward.get(tokens.clone());

        let pool_liquidity = if tokens_reward_info.is_some() {
            tokens_detailed
                .get(pool_index)
                .unwrap_or(U256::from_u32(&e, 0))
        } else {
            U256::from_u32(&e, 0)
        };

        let reward_info = match tokens_reward_info {
            Some(v) => v,
            // if tokens not found in current config, deactivate them
            None => LiquidityPoolRewardInfo {
                voting_share: 0,
                processed: true,
                total_liquidity: U256::from_u32(&e, 0),
            },
        };

        if !reward_info.processed {
            panic!("liquidity info not available yet. run `fill_liquidity` first")
        }
        // it's safe to convert tps to u128 since it cannot be bigger than total tps which is u128
        let pool_tps = if pool_liquidity > U256::from_u32(&e, 0) {
            U256::from_u128(&e, rewards_config.tps)
                .mul(&U256::from_u32(&e, reward_info.voting_share))
                .mul(&pool_liquidity)
                .div(&reward_info.total_liquidity)
                .div(&U256::from_u32(&e, 1_0000000))
                .to_u128()
                .unwrap()
        } else {
            0
        };

        e.invoke_contract::<Val>(
            &pool_id,
            &Symbol::new(&e, "set_rewards_config"),
            Vec::from_array(
                &e,
                [
                    admin.into_val(&e),
                    rewards_config.expired_at.into_val(&e),
                    pool_tps.into_val(&e),
                ],
            ),
        );

        pool_tps
    }

    fn get_rewards_info(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
    ) -> Map<Symbol, i128> {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

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
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_user_reward"),
            Vec::from_array(&e, [user.clone().into_val(&e)]),
        )
    }

    fn claim(e: Env, user: Address, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        user.require_auth();

        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

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
        let salt = get_tokens_salt(&e, tokens.clone());
        let pools = get_pools_plain(&e, &salt);
        if pools.is_empty() {
            deploy_standard_pool(&e, tokens, 30)
        } else {
            let pool_hash = pools.keys().first().unwrap();
            (pool_hash.clone(), pools.get(pool_hash).unwrap())
        }
    }

    fn init_standard_pool(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        fee_fraction: u32,
    ) -> (BytesN<32>, Address) {
        user.require_auth();
        if !CONSTANT_PRODUCT_FEE_AVAILABLE.contains(&fee_fraction) {
            panic_with_error!(&e, LiquidityPoolRouterError::BadFee);
        }

        let salt = get_tokens_salt(&e, tokens.clone());
        let pools = get_pools_plain(&e, &salt);
        let pool_index = get_standard_pool_salt(&e, &fee_fraction);

        match pools.get(pool_index.clone()) {
            Some(pool_address) => (pool_index, pool_address),
            None => deploy_standard_pool(&e, tokens, fee_fraction),
        }
    }

    fn init_stableswap_pool(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        a: u128,
        fee_fraction: u32,
        admin_fee: u32,
    ) -> (BytesN<32>, Address) {
        user.require_auth();

        // pay for pool creation
        let init_pool_token = get_init_pool_payment_token(&e);
        let init_pool_amount = get_init_pool_payment_amount(&e);
        let init_pool_address = get_init_pool_payment_address(&e);
        SorobanTokenClient::new(&e, &init_pool_token).transfer(
            &user,
            &init_pool_address,
            &(init_pool_amount as i128),
        );

        let salt = get_tokens_salt(&e, tokens.clone());
        let pools = get_pools_plain(&e, &salt);
        let pool_index = get_stableswap_pool_salt(&e);

        match pools.get(pool_index.clone()) {
            Some(pool_address) => (pool_index, pool_address),
            None => deploy_stableswap_pool(&e, tokens, a, fee_fraction, admin_fee),
        }
    }

    fn get_pools(e: Env, tokens: Vec<Address>) -> Map<BytesN<32>, Address> {
        let salt = get_tokens_salt(&e, tokens);
        get_pools_plain(&e, &salt)
    }

    fn remove_pool(e: Env, user: Address, tokens: Vec<Address>, pool_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        user.require_auth();
        access_control.check_admin(&user);
        let salt = get_tokens_salt(&e, tokens.clone());
        if has_pool(&e, &salt, pool_hash.clone()) {
            remove_pool(&e, &salt, pool_hash)
        }
    }
}

#[contractimpl]
impl PoolPlaneInterface for LiquidityPoolRouter {
    fn set_pools_plane(e: Env, admin: Address, plane: Address) {
        let access_control = AccessControl::new(&e);
        admin.require_auth();
        access_control.check_admin(&admin);

        set_pool_plane(&e, &plane);
    }

    fn get_plane(e: Env) -> Address {
        get_pool_plane(&e)
    }
}

#[contractimpl]
impl SwapRouterInterface for LiquidityPoolRouter {
    fn set_swap_router(e: Env, admin: Address, router: Address) {
        let access_control = AccessControl::new(&e);
        admin.require_auth();
        access_control.check_admin(&admin);
        set_swap_router(&e, &router);
    }

    fn get_swap_router(e: Env) -> Address {
        get_swap_router(&e)
    }

    fn estimate_swap_routed(
        e: Env,
        tokens: Vec<Address>,
        token_in: Address,
        token_out: Address,
        in_amount: u128,
    ) -> (BytesN<32>, Address, u128) {
        let salt = get_tokens_salt(&e, tokens.clone());
        let pools = get_pools_plain(&e, &salt);

        let swap_router = get_swap_router(&e);
        let mut pools_vec: Vec<Address> = Vec::new(&e);
        let mut pools_reversed: Map<Address, BytesN<32>> = Map::new(&e);
        for (key, value) in pools {
            pools_vec.push_back(value.clone());
            pools_reversed.set(value, key);
        }

        let (best_pool_address, swap_result) = SwapRouterClient::new(&e, &swap_router)
            .estimate_swap(
                &pools_vec,
                &(tokens.first_index_of(token_in).unwrap()),
                &(tokens.first_index_of(token_out).unwrap()),
                &in_amount,
            );

        (
            match pools_reversed.get(best_pool_address.clone()) {
                Some(v) => v,
                None => panic_with_error!(e, LiquidityPoolRouterError::PoolNotFound),
            },
            best_pool_address,
            swap_result,
        )
    }
}
