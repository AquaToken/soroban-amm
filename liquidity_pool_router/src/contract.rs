use crate::constants::CONSTANT_PRODUCT_FEE_AVAILABLE;
use crate::events::{Events, LiquidityPoolRouterEvents};
use crate::pool_interface::{
    LiquidityPoolInterfaceTrait, PoolsManagementTrait, RewardsInterfaceTrait,
};
use crate::pool_utils::{
    deploy_stableswap_pool, deploy_standard_pool, get_custom_salt, get_stableswap_pool_salt,
    get_standard_pool_salt, get_total_liquidity, pool_salt,
};
use crate::rewards::get_rewards_manager;
use crate::router_interface::{AdminInterface, UpgradeableContract};
use crate::storage::{
    add_pool, get_init_pool_payment_amount, get_init_pool_payment_token, get_pool, get_pools_plain,
    get_reward_tokens, get_reward_tokens_detailed, get_rewards_config, has_pool, remove_pool,
    set_constant_product_pool_hash, set_init_pool_payment_amount, set_init_pool_payment_token,
    set_reward_tokens, set_reward_tokens_detailed, set_rewards_config, set_stableswap_pool_hash,
    set_token_hash, GlobalRewardsConfig, LiquidityPoolRewardInfo, LiquidityPoolType,
};
use access_control::access::{AccessControl, AccessControlTrait};
use rewards::storage::RewardsStorageTrait;
use soroban_sdk::token::Client as SorobanTokenClient;
use soroban_sdk::{
    contract, contractimpl, symbol_short, Address, BytesN, Env, IntoVal, Map, Symbol, Val, Vec,
};
use utils::utils::check_vec_ordered;

#[contract]
pub struct LiquidityPoolRouter;

