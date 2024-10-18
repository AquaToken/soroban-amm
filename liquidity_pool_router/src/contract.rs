use crate::access_utils::require_admin_or_operator;
use crate::constants::{CONSTANT_PRODUCT_FEE_AVAILABLE, STABLESWAP_DEFAULT_A, STABLESWAP_MAX_FEE};
use crate::errors::LiquidityPoolRouterError;
use crate::events::{Events, LiquidityPoolRouterEvents};
use crate::liquidity_calculator::LiquidityCalculatorClient;
use crate::pool_interface::{
    CombinedSwapInterface, LiquidityPoolInterfaceTrait, PoolPlaneInterface, PoolsManagementTrait,
    RewardsInterfaceTrait,
};
use crate::pool_utils::{
    deploy_stableswap_pool, deploy_standard_pool, get_stableswap_pool_salt, get_standard_pool_salt,
    get_tokens_salt, get_total_liquidity, validate_tokens,
};
use crate::rewards::get_rewards_manager;
use crate::router_interface::{AdminInterface, UpgradeableContract};
use crate::storage::{
    get_init_pool_payment_address, get_init_pool_payment_token,
    get_init_stable_pool_payment_amount, get_init_standard_pool_payment_amount,
    get_liquidity_calculator, get_pool, get_pool_plane, get_pools_plain, get_reward_tokens,
    get_reward_tokens_detailed, get_rewards_config, get_tokens_set, get_tokens_set_count, has_pool,
    remove_pool, set_constant_product_pool_hash, set_init_pool_payment_address,
    set_init_pool_payment_token, set_init_stable_pool_payment_amount,
    set_init_standard_pool_payment_amount, set_liquidity_calculator, set_pool_plane,
    set_reward_tokens, set_reward_tokens_detailed, set_rewards_config, set_stableswap_pool_hash,
    set_token_hash, GlobalRewardsConfig, LiquidityPoolRewardInfo,
};
use access_control::access::{AccessControl, AccessControlTrait, OperatorAccessTrait};
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

