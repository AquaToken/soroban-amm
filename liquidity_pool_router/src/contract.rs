use crate::constants::{CONSTANT_PRODUCT_FEE_AVAILABLE, STABLESWAP_MAX_FEE};
use crate::errors::LiquidityPoolRouterError;
use crate::events::{Events, LiquidityPoolRouterEvents};
use crate::liquidity_calculator::LiquidityCalculatorClient;
use crate::pool_interface::{
    CombinedSwapInterface, LiquidityPoolInterfaceTrait, PoolPlaneInterface, PoolsManagementTrait,
    RewardsInterfaceTrait, SwapRouterInterface,
};
use crate::pool_utils::{
    deploy_stableswap_pool, deploy_standard_pool, get_stableswap_pool_salt, get_standard_pool_salt,
    get_tokens_salt, get_total_liquidity,
};
use crate::rewards::get_rewards_manager;
use crate::router_interface::{AdminInterface, UpgradeableContract};
use crate::storage::{
    get_init_pool_payment_address, get_init_pool_payment_amount, get_init_pool_payment_token,
    get_liquidity_calculator, get_pool, get_pool_plane, get_pools_plain, get_reward_tokens,
    get_reward_tokens_detailed, get_rewards_config, get_swap_router, get_tokens_set,
    get_tokens_set_count, has_pool, remove_pool, set_constant_product_pool_hash,
    set_init_pool_payment_address, set_init_pool_payment_amount, set_init_pool_payment_token,
    set_liquidity_calculator, set_pool_plane, set_reward_tokens, set_reward_tokens_detailed,
    set_rewards_config, set_stableswap_pool_hash, set_swap_router, set_token_hash,
    GlobalRewardsConfig, LiquidityPoolRewardInfo,
};
use crate::swap_router::SwapRouterClient;
use access_control::access::{AccessControl, AccessControlTrait};
use access_control::errors::AccessControlError;
use liquidity_pool_validation_errors::LiquidityPoolValidationError;
use rewards::storage::RewardsStorageTrait;
use soroban_sdk::auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation};
use soroban_sdk::token::Client as SorobanTokenClient;
use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, vec, Address, BytesN, Env, IntoVal,
    Map, Symbol, Val, Vec, U256,
};
use utils::storage_errors::StorageError;
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
        let pool_id = match get_pool(&e, tokens, pool_index) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let calculator = get_liquidity_calculator(&e);
        match LiquidityCalculatorClient::new(&e, &calculator)
            .get_liquidity(&Vec::from_array(&e, [pool_id]))
            .get(0)
        {
            Some(v) => v,
            None => panic_with_error!(&e, LiquidityPoolRouterError::LiquidityCalculationError),
        }
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
        103
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

    fn set_stableswap_pool_hash(e: Env, new_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_stableswap_pool_hash(&e, &new_hash);
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
    fn get_rewards_config(e: Env) -> Map<Symbol, i128> {
        let rewards_config = get_rewards_config(&e);
        let mut result = Map::new(&e);
        result.set(symbol_short!("tps"), rewards_config.tps as i128);
        result.set(symbol_short!("exp_at"), rewards_config.expired_at as i128);
        result.set(symbol_short!("block"), rewards_config.current_block as i128);
        result
    }

    fn get_tokens_for_reward(e: Env) -> Map<Vec<Address>, (u32, bool, U256)> {
        let rewards_config = get_rewards_config(&e);
        let tokens = get_reward_tokens(&e, rewards_config.current_block);
        let mut result = Map::new(&e);
        for (key, value) in tokens {
            result.set(
                key,
                (value.voting_share, value.processed, value.total_liquidity),
            );
        }
        result
    }

    fn get_total_liquidity(e: Env, tokens: Vec<Address>) -> U256 {
        if !check_vec_ordered(&tokens) {
            panic_with_error!(e, LiquidityPoolValidationError::TokensNotSorted);
        }
        let tokens_salt = get_tokens_salt(&e, tokens.clone());
        let pools = get_pools_plain(&e, &tokens_salt);

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
        tokens_votes: Vec<(Vec<Address>, u32)>, // {[token1, token2]: voting_percentage}, voting percentage 0_0000000 .. 1_0000000
    ) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        let rewards_config = get_rewards_config(&e);
        let new_rewards_block = rewards_config.current_block + 1;

        let mut tokens_with_liquidity = Map::new(&e);
        for (tokens, voting_share) in tokens_votes {
            // since we expect tokens to be sorted, we can safely compare neighbors
            for i in 0..tokens.len() - 1 {
                if tokens.get_unchecked(i) == tokens.get_unchecked(i + 1) {
                    panic_with_error!(&e, LiquidityPoolRouterError::DuplicatesNotAllowed);
                }
            }

            if !check_vec_ordered(&tokens) {
                panic_with_error!(&e, LiquidityPoolValidationError::TokensNotSorted);
            }

            tokens_with_liquidity.set(
                tokens,
                LiquidityPoolRewardInfo {
                    voting_share,
                    processed: false,
                    total_liquidity: U256::from_u32(&e, 0),
                },
            );
        }
        let mut sum = 0;
        for (_, reward_info) in tokens_with_liquidity.iter() {
            sum += reward_info.voting_share;
        }
        if sum > 1_0000000 {
            panic_with_error!(e, LiquidityPoolRouterError::VotingShareExceedsMax);
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

    fn fill_liquidity(e: Env, tokens: Vec<Address>) {
        let rewards_config = get_rewards_config(&e);
        let tokens_salt = get_tokens_salt(&e, tokens.clone());
        let calculator = get_liquidity_calculator(&e);
        let (pools, total_liquidity) = get_total_liquidity(&e, tokens.clone(), calculator);

        let mut pools_with_processed_info = Map::new(&e);
        for (key, value) in pools {
            pools_with_processed_info.set(key, (value, false));
        }

        let mut tokens_with_liquidity = get_reward_tokens(&e, rewards_config.current_block);
        let mut token_data = match tokens_with_liquidity.get(tokens.clone()) {
            Some(v) => v,
            None => panic_with_error!(e, LiquidityPoolRouterError::TokensAreNotForReward),
        };
        if token_data.processed {
            panic_with_error!(e, LiquidityPoolRouterError::LiquidityAlreadyFilled);
        }
        token_data.processed = true;
        token_data.total_liquidity = total_liquidity;
        tokens_with_liquidity.set(tokens, token_data);
        set_reward_tokens(&e, rewards_config.current_block, &tokens_with_liquidity);
        set_reward_tokens_detailed(
            &e,
            rewards_config.current_block,
            tokens_salt,
            &pools_with_processed_info,
        );
    }

    fn config_pool_rewards(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let rewards_config = get_rewards_config(&e);
        let tokens_salt = get_tokens_salt(&e, tokens.clone());
        let mut tokens_detailed =
            get_reward_tokens_detailed(&e, rewards_config.current_block, tokens_salt.clone());
        let tokens_reward = get_reward_tokens(&e, rewards_config.current_block);
        let tokens_reward_info = tokens_reward.get(tokens.clone());

        let (pool_liquidity, pool_configured) = if tokens_reward_info.is_some() {
            tokens_detailed
                .get(pool_index.clone())
                .unwrap_or((U256::from_u32(&e, 0), false))
        } else {
            (U256::from_u32(&e, 0), false)
        };

        if pool_configured {
            panic_with_error!(&e, LiquidityPoolRouterError::RewardsAlreadyConfigured);
        }

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
            panic_with_error!(&e, LiquidityPoolRouterError::LiquidityNotFilled);
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
                    e.current_contract_address().to_val(),
                    rewards_config.expired_at.into_val(&e),
                    pool_tps.into_val(&e),
                ],
            ),
        );

        if pool_tps > 0 {
            // mark pool as configured to avoid reentrancy
            tokens_detailed.set(pool_index, (pool_liquidity, true));
            set_reward_tokens_detailed(
                &e,
                rewards_config.current_block,
                tokens_salt,
                &tokens_detailed,
            );
        }

        Events::new(&e).config_rewards(tokens, pool_id, pool_tps, rewards_config.expired_at);

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

    fn get_total_accumulated_reward(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_total_accumulated_reward"),
            Vec::new(&e),
        )
    }

    fn get_total_configured_reward(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_total_configured_reward"),
            Vec::new(&e),
        )
    }

    fn get_total_claimed_reward(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_total_claimed_reward"),
            Vec::new(&e),
        )
    }

    fn claim(e: Env, user: Address, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        user.require_auth();

        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let amount = e.invoke_contract(
            &pool_id,
            &symbol_short!("claim"),
            Vec::from_array(&e, [user.clone().into_val(&e)]),
        );

        Events::new(&e).claim(
            tokens,
            user,
            pool_id,
            get_rewards_manager(&e).storage().get_reward_token(),
            amount,
        );

        amount
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
        if fee_fraction > STABLESWAP_MAX_FEE {
            panic_with_error!(&e, LiquidityPoolRouterError::BadFee);
        }

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

    fn get_tokens_sets_count(e: Env) -> u128 {
        get_tokens_set_count(&e)
    }

    fn get_tokens(e: Env, index: u128) -> Vec<Address> {
        get_tokens_set(&e, index)
    }

    fn get_pools_for_tokens_range(
        e: Env,
        start: u128,
        end: u128,
    ) -> Vec<(Vec<Address>, Map<BytesN<32>, Address>)> {
        // chained operation for better efficiency
        let mut result = Vec::new(&e);
        for index in start..end {
            let tokens = Self::get_tokens(e.clone(), index);
            result.push_back((tokens.clone(), Self::get_pools(e.clone(), tokens)))
        }
        result
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

#[contractimpl]
impl CombinedSwapInterface for LiquidityPoolRouter {
    fn swap_chained(
        e: Env,
        user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address,
        in_amount: u128,
        out_min: u128,
    ) -> u128 {
        user.require_auth();
        let mut last_token_out: Option<Address> = None;
        let mut last_swap_result = 0;

        if swaps_chain.len() == 0 {
            panic_with_error!(&e, LiquidityPoolRouterError::PathIsEmpty);
        }

        SorobanTokenClient::new(&e, &token_in).transfer(
            &user,
            &e.current_contract_address(),
            &(in_amount as i128),
        );

        for i in 0..swaps_chain.len() {
            let (tokens, pool_index, token_out) = swaps_chain.get(i).unwrap();
            if !check_vec_ordered(&tokens) {
                panic_with_error!(&e, LiquidityPoolValidationError::TokensNotSorted);
            }

            let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
                Ok(v) => v,
                Err(err) => panic_with_error!(&e, err),
            };

            let mut out_min_local = 0;
            let token_in_local;
            let in_amount_local;
            if i == 0 {
                token_in_local = token_in.clone();
                in_amount_local = in_amount;
            } else {
                token_in_local = match last_token_out {
                    Some(v) => v,
                    None => panic_with_error!(&e, StorageError::ValueNotInitialized),
                };
                in_amount_local = last_swap_result;
            }

            if i == swaps_chain.len() - 1 {
                out_min_local = out_min;
            }

            e.authorize_as_current_contract(vec![
                &e,
                InvokerContractAuthEntry::Contract(SubContractInvocation {
                    context: ContractContext {
                        contract: token_in_local.clone(),
                        fn_name: Symbol::new(&e, "transfer"),
                        args: (
                            e.current_contract_address(),
                            pool_id.clone(),
                            in_amount_local as i128,
                        )
                            .into_val(&e),
                    },
                    sub_invocations: vec![&e],
                }),
            ]);

            last_swap_result = e.invoke_contract(
                &pool_id,
                &symbol_short!("swap"),
                Vec::from_array(
                    &e,
                    [
                        e.current_contract_address().into_val(&e),
                        tokens
                            .first_index_of(token_in_local.clone())
                            .unwrap()
                            .into_val(&e),
                        tokens
                            .first_index_of(token_out.clone())
                            .unwrap()
                            .into_val(&e),
                        in_amount_local.into_val(&e),
                        out_min_local.into_val(&e),
                    ],
                ),
            );

            Events::new(&e).swap(
                tokens,
                user.clone(),
                pool_id,
                token_in_local.clone(),
                token_out.clone(),
                in_amount_local,
                last_swap_result,
            );

            last_token_out = Some(token_out);
        }

        let token_out_address = match last_token_out {
            Some(v) => v,
            None => panic_with_error!(&e, StorageError::ValueNotInitialized),
        };
        SorobanTokenClient::new(&e, &token_out_address).transfer(
            &e.current_contract_address(),
            &user,
            &(last_swap_result as i128),
        );

        last_swap_result
    }
}