#[contractimpl]
impl LiquidityPoolInterfaceTrait for LiquidityPoolRouter {
    fn pool_type(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Symbol {
        let pool_id = get_pool(&e, tokens, pool_index).expect("Pool doesn't exist");
        e.invoke_contract(&pool_id, &Symbol::new(&e, "pool_type"), Vec::new(&e))
    }

    fn get_info(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Map<Symbol, Val> {
        let pool_id = get_pool(&e, tokens, pool_index).expect("Pool doesn't exist");
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_info"), Vec::new(&e))
    }

    fn get_pool(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address {
        get_pool(&e, tokens, pool_index).expect("Error when get pool")
    }

    fn share_id(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address {
        let pool_id = get_pool(&e, tokens, pool_index).expect("Pool doesn't exist");
        e.invoke_contract(&pool_id, &Symbol::new(&e, "share_id"), Vec::new(&e))
    }

    fn get_reserves(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Vec<u128> {
        let pool_id = get_pool(&e, tokens, pool_index).expect("Pool doesn't exist");
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_reserves"), Vec::new(&e))
    }

    fn get_tokens(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Vec<Address> {
        let pool_id = get_pool(&e, tokens, pool_index).expect("Pool doesn't exist");
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

        let pool_id = get_pool(&e, tokens.clone(), pool_index).expect("unable to find pool");

        let (amounts, share_amount): (Vec<u128>, u128) = e.invoke_contract(
            &pool_id,
            &symbol_short!("deposit"),
            Vec::from_array(
                &e,
                [user.clone().into_val(&e), desired_amounts.into_val(&e)],
            ),
        );
        Events::new(&e).deposit(tokens, user, pool_id, amounts.clone(), share_amount.clone());
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
            panic!("tokens are not sorted")
        }
        let pool_id = get_pool(&e, tokens.clone(), pool_index.clone()).expect("Pool doesn't exist");
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
            tokens,
            user,
            pool_id,
            token_in,
            token_out,
            in_amount.clone(),
            out_amt,
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
        let pool_id = get_pool(&e, tokens.clone(), pool_index.clone()).expect("Pool doesn't exist");
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
        let pool_id = get_pool(&e, tokens.clone(), pool_index.clone()).expect("Pool doesn't exist");

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

    fn get_liquidity(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        let pool_id = get_pool(&e, tokens, pool_index).expect("Pool doesn't exist");
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_liquidity"), Vec::new(&e))
    }
}

#[contractimpl]
impl UpgradeableContract for LiquidityPoolRouter {
    fn version() -> u32 {
        7
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
        if !access_control.has_admin() {
            access_control.set_admin(&account)
        }
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

    fn configure_init_pool_payment(e: Env, token: Address, amount: u128) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_init_pool_payment_token(&e, &token);
        set_init_pool_payment_amount(&e, &amount);
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

    fn get_total_liquidity(e: Env, tokens: Vec<Address>) -> u128 {
        let salt = pool_salt(&e, tokens);
        let mut result = 0;
        for (_hash, pool_id) in get_pools_plain(&e, &salt) {
            result += e.invoke_contract::<u128>(
                &pool_id,
                &Symbol::new(&e, "get_liquidity"),
                Vec::new(&e),
            );
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
                    total_liquidity: 0,
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
        let (pools, total_liquidity) = get_total_liquidity(&e, tokens.clone());
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

        let pool_id = get_pool(&e, tokens.clone(), pool_index.clone()).expect("Pool doesn't exist");

        let rewards_config = get_rewards_config(&e);
        let salt = pool_salt(&e, tokens.clone());
        let tokens_detailed = get_reward_tokens_detailed(&e, rewards_config.current_block, salt);
        let tokens_reward = get_reward_tokens(&e, rewards_config.current_block);
        let tokens_reward_info = tokens_reward.get(tokens.clone());

        let pool_liquidity = if tokens_reward_info.is_some() {
            tokens_detailed.get(pool_index).unwrap_or(0)
        } else {
            0
        };

        let reward_info = match tokens_reward_info {
            Some(v) => v,
            // if tokens not found in current config, deactivate them
            None => LiquidityPoolRewardInfo {
                voting_share: 0,
                processed: true,
                total_liquidity: 0,
            },
        };

        if !reward_info.processed {
            panic!("liquidity info not available yet. run `fill_liquidity` first")
        }
        let pool_tps = if pool_liquidity > 0 {
            rewards_config.tps * reward_info.voting_share as u128 * pool_liquidity
                / reward_info.total_liquidity
                / 1_0000000
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
        let pool_id = get_pool(&e, tokens, pool_index.clone()).expect("Pool doesn't exist");

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
        let pool_id = get_pool(&e, tokens, pool_index.clone()).expect("Pool doesn't exist");

        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_user_reward"),
            Vec::from_array(&e, [user.clone().into_val(&e)]),
        )
    }

    fn claim(e: Env, user: Address, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        user.require_auth();
        let pool_id = get_pool(&e, tokens, pool_index.clone()).expect("Pool doesn't exist");

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
        let salt = pool_salt(&e, tokens.clone());
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
            panic!("non-standard fee");
        }

        let salt = pool_salt(&e, tokens.clone());
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
        SorobanTokenClient::new(&e, &init_pool_token).transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &(init_pool_amount as i128),
        );

        let salt = pool_salt(&e, tokens.clone());
        let pools = get_pools_plain(&e, &salt);
        let pool_index = get_stableswap_pool_salt(&e);

        match pools.get(pool_index.clone()) {
            Some(pool_address) => (pool_index, pool_address),
            None => deploy_stableswap_pool(&e, tokens, a, fee_fraction, admin_fee),
        }
    }

    fn get_pools(e: Env, tokens: Vec<Address>) -> Map<BytesN<32>, Address> {
        let salt = pool_salt(&e, tokens);
        get_pools_plain(&e, &salt)
    }

    fn add_custom_pool(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_address: Address,
        pool_type: Symbol,
        init_args: Vec<Val>,
    ) -> BytesN<32> {
        let access_control = AccessControl::new(&e);
        user.require_auth();
        access_control.check_admin(&user);
        let salt = pool_salt(&e, tokens.clone());
        let subpool_salt = get_custom_salt(&e, &pool_type, &init_args);

        if has_pool(&e, &salt, subpool_salt.clone()) {
            panic!("pool already exists")
        }

        add_pool(
            &e,
            &salt,
            subpool_salt.clone(),
            LiquidityPoolType::Custom,
            pool_address.clone(),
        );

        Events::new(&e).add_pool(
            tokens,
            pool_address,
            pool_type,
            subpool_salt.clone(),
            init_args,
        );
        subpool_salt
    }

    fn remove_pool(e: Env, user: Address, tokens: Vec<Address>, pool_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        user.require_auth();
        access_control.check_admin(&user);
        let salt = pool_salt(&e, tokens.clone());
        if has_pool(&e, &salt, pool_hash.clone()) {
            remove_pool(&e, &salt, pool_hash)
        }
    }
}
