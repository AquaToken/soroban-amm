use crate::constants::FEE_MULTIPLIER;
use crate::errors::LiquidityPoolError;
use crate::plane::update_plane;
use crate::plane_interface::Plane;
use crate::pool;
use crate::pool::get_amount_out;
use crate::pool_interface::{
    AdminInterfaceTrait, LiquidityPoolCrunch, LiquidityPoolTrait, RewardsTrait,
    UpgradeableContractTrait,
};
use crate::rewards::get_rewards_manager;
use crate::storage::{
    get_fee_fraction, get_is_killed_claim, get_is_killed_deposit, get_is_killed_swap, get_plane,
    get_reserve_a, get_reserve_b, get_router, get_token_a, get_token_b, has_plane,
    put_fee_fraction, put_reserve_a, put_reserve_b, put_token_a, put_token_b, set_is_killed_claim,
    set_is_killed_deposit, set_is_killed_swap, set_plane, set_router,
};
use crate::token::{create_contract, transfer_a, transfer_b};
use access_control::access::{AccessControl, AccessControlTrait};
use liquidity_pool_events::Events as PoolEvents;
use liquidity_pool_events::LiquidityPoolEvents;
use liquidity_pool_validation_errors::LiquidityPoolValidationError;
use rewards::storage::RewardsStorageTrait;
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{
    contract, contractimpl, contractmeta, panic_with_error, symbol_short, Address, BytesN, Env,
    IntoVal, Map, Symbol, Val, Vec, U256,
};
use token_share::{
    burn_shares, get_balance_shares, get_token_share, get_total_shares, get_user_balance_shares,
    mint_shares, put_token_share, Client as LPTokenClient,
};
use utils::u256_math::ExtraMath;

// Metadata that is added on to the WASM custom section
contractmeta!(
    key = "Description",
    val = "Constant product AMM with configurable swap fee"
);

#[contract]
pub struct LiquidityPool;

#[contractimpl]
impl LiquidityPoolCrunch for LiquidityPool {
    // Initializes all the components of the liquidity pool.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin user.
    // * `router` - The address of the router.
    // * `lp_token_wasm_hash` - The hash of the liquidity pool token contract.
    // * `tokens` - A vector of token addresses.
    // * `fee_fraction` - The fee fraction for the pool.
    // * `reward_token` - The address of the reward token.
    // * `plane` - The address of the plane.
    fn initialize_all(
        e: Env,
        admin: Address,
        router: Address,
        lp_token_wasm_hash: BytesN<32>,
        tokens: Vec<Address>,
        fee_fraction: u32,
        reward_token: Address,
        plane: Address,
    ) {
        // merge whole initialize process into one because lack of caching of VM components
        // https://github.com/stellar/rs-soroban-env/issues/827
        Self::init_pools_plane(e.clone(), plane);
        Self::initialize(
            e.clone(),
            admin,
            router,
            lp_token_wasm_hash,
            tokens,
            fee_fraction,
        );
        Self::initialize_rewards_config(e.clone(), reward_token);
    }
}

#[contractimpl]
impl LiquidityPoolTrait for LiquidityPool {
    // Returns the type of the pool.
    //
    // # Returns
    //
    // The type of the pool as a Symbol.
    fn pool_type(e: Env) -> Symbol {
        Symbol::new(&e, "constant_product")
    }