// The `LiquidityPoolInterfaceTrait` trait provides the interface for interacting with a liquidity pool.
#[contractimpl]
impl LiquidityPoolInterfaceTrait for LiquidityPoolRouter {
    // Returns the type of the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // The type of the pool as a Symbol.
    fn pool_type(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Symbol {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        e.invoke_contract(&pool_id, &Symbol::new(&e, "pool_type"), Vec::new(&e))
    }

    // Returns information about the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // A map of Symbols to Vals representing the pool's information.
    fn get_info(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Map<Symbol, Val> {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_info"), Vec::new(&e))
    }

    // Returns the pool's address.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // The address of the pool.
    fn get_pool(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address {
        match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        }
    }

    // Returns the pool's share token address.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // The pool's share token as an Address.
    fn share_id(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        e.invoke_contract(&pool_id, &Symbol::new(&e, "share_id"), Vec::new(&e))
    }

    // Returns the total shares of the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // The total shares of the pool as a u128.
    fn get_total_shares(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_total_shares"), Vec::new(&e))
    }

    // Returns the pool's reserves.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // A vector of u128s representing the pool's reserves.
    fn get_reserves(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Vec<u128> {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };
        e.invoke_contract(&pool_id, &Symbol::new(&e, "get_reserves"), Vec::new(&e))
    }

    // Deposits tokens into the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `user` - The address of the user depositing the tokens.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    // * `amounts` - A vector of u128s representing the amounts of each token to deposit.
    // * `min_mint_amount` - The minimum amount of pool tokens to mint.
    //
    // # Returns
    //
    // A tuple containing a vector of u128s representing the amounts of each token deposited and a u128 representing the amount of pool tokens minted.
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

    // Swaps tokens in the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `user` - The address of the user swapping the tokens.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    // * `token_in` - The address of the input token to be swapped.
    // * `token_out` - The address of the output token to be received.
    // * `in_amount` - The amount of the input token to be swapped.
    // * `min_out_amount` - The minimum amount of the output token to be received.
    //
    // # Returns
    //
    // The amount of the output token received.
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

    // Estimates the result of a swap operation.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    // * `token_in` - The address of the input token to be swapped.
    // * `token_out` - The address of the output token to be received.
    // * `in_amount` - The amount of the input token to be swapped.
    //
    // # Returns
    //
    // The estimated amount of the output token that would be received.
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

    // Withdraws tokens from the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `user` - The address of the user withdrawing the tokens.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    // * `burn_amount` - The amount of pool tokens to burn.
    // * `min_amounts` - A vector of u128s representing the minimum amounts of each token to be received.
    //
    // # Returns
    //
    // A vector of u128s representing the amounts of each token withdrawn.
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

    // Returns the total liquidity of the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // The total liquidity of the pool as a U256.
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

    // Returns the address of the liquidity calculator.
    //
    // # Arguments
    //
    // * `e` - The environment.
    //
    // # Returns
    //
    // The address of the liquidity calculator.
    fn get_liquidity_calculator(e: Env) -> Address {
        get_liquidity_calculator(&e)
    }

    // Sets the liquidity calculator.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `admin` - The address of the admin user.
    // * `calculator` - The address of the liquidity calculator.
    fn set_liquidity_calculator(e: Env, admin: Address, calculator: Address) {
        let access_control = AccessControl::new(&e);
        admin.require_auth();
        access_control.check_admin(&admin);

        set_liquidity_calculator(&e, &calculator);
    }
}

// The `UpgradeableContract` trait provides the interface for upgrading the contract.
#[contractimpl]
impl UpgradeableContract for LiquidityPoolRouter {
    // Returns the version of the contract.
    //
    // # Returns
    //
    // The version of the contract as a u32.
    fn version() -> u32 {
        120
    }

    // Upgrades the contract to a new version.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `new_wasm_hash` - The hash of the new contract version.
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}

// The `AdminInterface` trait provides the interface for administrative actions.
#[contractimpl]
impl AdminInterface for LiquidityPoolRouter {
    // Initializes the admin user.
    //
    // # Arguments
    //
    // * `account` - The address of the admin user.
    fn init_admin(e: Env, account: Address) {
        let access_control = AccessControl::new(&e);
        if access_control.has_admin() {
            panic_with_error!(&e, AccessControlError::AdminAlreadySet);
        }
        access_control.set_admin(&account);
    }

    // Set operator address which can perform some restricted actions
    //
    // # Arguments
    //
    // * `account` - The address of the operator user.
    fn set_operator(e: Env, operator: Address) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        access_control.set_operator(&operator);
    }

    // Sets the liquidity pool share token wasm hash.
    //
    // # Arguments
    //
    // * `new_hash` - The token wasm hash.
    fn set_token_hash(e: Env, new_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_token_hash(&e, &new_hash);
    }

    // Sets the standard pool wasm hash.
    //
    // # Arguments
    //
    // * `new_hash` - The standard pool wasm hash.
    fn set_pool_hash(e: Env, new_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_constant_product_pool_hash(&e, &new_hash);
    }

    // Sets the stableswap pool wasm hash.
    //
    // # Arguments
    //
    // * `new_hash` - The new stableswap pool wasm hash.
    fn set_stableswap_pool_hash(e: Env, new_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_stableswap_pool_hash(&e, &new_hash);
    }

    // Configures the stableswap pool deployment payment.
    //
    // # Arguments
    //
    // * `token` - The address of the token.
    // * `amount` - The amount of the token.
    // * `to` - The address to send the payment to.
    fn configure_init_pool_payment(
        e: Env,
        token: Address,
        standard_pool_amount: u128,
        stable_pool_amount: u128,
        to: Address,
    ) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        set_init_pool_payment_token(&e, &token);
        set_init_stable_pool_payment_amount(&e, &stable_pool_amount);
        set_init_standard_pool_payment_amount(&e, &standard_pool_amount);
        set_init_pool_payment_address(&e, &to);
    }

    // Getters for init pool payment config
    fn get_init_pool_payment_token(e: Env) -> Address {
        get_init_pool_payment_token(&e)
    }

    fn get_init_pool_payment_address(e: Env) -> Address {
        get_init_pool_payment_address(&e)
    }

    fn get_stable_pool_payment_amount(e: Env) -> u128 {
        get_init_stable_pool_payment_amount(&e)
    }

    fn get_standard_pool_payment_amount(e: Env) -> u128 {
        get_init_standard_pool_payment_amount(&e)
    }

    // Sets the reward token.
    //
    // # Arguments
    //
    // * `reward_token` - The address of the reward token.
    fn set_reward_token(e: Env, reward_token: Address) {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        let rewards = get_rewards_manager(&e);
        rewards.storage().put_reward_token(reward_token);
    }

    // Returns the operator address.
    //
    // # Returns
    //
    // The operator address.
    fn get_operator(e: Env) -> Address {
        match AccessControl::new(&e).get_operator() {
            Some(address) => address,
            None => panic_with_error!(e, AccessControlError::RoleNotFound),
        }
    }
}

