use crate::constants::{CONSTANT_PRODUCT_FEE_AVAILABLE, STABLESWAP_DEFAULT_A, STABLESWAP_MAX_FEE};
use crate::errors::LiquidityPoolRouterError;
use crate::events::{Events, LiquidityPoolRouterEvents};
use crate::liquidity_calculator::LiquidityCalculatorClient;
use crate::pool_interface::{
    CombinedSwapInterface, LiquidityPoolInterfaceTrait, PoolPlaneInterface, PoolsManagementTrait,
    RewardsInterfaceTrait,
};
use crate::pool_utils::{
    assert_tokens_sorted, deploy_stableswap_pool, deploy_standard_pool, get_stableswap_pool_salt,
    get_standard_pool_salt, get_tokens_salt, get_total_liquidity, validate_tokens_contracts,
};
use crate::rewards::get_rewards_manager;
use crate::router_interface::AdminInterface;
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
use access_control::access::{AccessControl, AccessControlTrait};
use access_control::emergency::{get_emergency_mode, set_emergency_mode};
use access_control::errors::AccessControlError;
use access_control::events::Events as AccessControlEvents;
use access_control::interface::TransferableContract;
use access_control::management::{MultipleAddressesManagementTrait, SingleAddressManagementTrait};
use access_control::role::Role;
use access_control::role::SymbolRepresentation;
use access_control::transfer::TransferOwnershipTrait;
use access_control::utils::{require_operations_admin_or_owner, require_rewards_admin_or_owner};
use rewards::storage::{BoostFeedStorageTrait, BoostTokenStorageTrait, RewardTokenStorageTrait};
use soroban_sdk::auth::{ContractContext, InvokerContractAuthEntry, SubContractInvocation};
use soroban_sdk::token::Client as SorobanTokenClient;
use soroban_sdk::{
    contract, contractimpl, panic_with_error, symbol_short, vec, Address, BytesN, Env, IntoVal,
    Map, Symbol, Val, Vec, U256,
};
use upgrade::events::Events as UpgradeEvents;
use upgrade::interface::UpgradeableContract;
use upgrade::{apply_upgrade, commit_upgrade, revert_upgrade};
use utils::storage_errors::StorageError;

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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);
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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);
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
        assert_tokens_sorted(&e, &tokens);
        get_pool(&e, &tokens, pool_index)
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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);
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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);
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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);
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
        assert_tokens_sorted(&e, &tokens);

        let pool_id = get_pool(&e, &tokens, pool_index);

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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);

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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);

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
        assert_tokens_sorted(&e, &tokens);

        let pool_id = get_pool(&e, &tokens, pool_index);

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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);

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
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);

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
        150
    }

    // Commits a new wasm hash for a future upgrade.
    // The upgrade will be available through `apply_upgrade` after the standard upgrade delay
    // unless the system is in emergency mode.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `new_wasm_hash` - The new wasm hash to commit.
    fn commit_upgrade(e: Env, admin: Address, new_wasm_hash: BytesN<32>) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        commit_upgrade(&e, &new_wasm_hash);
        UpgradeEvents::new(&e).commit_upgrade(Vec::from_array(&e, [new_wasm_hash.clone()]));
    }

    // Applies the committed upgrade.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn apply_upgrade(e: Env, admin: Address) -> BytesN<32> {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        let new_wasm_hash = apply_upgrade(&e);
        UpgradeEvents::new(&e).apply_upgrade(Vec::from_array(&e, [new_wasm_hash.clone()]));
        new_wasm_hash
    }

    // Reverts the committed upgrade.
    // This can be used to cancel a previously committed upgrade.
    // The upgrade will be canceled only if it has not been applied yet.
    // If the upgrade has already been applied, it cannot be reverted.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn revert_upgrade(e: Env, admin: Address) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        revert_upgrade(&e);
        UpgradeEvents::new(&e).revert_upgrade();
    }

    // Sets the emergency mode.
    // When the emergency mode is set to true, the contract will allow instant upgrades without the delay.
    // This is useful in case of critical issues that need to be fixed immediately.
    // When the emergency mode is set to false, the contract will require the standard upgrade delay.
    // The emergency mode can only be set by the emergency admin.
    //
    // # Arguments
    //
    // * `emergency_admin` - The address of the emergency admin.
    // * `value` - The value to set the emergency mode to.
    fn set_emergency_mode(e: Env, emergency_admin: Address, value: bool) {
        emergency_admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&emergency_admin, &Role::EmergencyAdmin);
        set_emergency_mode(&e, &value);
        AccessControlEvents::new(&e).set_emergency_mode(value);
    }

    // Returns the emergency mode flag value.
    fn get_emergency_mode(e: Env) -> bool {
        get_emergency_mode(&e)
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
        if access_control.get_role_safe(&Role::Admin).is_some() {
            panic_with_error!(&e, AccessControlError::AdminAlreadySet);
        }
        access_control.set_role_address(&Role::Admin, &account);
    }

    // Sets the privileged addresses.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `rewards_admin` - The address of the rewards admin.
    // * `operations_admin` - The address of the operations admin.
    // * `pause_admin` - The address of the pause admin.
    // * `emergency_pause_admin` - The addresses of the emergency pause admins.
    fn set_privileged_addrs(
        e: Env,
        admin: Address,
        rewards_admin: Address,
        operations_admin: Address,
        pause_admin: Address,
        emergency_pause_admins: Vec<Address>,
    ) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, &Role::Admin);

        access_control.set_role_address(&Role::RewardsAdmin, &rewards_admin);
        access_control.set_role_address(&Role::OperationsAdmin, &operations_admin);
        access_control.set_role_address(&Role::PauseAdmin, &pause_admin);
        access_control.set_role_addresses(&Role::EmergencyPauseAdmin, &emergency_pause_admins);
        AccessControlEvents::new(&e).set_privileged_addrs(
            rewards_admin,
            operations_admin,
            pause_admin,
            emergency_pause_admins,
        );
    }

    // Returns a map of privileged roles.
    //
    // # Returns
    //
    // A map of privileged roles to their respective addresses.
    fn get_privileged_addrs(e: Env) -> Map<Symbol, Vec<Address>> {
        let access_control = AccessControl::new(&e);
        let mut result: Map<Symbol, Vec<Address>> = Map::new(&e);
        for role in [
            Role::Admin,
            Role::EmergencyAdmin,
            Role::RewardsAdmin,
            Role::OperationsAdmin,
            Role::PauseAdmin,
        ] {
            result.set(
                role.as_symbol(&e),
                match access_control.get_role_safe(&role) {
                    Some(v) => Vec::from_array(&e, [v]),
                    None => Vec::new(&e),
                },
            );
        }

        result.set(
            Role::EmergencyPauseAdmin.as_symbol(&e),
            access_control.get_role_addresses(&Role::EmergencyPauseAdmin),
        );

        result
    }

    // Sets the liquidity pool share token wasm hash.
    //
    // # Arguments
    //
    // * `new_hash` - The token wasm hash.
    fn set_token_hash(e: Env, admin: Address, new_hash: BytesN<32>) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        set_token_hash(&e, &new_hash);
    }

    // Sets the standard pool wasm hash.
    //
    // # Arguments
    //
    // * `new_hash` - The standard pool wasm hash.
    fn set_pool_hash(e: Env, admin: Address, new_hash: BytesN<32>) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        set_constant_product_pool_hash(&e, &new_hash);
    }

    // Sets the stableswap pool wasm hash.
    //
    // # Arguments
    //
    // * `new_hash` - The new stableswap pool wasm hash.
    fn set_stableswap_pool_hash(e: Env, admin: Address, new_hash: BytesN<32>) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        set_stableswap_pool_hash(&e, &new_hash);
    }

    // Configures the stableswap pool deployment payment.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `token` - The address of the token.
    // * `amount` - The amount of the token.
    // * `to` - The address to send the payment to.
    fn configure_init_pool_payment(
        e: Env,
        admin: Address,
        token: Address,
        standard_pool_amount: u128,
        stable_pool_amount: u128,
        to: Address,
    ) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);

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
    fn set_reward_token(e: Env, admin: Address, reward_token: Address) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        get_rewards_manager(&e)
            .storage()
            .put_reward_token(reward_token);
    }

    fn set_reward_boost_config(
        e: Env,
        admin: Address,
        reward_boost_token: Address,
        reward_boost_feed: Address,
    ) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);

        let rewards_storage = get_rewards_manager(&e).storage();
        rewards_storage.put_reward_boost_token(reward_boost_token);
        rewards_storage.put_reward_boost_feed(reward_boost_feed);
    }
}