    // Initializes the liquidity pool.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin user.
    // * `router` - The address of the router.
    // * `lp_token_wasm_hash` - The hash of the liquidity pool token contract.
    // * `tokens` - A vector of token addresses.
    // * `fee_fraction` - The fee fraction for the pool.
    fn initialize(
        e: Env,
        admin: Address,
        router: Address,
        lp_token_wasm_hash: BytesN<32>,
        tokens: Vec<Address>,
        fee_fraction: u32,
    ) {
        let access_control = AccessControl::new(&e);
        if access_control.has_admin() {
            panic_with_error!(&e, LiquidityPoolError::AlreadyInitialized);
        }
        access_control.set_admin(&admin);
        set_router(&e, &router);

        if tokens.len() != 2 {
            panic_with_error!(&e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        let token_a = tokens.get(0).unwrap();
        let token_b = tokens.get(1).unwrap();

        if token_a >= token_b {
            panic_with_error!(&e, LiquidityPoolValidationError::TokensNotSorted);
        }

        let share_contract = create_contract(&e, lp_token_wasm_hash, &token_a, &token_b);
        LPTokenClient::new(&e, &share_contract).initialize(
            &e.current_contract_address(),
            &7u32,
            &"Pool Share Token".into_val(&e),
            &"POOL".into_val(&e),
        );

        // 0.01% = 1; 1% = 100; 0.3% = 30
        if fee_fraction > 9999 {
            panic_with_error!(&e, LiquidityPoolValidationError::FeeOutOfBounds);
        }
        put_fee_fraction(&e, fee_fraction);

        put_token_a(&e, token_a);
        put_token_b(&e, token_b);
        put_token_share(&e, share_contract);
        put_reserve_a(&e, 0);
        put_reserve_b(&e, 0);

        // update plane data for every pool update
        update_plane(&e);
    }

    // Returns the pool's share token address.
    //
    // # Returns
    //
    // The pool's share token as an Address.
    fn share_id(e: Env) -> Address {
        get_token_share(&e)
    }

    // Returns the total shares of the pool.
    //
    // # Returns
    //
    // The total shares of the pool as a u128.
    fn get_total_shares(e: Env) -> u128 {
        get_total_shares(&e)
    }

    // Returns the pool's tokens.
    //
    // # Returns
    //
    // A vector of token addresses.
    fn get_tokens(e: Env) -> Vec<Address> {
        Vec::from_array(&e, [get_token_a(&e), get_token_b(&e)])
    }

    // Deposits tokens into the pool.
    //
    // # Arguments
    //
    // * `user` - The address of the user depositing the tokens.
    // * `desired_amounts` - A vector of desired amounts of each token to deposit.
    // * `min_shares` - The minimum amount of pool tokens to mint.
    //
    // # Returns
    //
    // A tuple containing a vector of actual amounts of each token deposited and a u128 representing the amount of pool tokens minted.
    fn deposit(
        e: Env,
        user: Address,
        desired_amounts: Vec<u128>,
        min_shares: u128,
    ) -> (Vec<u128>, u128) {
        // Depositor needs to authorize the deposit
        user.require_auth();

        if get_is_killed_deposit(&e) {
            panic_with_error!(e, LiquidityPoolError::PoolDepositKilled);
        }

        if desired_amounts.len() != 2 {
            panic_with_error!(&e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        let (reserve_a, reserve_b) = (get_reserve_a(&e), get_reserve_b(&e));

        // Before actual changes were made to the pool, update total rewards data and refresh/initialize user reward
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        let pool_data = rewards.manager().update_rewards_data(total_shares);
        rewards
            .manager()
            .update_user_reward(&pool_data, &user, user_shares);
        rewards.storage().bump_user_reward_data(&user);

        let desired_a = desired_amounts.get(0).unwrap();
        let desired_b = desired_amounts.get(1).unwrap();

        if (reserve_a == 0 && reserve_b == 0) && (desired_a == 0 || desired_b == 0) {
            panic_with_error!(&e, LiquidityPoolValidationError::AllCoinsRequired);
        }

        let token_a_client = SorobanTokenClient::new(&e, &get_token_a(&e));
        let token_b_client = SorobanTokenClient::new(&e, &get_token_b(&e));
        // transfer full amount then return back remaining parts to have tx auth deterministic
        token_a_client.transfer(&user, &e.current_contract_address(), &(desired_a as i128));
        token_b_client.transfer(&user, &e.current_contract_address(), &(desired_b as i128));

        let (min_a, min_b) = (0, 0);

        // Calculate deposit amounts
        let amounts =
            pool::get_deposit_amounts(&e, desired_a, min_a, desired_b, min_b, reserve_a, reserve_b);

        // Increase reserves
        put_reserve_a(&e, reserve_a + amounts.0);
        put_reserve_b(&e, reserve_a + amounts.1);

        if amounts.0 < desired_a {
            token_a_client.transfer(
                &e.current_contract_address(),
                &user,
                &((desired_a - amounts.0) as i128),
            );
        }
        if amounts.1 < desired_b {
            token_b_client.transfer(
                &e.current_contract_address(),
                &user,
                &((desired_b - amounts.1) as i128),
            );
        }

        // Now calculate how many new pool shares to mint
        let (new_reserve_a, new_reserve_b) = (get_reserve_a(&e), get_reserve_b(&e));
        let total_shares = get_total_shares(&e);

        let zero = 0;
        let new_total_shares = if reserve_a > zero && reserve_b > zero {
            let shares_a = new_reserve_a.fixed_mul_floor(&e, &total_shares, &reserve_a);
            let shares_b = new_reserve_b.fixed_mul_floor(&e, &total_shares, &reserve_b);
            shares_a.min(shares_b)
        } else {
            // if .mul doesn't fail, sqrt also won't -> safe to unwrap
            U256::from_u128(&e, new_reserve_a)
                .mul(&U256::from_u128(&e, new_reserve_b))
                .sqrt()
                .to_u128()
                .unwrap()
        };

        let shares_to_mint = new_total_shares - total_shares;
        if shares_to_mint < min_shares {
            panic_with_error!(&e, LiquidityPoolValidationError::OutMinNotSatisfied);
        }
        mint_shares(&e, user, shares_to_mint as i128);
        put_reserve_a(&e, new_reserve_a);
        put_reserve_b(&e, new_reserve_b);

        // update plane data for every pool update
        update_plane(&e);

        let amounts_vec = Vec::from_array(&e, [amounts.0, amounts.1]);
        PoolEvents::new(&e).deposit_liquidity(
            Self::get_tokens(e.clone()),
            amounts_vec.clone(),
            shares_to_mint,
        );

        (amounts_vec, shares_to_mint)
    }

    // Swaps tokens in the pool.
    //
    // # Arguments
    //
    // * `user` - The address of the user swapping the tokens.
    // * `in_idx` - The index of the input token to be swapped.
    // * `out_idx` - The index of the output token to be received.
    // * `in_amount` - The amount of the input token to be swapped.
    // * `out_min` - The minimum amount of the output token to be received.
    //
    // # Returns
    //
    // The amount of the output token received.
    fn swap(
        e: Env,
        user: Address,
        in_idx: u32,
        out_idx: u32,
        in_amount: u128,
        out_min: u128,
    ) -> u128 {
        user.require_auth();

        if get_is_killed_swap(&e) {
            panic_with_error!(e, LiquidityPoolError::PoolSwapKilled);
        }

        if in_idx == out_idx {
            panic_with_error!(&e, LiquidityPoolValidationError::CannotSwapSameToken);
        }

        if in_idx > 1 {
            panic_with_error!(&e, LiquidityPoolValidationError::InTokenOutOfBounds);
        }

        if out_idx > 1 {
            panic_with_error!(&e, LiquidityPoolValidationError::OutTokenOutOfBounds);
        }

        if in_amount == 0 {
            panic_with_error!(e, LiquidityPoolValidationError::ZeroAmount);
        }

        let reserve_a = get_reserve_a(&e);
        let reserve_b = get_reserve_b(&e);
        let reserves = Vec::from_array(&e, [reserve_a, reserve_b]);
        let tokens = Self::get_tokens(e.clone());

        let reserve_sell = reserves.get(in_idx).unwrap();
        let reserve_buy = reserves.get(out_idx).unwrap();
        if reserve_sell == 0 || reserve_buy == 0 {
            panic_with_error!(&e, LiquidityPoolValidationError::EmptyPool);
        }

        let (out, fee) = get_amount_out(&e, in_amount, reserve_sell, reserve_buy);

        if out < out_min {
            panic_with_error!(&e, LiquidityPoolValidationError::OutMinNotSatisfied);
        }

        // Transfer the amount being sold to the contract
        let sell_token = tokens.get(in_idx).unwrap();
        let sell_token_client = SorobanTokenClient::new(&e, &sell_token);
        sell_token_client.transfer(&user, &e.current_contract_address(), &(in_amount as i128));

        if in_idx == 0 {
            put_reserve_a(&e, reserve_a + in_amount);
        } else {
            put_reserve_b(&e, reserve_b + in_amount);
        }

        let (new_reserve_a, new_reserve_b) = (get_reserve_a(&e), get_reserve_b(&e));

        // residue_numerator and residue_denominator are the amount that the invariant considers after
        // deducting the fee, scaled up by FEE_MULTIPLIER to avoid fractions
        let residue_numerator = FEE_MULTIPLIER - (get_fee_fraction(&e) as u128);
        let residue_denominator = U256::from_u128(&e, FEE_MULTIPLIER);

        let new_invariant_factor = |reserve: u128, old_reserve: u128, out: u128| {
            if reserve - old_reserve > out {
                residue_denominator
                    .mul(&U256::from_u128(&e, old_reserve))
                    .add(
                        &(U256::from_u128(&e, residue_numerator)
                            .mul(&U256::from_u128(&e, reserve - old_reserve - out))),
                    )
            } else {
                residue_denominator
                    .mul(&U256::from_u128(&e, old_reserve))
                    .add(&residue_denominator.mul(&U256::from_u128(&e, reserve)))
                    .sub(&(residue_denominator.mul(&U256::from_u128(&e, old_reserve + out))))
            }
        };

        let (out_a, out_b) = if out_idx == 0 { (out, 0) } else { (0, out) };

        let new_inv_a = new_invariant_factor(new_reserve_a, reserve_a, out_a);
        let new_inv_b = new_invariant_factor(new_reserve_b, reserve_b, out_b);
        let old_inv_a = residue_denominator.mul(&U256::from_u128(&e, reserve_a));
        let old_inv_b = residue_denominator.mul(&U256::from_u128(&e, reserve_b));

        if new_inv_a.mul(&new_inv_b) < old_inv_a.mul(&old_inv_b) {
            panic_with_error!(&e, LiquidityPoolError::InvariantDoesNotHold);
        }

        if out_idx == 0 {
            transfer_a(&e, user.clone(), out_a);
            put_reserve_a(&e, reserve_a - out);
        } else {
            transfer_b(&e, user.clone(), out_b);
            put_reserve_b(&e, reserve_b - out);
        }

        // update plane data for every pool update
        update_plane(&e);

        PoolEvents::new(&e).trade(
            user,
            sell_token,
            tokens.get(out_idx).unwrap(),
            in_amount,
            out,
            fee,
        );

        out
    }

    // Estimates the result of a swap operation.
    //
    // # Arguments
    //
    // * `in_idx` - The index of the input token to be swapped.
    // * `out_idx` - The index of the output token to be received.
    // * `in_amount` - The amount of the input token to be swapped.
    //
    // # Returns
    //
    // The estimated amount of the output token that would be received.
    fn estimate_swap(e: Env, in_idx: u32, out_idx: u32, in_amount: u128) -> u128 {
        if in_idx == out_idx {
            panic_with_error!(&e, LiquidityPoolValidationError::CannotSwapSameToken);
        }

        if in_idx > 1 {
            panic_with_error!(&e, LiquidityPoolValidationError::InTokenOutOfBounds);
        }

        if out_idx > 1 {
            panic_with_error!(&e, LiquidityPoolValidationError::OutTokenOutOfBounds);
        }

        let reserve_a = get_reserve_a(&e);
        let reserve_b = get_reserve_b(&e);
        let reserves = Vec::from_array(&e, [reserve_a, reserve_b]);
        let reserve_sell = reserves.get(in_idx).unwrap();
        let reserve_buy = reserves.get(out_idx).unwrap();

        get_amount_out(&e, in_amount, reserve_sell, reserve_buy).0
    }

    // Withdraws tokens from the pool.
    //
    // # Arguments
    //
    // * `user` - The address of the user withdrawing the tokens.
    // * `share_amount` - The amount of pool tokens to burn.
    // * `min_amounts` - A vector of minimum amounts of each token to be received.
    //
    // # Returns
    //
    // A vector of actual amounts of each token withdrawn.
    fn withdraw(e: Env, user: Address, share_amount: u128, min_amounts: Vec<u128>) -> Vec<u128> {
        user.require_auth();

        if min_amounts.len() != 2 {
            panic_with_error!(&e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        // Before actual changes were made to the pool, update total rewards data and refresh user reward
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        let pool_data = rewards.manager().update_rewards_data(total_shares);
        rewards
            .manager()
            .update_user_reward(&pool_data, &user, user_shares);
        rewards.storage().bump_user_reward_data(&user);

        // First transfer the pool shares that need to be redeemed
        let share_token_client = SorobanTokenClient::new(&e, &get_token_share(&e));
        share_token_client.transfer(
            &user,
            &e.current_contract_address(),
            &(share_amount as i128),
        );

        let (reserve_a, reserve_b) = (get_reserve_a(&e), get_reserve_b(&e));
        let balance_shares = get_balance_shares(&e);
        let total_shares = get_total_shares(&e);

        // Now calculate the withdraw amounts
        let out_a = reserve_a.fixed_mul_floor(&e, &balance_shares, &total_shares);
        let out_b = reserve_b.fixed_mul_floor(&e, &balance_shares, &total_shares);

        let min_a = min_amounts.get(0).unwrap();
        let min_b = min_amounts.get(1).unwrap();

        if out_a < min_a || out_b < min_b {
            panic_with_error!(&e, LiquidityPoolValidationError::OutMinNotSatisfied);
        }

        burn_shares(&e, balance_shares as i128);
        transfer_a(&e, user.clone(), out_a);
        transfer_b(&e, user, out_b);
        put_reserve_a(&e, reserve_a - out_a);
        put_reserve_b(&e, reserve_b - out_b);

        // update plane data for every pool update
        update_plane(&e);

        let withdraw_amounts = Vec::from_array(&e, [out_a, out_b]);
        PoolEvents::new(&e).withdraw_liquidity(
            Self::get_tokens(e.clone()),
            withdraw_amounts.clone(),
            share_amount,
        );

        withdraw_amounts
    }

    // Returns the pool's reserves.
    //
    // # Returns
    //
    // A vector of the pool's reserves.
    fn get_reserves(e: Env) -> Vec<u128> {
        Vec::from_array(&e, [get_reserve_a(&e), get_reserve_b(&e)])
    }

    // Returns the pool's fee fraction.
    //
    // # Returns
    //
    // The pool's fee fraction as a u32.
    fn get_fee_fraction(e: Env) -> u32 {
        // returns fee fraction. 0.01% = 1; 1% = 100; 0.3% = 30
        get_fee_fraction(&e)
    }

    // Returns information about the pool.
    //
    // # Returns
    //
    // A map of Symbols to Vals representing the pool's information.
    fn get_info(e: Env) -> Map<Symbol, Val> {
        let fee = get_fee_fraction(&e);
        let pool_type = Self::pool_type(e.clone());
        let mut result = Map::new(&e);
        result.set(symbol_short!("pool_type"), pool_type.into_val(&e));
        result.set(symbol_short!("fee"), fee.into_val(&e));
        result
    }
}

#[contractimpl]
impl AdminInterfaceTrait for LiquidityPool {
    // Stops the pool deposits instantly.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn kill_deposit(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        set_is_killed_deposit(&e, &true);
        PoolEvents::new(&e).kill_deposit();
    }

    // Stops the pool swaps instantly.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn kill_swap(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        set_is_killed_swap(&e, &true);
        PoolEvents::new(&e).kill_swap();
    }

    // Stops the pool claims instantly.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn kill_claim(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        set_is_killed_claim(&e, &true);
        PoolEvents::new(&e).kill_claim();
    }

    // Resumes the pool deposits.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn unkill_deposit(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        set_is_killed_deposit(&e, &false);
        PoolEvents::new(&e).unkill_deposit();
    }

    // Resumes the pool swaps.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn unkill_swap(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        set_is_killed_swap(&e, &false);
        PoolEvents::new(&e).unkill_swap();
    }

    // Resumes the pool claims.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn unkill_claim(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        set_is_killed_claim(&e, &false);
        PoolEvents::new(&e).unkill_claim();
    }