// The `RewardsInterfaceTrait` trait provides the interface for interacting with rewards.
#[contractimpl]
impl RewardsInterfaceTrait for LiquidityPoolRouter {
    // Retrieves the global rewards configuration and returns it as a `Map`.
    //
    // This function fetches the global rewards configuration from the contract's state.
    // The configuration includes the rewards per second (`tps`), the expiration timestamp (`expired_at`),
    // and the current block number (`current_block`).
    //
    // # Returns
    //
    // A `Map` where each key is a `Symbol` representing a configuration parameter, and the value is the corresponding value.
    // The keys are "tps", "expired_at", and "current_block".
    fn get_rewards_config(e: Env) -> Map<Symbol, i128> {
        let rewards_config = get_rewards_config(&e);
        let mut result = Map::new(&e);
        result.set(symbol_short!("tps"), rewards_config.tps as i128);
        result.set(symbol_short!("exp_at"), rewards_config.expired_at as i128);
        result.set(symbol_short!("block"), rewards_config.current_block as i128);
        result
    }

    // Returns a mapping of token addresses to their respective reward information.
    //
    // # Returns
    //
    // A `Map` where each key is a `Vec<Address>` representing a set of token addresses, and the value is a tuple
    // `(u32, bool, U256)`. The tuple elements represent the voting share, processed status, and total liquidity
    // of the tokens respectively.
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

    // Sums up the liquidity of all pools for given tokens set and returns the total liquidity
    //
    // # Arguments
    //
    // * `tokens` - A vector of token addresses for which to calculate the total liquidity.
    //
    // # Returns
    //
    // A `U256` value representing the total liquidity for the given set of tokens.
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

    // Configures the global rewards for the liquidity pool.
    //
    // # Arguments
    //
    // * `user` - This user must be authenticated and have admin or operator privileges.
    // * `reward_tps` - The rewards per second. This value is scaled by 1e7 for precision.
    // * `expired_at` - The timestamp at which the rewards configuration will expire.
    // * `tokens_votes` - A vector of tuples, where each tuple contains a vector of token addresses and a voting share.
    //   The voting share is a value between 0 and 1, scaled by 1e7 for precision.
    fn config_global_rewards(
        e: Env,
        user: Address,
        reward_tps: u128, // value with 7 decimal places. example: 600_0000000
        expired_at: u64,  // timestamp
        tokens_votes: Vec<(Vec<Address>, u32)>, // {[token1, token2]: voting_percentage}, voting percentage 0_0000000 .. 1_0000000
    ) {
        user.require_auth();
        require_admin_or_operator(&e, user);

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

    // Fills the aggregated liquidity information for a given set of tokens.
    //
    // # Arguments
    //
    // * `tokens` - A vector of token addresses for which to fill the liquidity.
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

    // Configures the rewards for a specific pool.
    //
    // This function is used to set up the rewards configuration for a specific pool.
    // It calculates the pool's share of the total rewards based on its liquidity and sets the pool's rewards configuration.
    //
    // # Arguments
    //
    // * `tokens` - A vector of token addresses that the pool consists of.
    // * `pool_index` - The index of the pool.
    //
    // # Returns
    //
    // * `pool_tps` - The total reward tokens per second (TPS) to be distributed to the pool.
    //
    // # Errors
    //
    // This function will panic if:
    //
    // * The pool does not exist.
    // * The tokens are not found in the current rewards configuration.
    // * The liquidity for the tokens has not been filled.
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

    // Get rewards status for the pool, including amount available for the user
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `user` - The address of the user.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // A map of symbols to integers representing the rewards info.
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

    // Get amount of reward tokens available for the user to claim.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `user` - The address of the user.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // The user reward as a u128.
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

    // Returns the total accumulated reward.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // The total accumulated reward as a u128.
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

    // Returns the total configured reward.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // The total configured reward as a u128.
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

    // Returns the total claimed reward.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // The total claimed reward as a u128.
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

    // Calculate difference between total configured reward and total claimed reward.
    // Helps to estimate the amount of missing reward tokens pool has configured to distribute
    fn get_total_outstanding_reward(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let configured_reward: u128 = e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_total_configured_reward"),
            Vec::new(&e),
        );
        let claimed_reward: u128 = e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_total_claimed_reward"),
            Vec::new(&e),
        );