// The `RewardsInterfaceTrait` trait provides the interface for interacting with rewards.
#[contractimpl]
impl RewardsInterfaceTrait for LiquidityPoolRouter {
    // Retrieves the global rewards configuration and returns it as a `Map`.
    //
    // This function fetches the global rewards configuration from the contract's state.
    // The configuration includes the rewards per second (`tps`) and the expiration timestamp (`expired_at`)
    //
    // # Returns
    //
    // A `Map` where each key is a `Symbol` representing a configuration parameter, and the value is the corresponding value.
    // The keys are "tps" and "expired_at".
    fn get_rewards_config(e: Env) -> Map<Symbol, i128> {
        let rewards_config = get_rewards_config(&e);
        let mut result = Map::new(&e);
        result.set(symbol_short!("tps"), rewards_config.tps as i128);
        result.set(symbol_short!("exp_at"), rewards_config.expired_at as i128);
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
        let tokens = get_reward_tokens(&e);
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
        assert_tokens_sorted(&e, &tokens);
        let tokens_salt = get_tokens_salt(&e, &tokens);
        let pools = get_pools_plain(&e, tokens_salt);

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
        require_rewards_admin_or_owner(&e, &user);

        let mut tokens_with_liquidity = Map::new(&e);
        for (tokens, voting_share) in tokens_votes {
            assert_tokens_sorted(&e, &tokens);

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

        set_reward_tokens(&e, &tokens_with_liquidity);
        set_rewards_config(
            &e,
            &GlobalRewardsConfig {
                tps: reward_tps,
                expired_at,
            },
        )
    }

    // Fills the aggregated liquidity information for a given set of tokens.
    //
    // # Arguments
    //
    // * `tokens` - A vector of token addresses for which to fill the liquidity.
    fn fill_liquidity(e: Env, tokens: Vec<Address>) {
        assert_tokens_sorted(&e, &tokens);
        let tokens_salt = get_tokens_salt(&e, &tokens);
        let calculator = get_liquidity_calculator(&e);
        let (pools, total_liquidity) = get_total_liquidity(&e, &tokens, calculator);

        let mut pools_with_processed_info = Map::new(&e);
        for (key, value) in pools {
            pools_with_processed_info.set(key, (value, false));
        }

        let mut tokens_with_liquidity = get_reward_tokens(&e);
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
        set_reward_tokens(&e, &tokens_with_liquidity);
        set_reward_tokens_detailed(&e, tokens_salt, &pools_with_processed_info);
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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index.clone());

        let rewards_config = get_rewards_config(&e);
        let tokens_salt = get_tokens_salt(&e, &tokens);
        let mut tokens_detailed = get_reward_tokens_detailed(&e, tokens_salt.clone());
        let tokens_reward = get_reward_tokens(&e);
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
            set_reward_tokens_detailed(&e, tokens_salt, &tokens_detailed);
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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);

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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);

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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);

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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);

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
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);

        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_total_claimed_reward"),
            Vec::new(&e),
        )
    }

    // Calculate difference between total configured reward and total claimed reward.
    // Helps to estimate the amount of missing reward tokens pool has configured to distribute
    fn get_total_outstanding_reward(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128 {
        assert_tokens_sorted(&e, &tokens);
        let pool_id = get_pool(&e, &tokens, pool_index);

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
        configured_reward.saturating_sub(claimed_reward + pool_reward_balance)
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
        require_rewards_admin_or_owner(&e, &user);
        assert_tokens_sorted(&e, &tokens);

        let pool_id = get_pool(&e, &tokens, pool_index.clone());

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
        assert_tokens_sorted(&e, &tokens);

        let pool_id = get_pool(&e, &tokens, pool_index);

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
        validate_tokens_contracts(&e, &tokens);
        assert_tokens_sorted(&e, &tokens);

        if !CONSTANT_PRODUCT_FEE_AVAILABLE.contains(&fee_fraction) {
            panic_with_error!(&e, LiquidityPoolRouterError::BadFee);
        }

        let salt = get_tokens_salt(&e, &tokens);
        let pools = get_pools_plain(&e, salt);
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

                deploy_standard_pool(&e, &tokens, fee_fraction)
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
        validate_tokens_contracts(&e, &tokens);
        assert_tokens_sorted(&e, &tokens);

        if fee_fraction > STABLESWAP_MAX_FEE {
            panic_with_error!(&e, LiquidityPoolRouterError::BadFee);
        }

        let salt = get_tokens_salt(&e, &tokens);
        let pools = get_pools_plain(&e, salt);
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

                // calculate amplification factor
                // Amp = A*N**(N-1)
                let n = tokens.len();
                let amp = STABLESWAP_DEFAULT_A * (n as u128).pow(n - 1);
                deploy_stableswap_pool(&e, &tokens, amp, fee_fraction)
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
        assert_tokens_sorted(&e, &tokens);
        let salt = get_tokens_salt(&e, &tokens);
        get_pools_plain(&e, salt)
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
        user.require_auth();
        require_operations_admin_or_owner(&e, &user);
        assert_tokens_sorted(&e, &tokens);

        let salt = get_tokens_salt(&e, &tokens);
        if has_pool(&e, salt.clone(), pool_hash.clone()) {
            remove_pool(&e, salt, pool_hash)
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
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);

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
            assert_tokens_sorted(&e, &tokens);

            let pool_id = get_pool(&e, &tokens, pool_index);

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
    // * `out_amount` - The amount of the output token to be received.
    // * `in_max` - The max amount of the input token to spend.
    //
    // # Returns
    //
    // The amount of the input token spent after all swaps have been executed.
    // Executes a chain of token swaps with strict receive functionality.
    fn swap_chained_strict_receive(
        e: Env,
        user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address,
        out_amount: u128, // fixed amount of output token to receive
        max_in: u128,     // maximum input token amount allowed
    ) -> u128 {
        user.require_auth();

        if swaps_chain.len() == 0 {
            panic_with_error!(&e, LiquidityPoolRouterError::PathIsEmpty);
        }

        // -------------------------------
        // Reverse pass: compute required inputs per hop
        // -------------------------------
        let mut required_amounts: Vec<u128> = Vec::new(&e);
        let mut desired_out = out_amount;

        let estimate_fn = Symbol::new(&e, "estimate_swap_strict_receive");
        let swap_fn = Symbol::new(&e, "swap_strict_receive");

        // Process swaps in reverse order
        for i in (0..swaps_chain.len()).rev() {
            let (tokens, pool_index, token_out) = swaps_chain.get(i).unwrap();
            let pool_id = get_pool(&e, &tokens, pool_index);
            let token_in_for_hop = if i == 0 {
                token_in.clone()
            } else {
                // For a middle hop, the input is the output of the previous swap in the chain.
                swaps_chain.get(i - 1).unwrap().2.clone()
            };

            // Calculate required input for this hop using pool pricing.
            // Assumes the pool has a function like `calc_in_given_out`.
            let required_in: u128 = e.invoke_contract(
                &pool_id,
                &estimate_fn,
                Vec::from_array(
                    &e,
                    [
                        tokens
                            .first_index_of(token_in_for_hop.clone())
                            .unwrap()
                            .into_val(&e),
                        tokens
                            .first_index_of(token_out.clone())
                            .unwrap()
                            .into_val(&e),
                        desired_out.into_val(&e),
                    ],
                ),
            );
            required_amounts.push_front(required_in);
            // The output required from the previous hop is the input needed here.
            desired_out = required_in;
        }
        let total_required_input = required_amounts.get_unchecked(0);

        // Verify that the required input does not exceed the maximum provided.
        if total_required_input > max_in {
            panic_with_error!(&e, LiquidityPoolRouterError::InMaxNotSatisfied);
        }

        // -------------------------------
        // Forward pass: execute the swaps
        // -------------------------------
        // Pull the maximum required input from the user.
        SorobanTokenClient::new(&e, &token_in).transfer(
            &user,
            &e.current_contract_address(),
            &(max_in as i128),
        );
        // Return back the difference
        if max_in > total_required_input {
            SorobanTokenClient::new(&e, &token_in).transfer(
                &e.current_contract_address(),
                &user,
                &((max_in - total_required_input) as i128),
            );
        }

        let mut current_in = total_required_input;
        let mut last_token_out: Option<Address> = None;

        // Execute each swap in sequence.
        for i in 0..swaps_chain.len() {
            let (tokens, pool_index, token_out) = swaps_chain.get(i).unwrap();
            let pool_id = get_pool(&e, &tokens, pool_index);
            let token_in_local = if i == 0 {
                token_in.clone()
            } else {
                last_token_out.unwrap()
            };

            // Set the minimum acceptable output for this hop.
            // For intermediate hops, this is the required amount computed for the next swap.
            // For the final hop, it is the desired `out_amount`.
            let out_local = if i == swaps_chain.len() - 1 {
                out_amount
            } else {
                required_amounts.get_unchecked(i + 1)
            };

            // Authorize and perform the swap.
            e.authorize_as_current_contract(vec![
                &e,
                InvokerContractAuthEntry::Contract(SubContractInvocation {
                    context: ContractContext {
                        contract: token_in_local.clone(),
                        fn_name: Symbol::new(&e, "transfer"),
                        args: (
                            e.current_contract_address(),
                            pool_id.clone(),
                            current_in as i128,
                        )
                            .into_val(&e),
                    },
                    sub_invocations: vec![&e],
                }),
            ]);

            let in_local: u128 = e.invoke_contract(
                &pool_id,
                &swap_fn,
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
                        out_local.into_val(&e),
                        current_in.into_val(&e),
                    ],
                ),
            );

            // Emit an event for the swap.
            Events::new(&e).swap(
                tokens,
                user.clone(),
                pool_id,
                token_in_local.clone(),
                token_out.clone(),
                in_local,
                out_local,
            );

            current_in = out_local;
            last_token_out = Some(token_out);
        }

        // Finally, transfer the received output tokens to the user.
        let final_token = last_token_out.unwrap();
        SorobanTokenClient::new(&e, &final_token).transfer(
            &e.current_contract_address(),
            &user,
            &(current_in as i128),
        );

        total_required_input
    }
}