    // Get deposit killswitch status.
    fn get_is_killed_deposit(e: Env) -> bool {
        get_is_killed_deposit(&e)
    }

    // Get swap killswitch status.
    fn get_is_killed_swap(e: Env) -> bool {
        get_is_killed_swap(&e)
    }

    // Get claim killswitch status.
    fn get_is_killed_claim(e: Env) -> bool {
        get_is_killed_claim(&e)
    }
}

#[contractimpl]
impl UpgradeableContractTrait for LiquidityPool {
    // Returns the version of the contract.
    //
    // # Returns
    //
    // The version of the contract as a u32.
    fn version() -> u32 {
        105
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

#[contractimpl]
impl RewardsTrait for LiquidityPool {
    // Initializes the rewards configuration.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `reward_token` - The address of the reward token.
    fn initialize_rewards_config(e: Env, reward_token: Address) {
        let rewards = get_rewards_manager(&e);
        if rewards.storage().has_reward_token() {
            panic_with_error!(&e, LiquidityPoolError::RewardsAlreadyInitialized);
        }

        rewards.storage().put_reward_token(reward_token);
    }

    // Sets the rewards configuration.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `admin` - The address of the admin user.
    // * `expired_at` - The timestamp when the rewards expire.
    // * `tps` - The value with 7 decimal places. Example: 600_0000000
    fn set_rewards_config(
        e: Env,
        admin: Address,
        expired_at: u64, // timestamp
        tps: u128,       // value with 7 decimal places. example: 600_0000000
    ) {
        admin.require_auth();

        // either admin or router can set the rewards config
        if admin != get_router(&e) {
            AccessControl::new(&e).check_admin(&admin);
        }

        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        rewards
            .manager()
            .set_reward_config(total_shares, expired_at, tps);
    }

    // Returns the rewards information:
    //     tps, total accumulated amount for user, expiration, amount available to claim, debug info.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `user` - The address of the user.
    //
    // # Returns
    //
    // A map of Symbols to i128 representing the rewards information.
    fn get_rewards_info(e: Env, user: Address) -> Map<Symbol, i128> {
        let rewards = get_rewards_manager(&e);
        let config = rewards.storage().get_pool_reward_config();
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        let pool_data = rewards.manager().update_rewards_data(total_shares);
        let user_data = rewards
            .manager()
            .update_user_reward(&pool_data, &user, user_shares);
        let mut result = Map::new(&e);
        result.set(symbol_short!("tps"), config.tps as i128);
        result.set(symbol_short!("exp_at"), config.expired_at as i128);
        result.set(symbol_short!("acc"), pool_data.accumulated as i128);
        result.set(symbol_short!("last_time"), pool_data.last_time as i128);
        result.set(
            symbol_short!("pool_acc"),
            user_data.pool_accumulated as i128,
        );
        result.set(symbol_short!("block"), pool_data.block as i128);
        result.set(symbol_short!("usr_block"), user_data.last_block as i128);
        result.set(symbol_short!("to_claim"), user_data.to_claim as i128);
        result
    }

    // Returns the amount of reward tokens available for the user to claim.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `user` - The address of the user.
    //
    // # Returns
    //
    // The amount of reward tokens available for the user to claim as a u128.
    fn get_user_reward(e: Env, user: Address) -> u128 {
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        rewards
            .manager()
            .get_amount_to_claim(&user, total_shares, user_shares)
    }

    // Returns the total amount of accumulated reward for the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    //
    // # Returns
    //
    // The total amount of accumulated reward for the pool as a u128.
    fn get_total_accumulated_reward(e: Env) -> u128 {
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        rewards.manager().get_total_accumulated_reward(total_shares)
    }

    // Returns the total amount of configured reward for the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    //
    // # Returns
    //
    // The total amount of configured reward for the pool as a u128.
    fn get_total_configured_reward(e: Env) -> u128 {
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        rewards.manager().get_total_configured_reward(total_shares)
    }

    // Returns the total amount of claimed reward for the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    //
    // # Returns
    //
    // The total amount of claimed reward for the pool as a u128.
    fn get_total_claimed_reward(e: Env) -> u128 {
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        rewards.manager().get_total_claimed_reward(total_shares)
    }

    // Claims the reward as a user.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `user` - The address of the user.
    //
    // # Returns
    //
    // The amount of tokens rewarded to the user as a u128.
    fn claim(e: Env, user: Address) -> u128 {
        if get_is_killed_claim(&e) {
            panic_with_error!(e, LiquidityPoolError::PoolClaimKilled);
        }

        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        let reward = rewards
            .manager()
            .claim_reward(&user, total_shares, user_shares);
        rewards.storage().bump_user_reward_data(&user);
        reward
    }
}

#[contractimpl]
impl Plane for LiquidityPool {
    // Sets the plane for the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `plane` - The address of the plane.
    //
    // # Panics
    //
    // If the plane has already been initialized.
    fn init_pools_plane(e: Env, plane: Address) {
        if has_plane(&e) {
            panic_with_error!(&e, LiquidityPoolError::PlaneAlreadyInitialized);
        }

        set_plane(&e, &plane);
    }

    fn set_pools_plane(e: Env, admin: Address, plane: Address) {
        let access_control = AccessControl::new(&e);
        admin.require_auth();
        access_control.check_admin(&admin);

        set_plane(&e, &plane);
    }

    // Returns the plane of the pool.
    //
    // # Arguments
    //
    // * `e` - The environment.
    //
    // # Returns
    //
    // The address of the plane.
    fn get_pools_plane(e: Env) -> Address {
        get_plane(&e)
    }

    // Updates the plane data in case the plane contract was updated.
    fn backfill_plane_data(e: Env) {
        update_plane(&e);
    }
}