        let rewards = get_rewards_manager(&e);
        let reward_token = rewards.storage().get_reward_token();
        let reward_token_client = SorobanTokenClient::new(&e, &reward_token);
        let mut pool_reward_balance = reward_token_client.balance(&pool_id) as u128;

        // handle edge case - if pool has reward token in reserves
        match tokens.first_index_of(reward_token) {
            Some(i) => {
                let pool_reserves: Vec<u128> =
                    e.invoke_contract(&pool_id, &Symbol::new(&e, "get_reserves"), Vec::new(&e));
                let reward_token_reserve = pool_reserves.get(i).unwrap();
                pool_reward_balance -= reward_token_reserve;
            }
            None => {}
        }
        if configured_reward - claimed_reward < pool_reward_balance {
            0_u128
        } else {
            configured_reward - claimed_reward - pool_reward_balance
        }
    }

    // Transfer outstanding reward to the pool
    fn distribute_outstanding_reward(
        e: Env,
        user: Address,
        from: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
    ) -> u128 {
        user.require_auth();
        require_admin_or_operator(&e, user);

        let pool_id = match get_pool(&e, tokens.clone(), pool_index.clone()) {
            Ok(v) => v,
            Err(err) => panic_with_error!(&e, err),
        };

        let outstanding_reward =
            Self::get_total_outstanding_reward(e.clone(), tokens.clone(), pool_index.clone());
        let rewards = get_rewards_manager(&e);
        let reward_token = rewards.storage().get_reward_token();

        if from != e.current_contract_address() {
            SorobanTokenClient::new(&e, &reward_token).transfer_from(
                &e.current_contract_address(),
                &from,
                &pool_id,
                &(outstanding_reward as i128),
            );
        } else {
            SorobanTokenClient::new(&e, &reward_token).transfer(
                &e.current_contract_address(),
                &pool_id,
                &(outstanding_reward as i128),
            );
        }
        outstanding_reward
    }

    // Claims the reward.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `user` - The address of the user.
    // * `tokens` - A vector of token addresses.
    // * `pool_index` - The pool index hash.
    //
    // # Returns
    //
    // The amount of tokens rewarded to the user as a u128.
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

// The `PoolsManagementTrait` trait provides the interface for managing liquidity pools.
#[contractimpl]
impl PoolsManagementTrait for LiquidityPoolRouter {
    // Initializes a standard pool with custom arguments.
    //
    // # Arguments
    //
    // * `user` - The address of the user initializing the pool.
    // * `tokens` - A vector of token addresses that the pool consists of.
    // * `fee_fraction` - The fee fraction for the pool. Should match pre-defined set of values: 0.1%, 0.3%, 1%.
    //
    // # Returns
    //
    // A tuple containing:
    // * The pool index hash.
    // * The address of the pool.
    fn init_standard_pool(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        fee_fraction: u32,
    ) -> (BytesN<32>, Address) {
        user.require_auth();
        validate_tokens(&e, &tokens);
        if !CONSTANT_PRODUCT_FEE_AVAILABLE.contains(&fee_fraction) {
            panic_with_error!(&e, LiquidityPoolRouterError::BadFee);
        }

        let salt = get_tokens_salt(&e, tokens.clone());
        let pools = get_pools_plain(&e, &salt);
        let pool_index = get_standard_pool_salt(&e, &fee_fraction);

        match pools.get(pool_index.clone()) {
            Some(pool_address) => (pool_index, pool_address),
            None => {
                // pay for pool creation
                let init_pool_token = get_init_pool_payment_token(&e);
                let init_pool_amount = get_init_standard_pool_payment_amount(&e);
                let init_pool_address = get_init_pool_payment_address(&e);
                if init_pool_amount > 0 {
                    SorobanTokenClient::new(&e, &init_pool_token).transfer(
                        &user,
                        &init_pool_address,
                        &(init_pool_amount as i128),
                    );
                }

                deploy_standard_pool(&e, tokens, fee_fraction)
            }
        }
    }