// The `TransferableContract` trait provides the interface for transferring ownership of the contract.
#[contractimpl]
impl TransferableContract for LiquidityPoolRouter {
    // Commits an ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `role_name` - The name of the role to transfer ownership of. The role must be one of the following:
    //     * `Admin`
    //     * `EmergencyAdmin`
    // * `new_address` - New address for the role
    fn commit_transfer_ownership(e: Env, admin: Address, role_name: Symbol, new_address: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, &Role::Admin);

        let role = Role::from_symbol(&e, role_name);
        access_control.commit_transfer_ownership(&role, &new_address);
        AccessControlEvents::new(&e).commit_transfer_ownership(role, new_address);
    }

    // Applies the committed ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `role_name` - The name of the role to transfer ownership of. The role must be one of the following:
    //     * `Admin`
    //     * `EmergencyAdmin`
    fn apply_transfer_ownership(e: Env, admin: Address, role_name: Symbol) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, &Role::Admin);

        let role = Role::from_symbol(&e, role_name);
        let new_address = access_control.apply_transfer_ownership(&role);
        AccessControlEvents::new(&e).apply_transfer_ownership(role, new_address);
    }

    // Reverts the committed ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `role_name` - The name of the role to transfer ownership of. The role must be one of the following:
    //     * `Admin`
    //     * `EmergencyAdmin`
    fn revert_transfer_ownership(e: Env, admin: Address, role_name: Symbol) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, &Role::Admin);

        let role = Role::from_symbol(&e, role_name);
        access_control.revert_transfer_ownership(&role);
        AccessControlEvents::new(&e).revert_transfer_ownership(role);
    }

    // Returns the future address for the role.
    // The future address is the address that the ownership of the role will be transferred to.
    // The future address is set using the `commit_transfer_ownership` function.
    // The address will be defaulted to the current address if the transfer is not committed.
    //
    // # Arguments
    //
    // * `role_name` - The name of the role to get the future address for. The role must be one of the following:
    //    * `Admin`
    //    * `EmergencyAdmin`
    fn get_future_address(e: Env, role_name: Symbol) -> Address {
        let access_control = AccessControl::new(&e);
        let role = Role::from_symbol(&e, role_name);
        match access_control.get_transfer_ownership_deadline(&role) {
            0 => match access_control.get_role_safe(&role) {
                Some(address) => address,
                None => panic_with_error!(&e, AccessControlError::RoleNotFound),
            },
            _ => access_control.get_future_address(&role),
        }
    }
}