    // Initializes a stableswap pool with custom arguments.
    //
    // # Arguments
    //
    // * `user` - The address of the user initializing the pool.
    // * `tokens` - A vector of token addresses that the pool consists of.
    // * `fee_fraction` - The fee fraction for the pool. Has denominator 10000; 1 = 0.01%, 10 = 0.1%, 100 = 1%.
    //
    // # Returns
    //
    // A tuple containing:
    // * The pool index hash.
    // * The address of the pool.
    fn init_stableswap_pool(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        fee_fraction: u32,
    ) -> (BytesN<32>, Address) {
        user.require_auth();
        validate_tokens(&e, &tokens);
        if fee_fraction > STABLESWAP_MAX_FEE {
            panic_with_error!(&e, LiquidityPoolRouterError::BadFee);
        }

        let salt = get_tokens_salt(&e, tokens.clone());
        let pools = get_pools_plain(&e, &salt);
        let pool_index = get_stableswap_pool_salt(&e);

        match pools.get(pool_index.clone()) {
            Some(pool_address) => (pool_index, pool_address),
            None => {
                // pay for pool creation
                let init_pool_token = get_init_pool_payment_token(&e);
                let init_pool_amount = get_init_stable_pool_payment_amount(&e);
                let init_pool_address = get_init_pool_payment_address(&e);
                if init_pool_amount > 0 {
                    SorobanTokenClient::new(&e, &init_pool_token).transfer(
                        &user,
                        &init_pool_address,
                        &(init_pool_amount as i128),
                    );
                }

                deploy_stableswap_pool(&e, tokens, STABLESWAP_DEFAULT_A, fee_fraction)
            }
        }
    }

    // Returns a map of pools for given set of tokens.
    //
    // # Arguments
    //
    // * `tokens` - A vector of token addresses that the pair consists of.
    //
    // # Returns
    //
    // A map of pool index hashes to pool addresses.
    fn get_pools(e: Env, tokens: Vec<Address>) -> Map<BytesN<32>, Address> {
        let salt = get_tokens_salt(&e, tokens);
        get_pools_plain(&e, &salt)
    }

    // Returns a map of pools for given set of tokens.
    //
    // # Arguments
    //
    // * `tokens` - A vector of token addresses that the pair consists of.
    //
    // # Returns
    //
    // A map of pool index hashes to pool addresses.
    fn remove_pool(e: Env, user: Address, tokens: Vec<Address>, pool_hash: BytesN<32>) {
        let access_control = AccessControl::new(&e);
        user.require_auth();
        access_control.check_admin(&user);
        let salt = get_tokens_salt(&e, tokens.clone());
        if has_pool(&e, &salt, pool_hash.clone()) {
            remove_pool(&e, &salt, pool_hash)
        }
    }

    // Returns the number of unique token sets.
    //
    // # Returns
    //
    // The number of unique token sets.
    fn get_tokens_sets_count(e: Env) -> u128 {
        get_tokens_set_count(&e)
    }

    // Retrieves tokens at a specified index.
    //
    // # Arguments
    //
    // * `index` - The index of the token set to retrieve.
    //
    // # Returns
    //
    // A vector of token addresses at the specified index.
    fn get_tokens(e: Env, index: u128) -> Vec<Address> {
        get_tokens_set(&e, index)
    }

    // Retrieves a list of pools in batch based on half-open `[..)` range of tokens indexes.
    //
    // # Arguments
    //
    // * `start` - The start index of the range.
    // * `end` - The end index of the range.
    //
    // # Returns
    //
    // A list containing tuples containing a vector of addresses of the corresponding tokens
    // and a mapping of pool hashes to pool addresses.
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

// The `PoolPlaneInterface` trait provides the interface for interacting with a pool plane.
#[contractimpl]
impl PoolPlaneInterface for LiquidityPoolRouter {
    // Sets the pool plane.
    // Pool plane is a contract which knows current state of every pool
    // and can be used to estimate swaps without calling pool contracts.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin user.
    // * `plane` - The address of the plane.
    fn set_pools_plane(e: Env, admin: Address, plane: Address) {
        let access_control = AccessControl::new(&e);
        admin.require_auth();
        access_control.check_admin(&admin);

        set_pool_plane(&e, &plane);
    }

    // Returns the address of the pool plane.
    fn get_plane(e: Env) -> Address {
        get_pool_plane(&e)
    }
}

#[contractimpl]
impl CombinedSwapInterface for LiquidityPoolRouter {
    // Executes a chain of token swaps to exchange an input token for an output token.
    //
    // # Arguments
    //
    // * `user` - The address of the user executing the swaps.
    // * `swaps_chain` - The series of swaps to be executed. Each swap is represented by a tuple containing:
    //   - A vector of token addresses liquidity pool belongs to
    //   - Pool index hash
    //   - The token to obtain
    // * `token_in` - The address of the input token to be swapped.
    // * `in_amount` - The amount of the input token to be swapped.
    // * `out_min` - The minimum amount of the output token to be received.
    //
    // # Returns
    //
    // The amount of the output token received after all swaps have been executed.
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
