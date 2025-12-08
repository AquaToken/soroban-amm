use crate::constants::FEE_MULTIPLIER;
use crate::errors::LiquidityPoolError;
use crate::plane::update_plane;
use crate::plane_interface::Plane;
use crate::pool;
use crate::pool::{get_amount_out, get_amount_out_strict_receive};
use crate::pool_interface::{
    AdminInterfaceTrait, LiquidityPoolCrunch, LiquidityPoolTrait, RewardsTrait, UpgradeableContract,
};
use crate::rewards::get_rewards_manager;
use crate::storage::{
    get_fee_fraction, get_gauge_future_wasm, get_is_killed_claim, get_is_killed_deposit,
    get_is_killed_swap, get_plane, get_protocol_fee_a, get_protocol_fee_b,
    get_protocol_fee_fraction, get_reserve_a, get_reserve_b, get_router, get_token_a, get_token_b,
    get_token_future_wasm, has_plane, put_fee_fraction, put_reserve_a, put_reserve_b, put_token_a,
    put_token_b, set_gauge_future_wasm, set_is_killed_claim, set_is_killed_deposit,
    set_is_killed_swap, set_plane, set_protocol_fee_a, set_protocol_fee_b,
    set_protocol_fee_fraction, set_router, set_token_future_wasm,
};
use crate::token::{create_contract, transfer_a, transfer_b};
use access_control::access::{AccessControl, AccessControlTrait};
use access_control::emergency::{get_emergency_mode, set_emergency_mode};
use access_control::errors::AccessControlError;
use access_control::events::Events as AccessControlEvents;
use access_control::interface::TransferableContract;
use access_control::management::{MultipleAddressesManagementTrait, SingleAddressManagementTrait};
use access_control::role::Role;
use access_control::role::SymbolRepresentation;
use access_control::transfer::TransferOwnershipTrait;
use access_control::utils::{
    require_operations_admin_or_owner, require_pause_admin_or_owner,
    require_pause_or_emergency_pause_admin_or_owner, require_rewards_admin_or_owner,
    require_system_fee_admin_or_owner,
};
use liqidity_pool_rewards_gauge as rewards_gauge;
use liqidity_pool_rewards_gauge::interface::RewardsGaugeInterface;
use liquidity_pool_config_storage as config_storage;
use liquidity_pool_config_storage::interface::ConfigStorageInterface;
use liquidity_pool_events::Events as PoolEvents;
use liquidity_pool_events::LiquidityPoolEvents;
use liquidity_pool_validation_errors::LiquidityPoolValidationError;
use rewards::events::Events as RewardEvents;
use rewards::storage::{
    BoostFeedStorageTrait, BoostTokenStorageTrait, PoolRewardsStorageTrait, RewardTokenStorageTrait,
};
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{
    contract, contractimpl, contractmeta, panic_with_error, symbol_short, Address, BytesN, Env,
    IntoVal, Map, Symbol, Val, Vec, U256,
};
use token_share::{
    burn_shares, get_token_share, get_total_shares, get_user_balance_shares, mint_shares,
    put_token_share, Client as LPTokenClient,
};
use upgrade::events::Events as UpgradeEvents;
use upgrade::{apply_upgrade, commit_upgrade, revert_upgrade};
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
    // * `privileged_addrs` - (
    //      emergency admin,
    //      rewards admin,
    //      operations admin,
    //      pause admin,
    //      emergency pause admins,
    //      system fee admin,
    //  ).
    // * `router` - The address of the router.
    // * `lp_token_wasm_hash` - The hash of the liquidity pool token contract.
    // * `tokens` - A vector of token addresses.
    // * `fees_config` - (
    //      `fee_fraction` - The fee fraction for the pool.
    //      `protocol_fee_fraction` - The protocol fee fraction for the pool.
    //  )
    // * `reward_config` - (
    // *    `reward_token` - The address of the reward token.
    // *    `reward_boost_token` - The address of the reward boost token.
    // *    `reward_boost_feed` - The address of the reward boost feed.
    // * )
    // * `plane` - The address of the plane.
    // * `config_storage` - The address of the configuration storage.
    fn initialize_all(
        e: Env,
        admin: Address,
        privileged_addrs: (Address, Address, Address, Address, Vec<Address>, Address),
        router: Address,
        lp_token_wasm_hash: BytesN<32>,
        tokens: Vec<Address>,
        fees_config: (u32, u32),
        reward_config: (Address, Address, Address),
        plane: Address,
        config_storage: Address,
    ) {
        let (reward_token, reward_boost_token, reward_boost_feed) = reward_config;

        // merge whole initialize process into one because lack of caching of VM components
        // https://github.com/stellar/rs-soroban-env/issues/827
        config_storage::operations::init_config_storage(&e, &config_storage);
        Self::init_pools_plane(e.clone(), plane);
        Self::initialize(
            e.clone(),
            admin,
            privileged_addrs,
            router,
            lp_token_wasm_hash,
            tokens,
            fees_config,
        );
        Self::initialize_boost_config(e.clone(), reward_boost_token, reward_boost_feed);
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
    // * `privileged_addrs` - (
    //      emergency admin,
    //      rewards admin,
    //      operations admin,
    //      pause admin,
    //      emergency pause admins
    //      system fee admin,
    //  ).
    // * `router` - The address of the router.
    // * `lp_token_wasm_hash` - The hash of the liquidity pool token contract.
    // * `tokens` - A vector of token addresses.
    // * `fees_config` - (
    //      `fee_fraction` - The fee fraction for the pool.
    //      `protocol_fee_fraction` - The protocol fee fraction for the pool.
    //  )
    fn initialize(
        e: Env,
        admin: Address,
        privileged_addrs: (Address, Address, Address, Address, Vec<Address>, Address),
        router: Address,
        lp_token_wasm_hash: BytesN<32>,
        tokens: Vec<Address>,
        fees_config: (u32, u32),
    ) {
        let (fee_fraction, protocol_fee_fraction) = fees_config;
        let access_control = AccessControl::new(&e);
        if access_control.get_role_safe(&Role::Admin).is_some() {
            panic_with_error!(&e, LiquidityPoolError::AlreadyInitialized);
        }
        access_control.set_role_address(&Role::Admin, &admin);
        access_control.set_role_address(&Role::EmergencyAdmin, &privileged_addrs.0);
        access_control.set_role_address(&Role::RewardsAdmin, &privileged_addrs.1);
        access_control.set_role_address(&Role::OperationsAdmin, &privileged_addrs.2);
        access_control.set_role_address(&Role::PauseAdmin, &privileged_addrs.3);
        access_control.set_role_addresses(&Role::EmergencyPauseAdmin, &privileged_addrs.4);
        access_control.set_role_address(&Role::SystemFeeAdmin, &privileged_addrs.5);

        set_router(&e, &router);

        if tokens.len() != 2 {
            panic_with_error!(&e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        let token_a = tokens.get(0).unwrap();
        let token_b = tokens.get(1).unwrap();

        let share_contract = create_contract(&e, lp_token_wasm_hash, &token_a, &token_b);
        LPTokenClient::new(&e, &share_contract).initialize(
            &e.current_contract_address(),
            &7u32,
            &"Pool Share Token".into_val(&e),
            &"POOL".into_val(&e),
        );

        // 0.01% = 1; 1% = 100; 0.3% = 30
        if fee_fraction as u128 > FEE_MULTIPLIER - 1 {
            panic_with_error!(&e, LiquidityPoolValidationError::FeeOutOfBounds);
        }
        if protocol_fee_fraction as u128 > FEE_MULTIPLIER - 1 {
            panic_with_error!(&e, LiquidityPoolValidationError::FeeOutOfBounds);
        }
        put_fee_fraction(&e, fee_fraction);
        set_protocol_fee_fraction(&e, &protocol_fee_fraction);

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
        let mut rewards_manager = rewards.manager();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            rewards_manager.get_working_balance(&user, user_shares),
            rewards_manager.get_working_supply(total_shares),
        );
        rewards_manager.checkpoint_user(&user, total_shares, user_shares);

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
        put_reserve_b(&e, reserve_b + amounts.1);

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
        mint_shares(&e, &user, shares_to_mint as i128);
        put_reserve_a(&e, new_reserve_a);
        put_reserve_b(&e, new_reserve_b);

        // Checkpoint resulting working balance
        let mut rewards_manager = rewards.manager();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            // working balance/supply were initialized, so we don't need defaults here
            rewards_manager.get_working_balance(&user, 0),
            rewards_manager.get_working_supply(0),
        );
        rewards_manager.update_working_balance(
            &user,
            new_total_shares,
            user_shares + shares_to_mint,
        );

        // update plane data for every pool update
        update_plane(&e);

        let amounts_vec = Vec::from_array(&e, [amounts.0, amounts.1]);
        PoolEvents::new(&e).deposit_liquidity(
            Self::get_tokens(e.clone()),
            amounts_vec.clone(),
            shares_to_mint,
        );
        PoolEvents::new(&e).update_reserves(Vec::from_array(&e, [new_reserve_a, new_reserve_b]));

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

        let (out, total_fee) = get_amount_out(&e, in_amount, reserve_sell, reserve_buy);
        let protocol_fee = total_fee * get_protocol_fee_fraction(&e) as u128 / FEE_MULTIPLIER;
        let lp_fee = total_fee - protocol_fee;

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

        let (mut new_reserve_a, mut new_reserve_b) = (get_reserve_a(&e), get_reserve_b(&e));

        // residue_numerator and residue_denominator are the amount that the invariant considers after
        // deducting the fee, scaled up by FEE_MULTIPLIER to avoid fractions
        let base_fee_fraction = get_fee_fraction(&e) as u128; // e.g. 30 = 0.3%
        let protocol_fee_frac =
            base_fee_fraction * get_protocol_fee_fraction(&e) as u128 / FEE_MULTIPLIER; // e.g. 30 * 50 / 100 = 0.15% admin fee
        let pool_fee_frac = base_fee_fraction - protocol_fee_frac; // e.g. 15 = 0.15% stays in pool
        let residue_numerator = FEE_MULTIPLIER - pool_fee_frac; // e.g. 10000 - 15  = 9985
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
            transfer_a(&e, &user, out_a);
            new_reserve_a = new_reserve_a - out_a;
            new_reserve_b = new_reserve_b - protocol_fee;
            set_protocol_fee_b(&e, &(get_protocol_fee_b(&e) + protocol_fee));
        } else {
            transfer_b(&e, &user, out_b);
            new_reserve_a = new_reserve_a - protocol_fee;
            new_reserve_b = new_reserve_b - out_b;
            set_protocol_fee_a(&e, &(get_protocol_fee_a(&e) + protocol_fee));
        }
        put_reserve_a(&e, new_reserve_a);
        put_reserve_b(&e, new_reserve_b);

        // update plane data for every pool update
        update_plane(&e);

        PoolEvents::new(&e).trade(
            user,
            sell_token,
            tokens.get(out_idx).unwrap(),
            in_amount,
            out,
            lp_fee,
        );
        PoolEvents::new(&e).update_reserves(Vec::from_array(&e, [new_reserve_a, new_reserve_b]));

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

    // Swaps tokens in the pool.
    // Perform an exchange between two coins with strict amount to receive.
    //
    // # Arguments
    //
    // * `user` - The address of the user swapping the tokens.
    // * `in_idx` - Index value for the coin to send
    // * `out_idx` - Index value of the coin to receive
    // * `out_amount` - Amount of out_idx being exchanged
    // * `in_max` - Maximum amount of in_idx to send
    //
    // # Returns
    //
    // The amount of the input token sent.
    fn swap_strict_receive(
        e: Env,
        user: Address,
        in_idx: u32,
        out_idx: u32,
        out_amount: u128,
        in_max: u128,
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

        if out_amount == 0 {
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

        let (in_amount, total_fee) =
            get_amount_out_strict_receive(&e, out_amount, reserve_sell, reserve_buy);

        if in_amount > in_max {
            panic_with_error!(&e, LiquidityPoolValidationError::InMaxNotSatisfied);
        }

        // Transfer the amount being sold to the contract
        let sell_token = tokens.get(in_idx).unwrap();
        let sell_token_client = SorobanTokenClient::new(&e, &sell_token);
        sell_token_client.transfer(&user, &e.current_contract_address(), &(in_max as i128));

        // Return the difference
        sell_token_client.transfer(
            &e.current_contract_address(),
            &user,
            &((in_max - in_amount) as i128),
        );

        if in_idx == 0 {
            put_reserve_a(&e, reserve_a + in_amount);
        } else {
            put_reserve_b(&e, reserve_b + in_amount);
        }

        let (mut new_reserve_a, mut new_reserve_b) = (get_reserve_a(&e), get_reserve_b(&e));

        // residue_numerator and residue_denominator are the amount that the invariant considers after
        // deducting the fee, scaled up by FEE_MULTIPLIER to avoid fractions
        let base_fee_fraction = get_fee_fraction(&e) as u128; // e.g. 30 = 0.3%
        let protocol_fee_frac =
            base_fee_fraction * get_protocol_fee_fraction(&e) as u128 / FEE_MULTIPLIER; // e.g. 30 * 5000 / 10000 = 0.15% admin fee
        let pool_fee_frac = base_fee_fraction - protocol_fee_frac; // e.g. 15 = 0.15% stays in pool
        let residue_numerator = FEE_MULTIPLIER - pool_fee_frac; // e.g. 10000 - 15  = 9985
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

        let (out_a, out_b) = if out_idx == 0 {
            (out_amount, 0)
        } else {
            (0, out_amount)
        };

        let new_inv_a = new_invariant_factor(new_reserve_a, reserve_a, out_a);
        let new_inv_b = new_invariant_factor(new_reserve_b, reserve_b, out_b);
        let old_inv_a = residue_denominator.mul(&U256::from_u128(&e, reserve_a));
        let old_inv_b = residue_denominator.mul(&U256::from_u128(&e, reserve_b));

        if new_inv_a.mul(&new_inv_b) < old_inv_a.mul(&old_inv_b) {
            panic_with_error!(&e, LiquidityPoolError::InvariantDoesNotHold);
        }

        // collect protocol_fee on input side
        let protocol_fee = total_fee * get_protocol_fee_fraction(&e) as u128 / FEE_MULTIPLIER;
        let lp_fee = total_fee - protocol_fee;

        // give trader the exact out_amount
        if out_idx == 0 {
            transfer_a(&e, &user, out_amount);
            new_reserve_a = new_reserve_a - out_amount;
            new_reserve_b = new_reserve_b - protocol_fee;
            set_protocol_fee_b(&e, &(get_protocol_fee_b(&e) + protocol_fee));
        } else {
            transfer_b(&e, &user, out_amount);
            new_reserve_a = new_reserve_a - protocol_fee;
            new_reserve_b = new_reserve_b - out_amount;
            set_protocol_fee_a(&e, &(get_protocol_fee_a(&e) + protocol_fee));
        }
        put_reserve_a(&e, new_reserve_a);
        put_reserve_b(&e, new_reserve_b);

        // update plane data for every pool update
        update_plane(&e);

        PoolEvents::new(&e).trade(
            user,
            sell_token,
            tokens.get(out_idx).unwrap(),
            in_amount,
            out_amount,
            lp_fee,
        );
        PoolEvents::new(&e).update_reserves(Vec::from_array(&e, [new_reserve_a, new_reserve_b]));

        in_amount
    }

    // Estimates the result of a swap_strict_receive operation.
    //
    // # Arguments
    //
    // * `in_idx` - The index of the input token to be swapped.
    // * `out_idx` - The index of the output token to be received.
    // * `out_amount` - The amount of the output token to be received.
    //
    // # Returns
    //
    // The estimated amount of the output token that would be received.
    fn estimate_swap_strict_receive(e: Env, in_idx: u32, out_idx: u32, out_amount: u128) -> u128 {
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

        get_amount_out_strict_receive(&e, out_amount, reserve_sell, reserve_buy).0
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
        let mut rewards_manager = rewards.manager();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            rewards_manager.get_working_balance(&user, user_shares),
            rewards_manager.get_working_supply(total_shares),
        );
        rewards_manager.checkpoint_user(&user, total_shares, user_shares);

        burn_shares(&e, &user, share_amount);

        let (reserve_a, reserve_b) = (get_reserve_a(&e), get_reserve_b(&e));

        // Now calculate the withdraw amounts
        let out_a = reserve_a.fixed_mul_floor(&e, &share_amount, &total_shares);
        let out_b = reserve_b.fixed_mul_floor(&e, &share_amount, &total_shares);

        let min_a = min_amounts.get(0).unwrap();
        let min_b = min_amounts.get(1).unwrap();

        if out_a < min_a || out_b < min_b {
            panic_with_error!(&e, LiquidityPoolValidationError::OutMinNotSatisfied);
        }

        transfer_a(&e, &user, out_a);
        transfer_b(&e, &user, out_b);
        let new_reserve_a = reserve_a - out_a;
        let new_reserve_b = reserve_b - out_b;
        put_reserve_a(&e, new_reserve_a);
        put_reserve_b(&e, new_reserve_b);

        // Checkpoint resulting working balance
        let mut rewards_manager = rewards.manager();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            rewards_manager.get_working_balance(&user, user_shares - share_amount),
            rewards_manager.get_working_supply(total_shares - share_amount),
        );
        rewards_manager.update_working_balance(
            &user,
            total_shares - share_amount,
            user_shares - share_amount,
        );

        // update plane data for every pool update
        update_plane(&e);

        let withdraw_amounts = Vec::from_array(&e, [out_a, out_b]);
        PoolEvents::new(&e).withdraw_liquidity(
            Self::get_tokens(e.clone()),
            withdraw_amounts.clone(),
            share_amount,
        );
        PoolEvents::new(&e).update_reserves(Vec::from_array(&e, [new_reserve_a, new_reserve_b]));

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

    // Returns part of the fee that goes to the protocol, 5000 = 50% of the fee goes to the protocol
    //
    // # Returns
    //
    // The pool's protocol fee fraction as a u32.
    fn get_protocol_fee_fraction(e: Env) -> u32 {
        get_protocol_fee_fraction(&e)
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
    // Sets the privileged addresses.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `rewards_admin` - The address of the rewards admin.
    // * `operations_admin` - The address of the operations admin.
    // * `pause_admin` - The address of the pause admin.
    // * `emergency_pause_admin` - The addresses of the emergency pause admins.
    // * `system_fee_admin` - The address of the system fee admin.
    fn set_privileged_addrs(
        e: Env,
        admin: Address,
        rewards_admin: Address,
        operations_admin: Address,
        pause_admin: Address,
        emergency_pause_admins: Vec<Address>,
        system_fee_admin: Address,
    ) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, &Role::Admin);

        access_control.set_role_address(&Role::RewardsAdmin, &rewards_admin);
        access_control.set_role_address(&Role::OperationsAdmin, &operations_admin);
        access_control.set_role_address(&Role::PauseAdmin, &pause_admin);
        access_control.set_role_addresses(&Role::EmergencyPauseAdmin, &emergency_pause_admins);
        access_control.set_role_address(&Role::SystemFeeAdmin, &system_fee_admin);
        AccessControlEvents::new(&e).set_privileged_addrs(
            rewards_admin,
            operations_admin,
            pause_admin,
            emergency_pause_admins,
            system_fee_admin,
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
            Role::SystemFeeAdmin,
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

    // Stops the pool deposits instantly.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn kill_deposit(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);

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
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);

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
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);

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
        require_pause_admin_or_owner(&e, &admin);

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
        require_pause_admin_or_owner(&e, &admin);

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
        require_pause_admin_or_owner(&e, &admin);

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

    // Sets the protocol fraction of total fee for the pool.
    fn set_protocol_fee_fraction(e: Env, admin: Address, new_fraction: u32) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);

        if new_fraction as u128 > FEE_MULTIPLIER {
            panic_with_error!(e, LiquidityPoolValidationError::FeeOutOfBounds);
        }

        set_protocol_fee_fraction(&e, &new_fraction);
        PoolEvents::new(&e).set_protocol_fee_fraction(new_fraction);
    }

    // Returns the protocol fees accumulated in the pool.
    fn get_protocol_fees(e: Env) -> Vec<u128> {
        Vec::from_array(&e, [get_protocol_fee_a(&e), get_protocol_fee_b(&e)])
    }

    // Claims the protocol fees accumulated in the pool.
    fn claim_protocol_fees(e: Env, admin: Address, destination: Address) -> Vec<u128> {
        admin.require_auth();
        require_system_fee_admin_or_owner(&e, &admin);

        let token_a = get_token_a(&e);
        let token_b = get_token_b(&e);

        let fee_a = get_protocol_fee_a(&e);
        let fee_b = get_protocol_fee_b(&e);

        if fee_a == 0 && fee_b == 0 {
            return Vec::from_array(&e, [0, 0]);
        }

        if fee_a > 0 {
            SorobanTokenClient::new(&e, &token_a).transfer(
                &e.current_contract_address(),
                &destination,
                &(fee_a as i128),
            );
            set_protocol_fee_a(&e, &0);
            PoolEvents::new(&e).claim_protocol_fee(token_a, destination.clone(), fee_a);
        }
        if fee_b > 0 {
            SorobanTokenClient::new(&e, &token_b).transfer(
                &e.current_contract_address(),
                &destination,
                &(fee_b as i128),
            );
            set_protocol_fee_b(&e, &0);
            PoolEvents::new(&e).claim_protocol_fee(token_b, destination, fee_b);
        }

        Vec::from_array(&e, [fee_a, fee_b])
    }
}

// The `UpgradeableContract` trait provides the interface for upgrading the contract.
#[contractimpl]
impl UpgradeableContract for LiquidityPool {
    // Returns the version of the contract.
    //
    // # Returns
    //
    // The version of the contract as a u32.
    fn version() -> u32 {
        170
    }

    // Get contract type symbolic name
    fn contract_name(e: Env) -> Symbol {
        Symbol::new(&e, "StandardLiquidityPool")
    }

    // Commits a new wasm hash for a future upgrade.
    // The upgrade will be available through `apply_upgrade` after the standard upgrade delay
    // unless the system is in emergency mode.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `new_wasm_hash` - The new wasm hash to commit.
    // * `new_token_wasm_hash` - The new token wasm hash to commit.
    // * `gauges_new_wasm_hash` - The new rewards gauge wasm hash to commit.
    fn commit_upgrade(
        e: Env,
        admin: Address,
        new_wasm_hash: BytesN<32>,
        token_new_wasm_hash: BytesN<32>,
        gauges_new_wasm_hash: BytesN<32>,
    ) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        commit_upgrade(&e, &new_wasm_hash);
        // handle dependent contracts upgrade together with pool upgrade
        set_token_future_wasm(&e, &token_new_wasm_hash);
        set_gauge_future_wasm(&e, &gauges_new_wasm_hash);

        UpgradeEvents::new(&e).commit_upgrade(Vec::from_array(
            &e,
            [new_wasm_hash, token_new_wasm_hash, gauges_new_wasm_hash],
        ));
    }

    // Applies the committed upgrade.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn apply_upgrade(e: Env, admin: Address) -> (BytesN<32>, BytesN<32>) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        let new_wasm_hash = apply_upgrade(&e);
        let token_new_wasm_hash = get_token_future_wasm(&e);
        token_share::Client::new(&e, &get_token_share(&e))
            .upgrade(&e.current_contract_address(), &token_new_wasm_hash);
        rewards_gauge::operations::upgrade(&e, &get_gauge_future_wasm(&e));

        UpgradeEvents::new(&e).apply_upgrade(Vec::from_array(
            &e,
            [new_wasm_hash.clone(), token_new_wasm_hash.clone()],
        ));

        (new_wasm_hash, token_new_wasm_hash)
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

    fn initialize_boost_config(e: Env, reward_boost_token: Address, reward_boost_feed: Address) {
        let rewards_storage = get_rewards_manager(&e).storage();
        if rewards_storage.has_reward_boost_token() {
            panic_with_error!(&e, LiquidityPoolError::RewardsAlreadyInitialized);
        }

        rewards_storage.put_reward_boost_token(reward_boost_token);
        rewards_storage.put_reward_boost_feed(reward_boost_feed);
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

        // rewards admin, owner and router are privileged to set the rewards config
        if admin != get_router(&e) {
            require_rewards_admin_or_owner(&e, &admin);
        }

        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        rewards
            .manager()
            .set_reward_config(total_shares, expired_at, tps);
        RewardEvents::new(&e).set_rewards_config(expired_at, tps);
    }

    // Get difference between the actual balance and the total unclaimed reward minus the reserves
    fn get_unused_reward(e: Env) -> u128 {
        let rewards = get_rewards_manager(&e);
        let mut rewards_manager = rewards.manager();
        let total_shares = get_total_shares(&e);
        let mut reward_balance_to_keep = rewards_manager.get_total_configured_reward(total_shares)
            - rewards_manager.get_total_claimed_reward(total_shares);

        let reward_token = rewards.storage().get_reward_token();
        let reward_balance = SorobanTokenClient::new(&e, &reward_token)
            .balance(&e.current_contract_address()) as u128;

        match Self::get_tokens(e.clone()).first_index_of(reward_token) {
            Some(idx) => {
                // since reward token is in the reserves, we need to keep also the reserves value
                reward_balance_to_keep += Self::get_reserves(e.clone()).get(idx).unwrap();
            }
            None => {}
        };

        if reward_balance > reward_balance_to_keep {
            reward_balance - reward_balance_to_keep
        } else {
            // balance is not sufficient, no surplus
            0
        }
    }

    // Return reward token above the configured amount back to router
    fn return_unused_reward(e: Env, admin: Address) -> u128 {
        admin.require_auth();
        require_rewards_admin_or_owner(&e, &admin);

        let unused_reward = Self::get_unused_reward(e.clone());

        if unused_reward == 0 {
            return 0;
        }

        let reward_token = get_rewards_manager(&e).storage().get_reward_token();
        SorobanTokenClient::new(&e, &reward_token).transfer(
            &e.current_contract_address(),
            &get_router(&e),
            &(unused_reward as i128),
        );
        unused_reward
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
        let mut manager = rewards.manager();
        let storage = rewards.storage();
        let config = storage.get_pool_reward_config();
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);

        // pre-fill result dict with stored values
        // or values won't be affected by checkpoint in any way
        let mut result = Map::from_array(
            &e,
            [
                (symbol_short!("tps"), config.tps as i128),
                (symbol_short!("exp_at"), config.expired_at as i128),
                (
                    symbol_short!("state"),
                    manager.get_user_rewards_state(&user) as i128,
                ),
                (symbol_short!("supply"), total_shares as i128),
                (
                    Symbol::new(&e, "working_balance"),
                    manager.get_working_balance(&user, user_shares) as i128,
                ),
                (
                    Symbol::new(&e, "working_supply"),
                    // it's safe to use total_shares here since it's just for initial value
                    manager.get_working_supply(total_shares) as i128,
                ),
                (
                    Symbol::new(&e, "boost_balance"),
                    manager.get_user_boost_balance(&user) as i128,
                ),
                (
                    Symbol::new(&e, "boost_supply"),
                    manager.get_total_locked() as i128,
                ),
            ],
        );

        // display actual values
        let user_data = manager.checkpoint_user(&user, total_shares, user_shares);
        let pool_data = storage.get_pool_reward_data();

        result.set(symbol_short!("acc"), pool_data.accumulated as i128);
        result.set(symbol_short!("last_time"), pool_data.last_time as i128);
        result.set(
            symbol_short!("pool_acc"),
            user_data.pool_accumulated as i128,
        );
        result.set(symbol_short!("block"), pool_data.block as i128);
        result.set(symbol_short!("usr_block"), user_data.last_block as i128);
        result.set(symbol_short!("to_claim"), user_data.to_claim as i128);

        // provide updated working balance information. if working_balance_new is bigger
        // than working_balance, it means that user has locked some tokens
        // and needs to checkpoint itself for more rewards
        result.set(
            Symbol::new(&e, "new_working_balance"),
            manager.get_working_balance(&user, user_shares) as i128,
        );
        result.set(
            Symbol::new(&e, "new_working_supply"),
            manager.get_working_supply(total_shares) as i128,
        );
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

    fn checkpoint_reward(e: Env, token_contract: Address, user: Address, user_shares: u128) {
        // checkpoint reward with provided values to avoid re-entrancy issue
        token_contract.require_auth();
        if token_contract != get_token_share(&e) {
            panic_with_error!(&e, AccessControlError::Unauthorized);
        }
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let mut rewards_manager = rewards.manager();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            rewards_manager.get_working_balance(&user, user_shares),
            rewards_manager.get_working_supply(total_shares),
        );
        rewards_manager.checkpoint_user(&user, total_shares, user_shares);
    }

    fn checkpoint_working_balance(
        e: Env,
        token_contract: Address,
        user: Address,
        user_shares: u128,
    ) {
        // checkpoint working balance with provided values to avoid re-entrancy issue
        token_contract.require_auth();
        if token_contract != get_token_share(&e) {
            panic_with_error!(&e, AccessControlError::Unauthorized);
        }
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let mut rewards_manager = rewards.manager();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            rewards_manager.get_working_balance(&user, user_shares),
            rewards_manager.get_working_supply(total_shares),
        );
        rewards_manager.checkpoint_user(&user, total_shares, user_shares);
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

    // Adjusts the total accumulated reward for the pool in case of reward tokens permalock
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `admin` - The address of the admin user.
    // * `diff` - The difference to adjust the total accumulated reward by. Can be positive or negative.
    fn adjust_total_accumulated_reward(e: Env, admin: Address, diff: i128) {
        admin.require_auth();
        require_rewards_admin_or_owner(&e, &admin);
        get_rewards_manager(&e)
            .manager()
            .adjust_total_accumulated_reward(get_total_shares(&e), diff)
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
        let mut rewards_manager = rewards.manager();
        let rewards_storage = rewards.storage();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            rewards_manager.get_working_balance(&user, user_shares),
            rewards_manager.get_working_supply(total_shares),
        );
        let reward = rewards_manager.claim_reward(&user, total_shares, user_shares);

        // validate reserves after claim - they should be less than or equal to the balance
        let tokens = Self::get_tokens(e.clone());
        let reward_token = rewards_storage.get_reward_token();
        let reserves = Self::get_reserves(e.clone());
        let protocol_fees = Self::get_protocol_fees(e.clone());

        for i in 0..reserves.len() {
            let token = tokens.get(i).unwrap();
            if token != reward_token {
                continue;
            }

            let balance = SorobanTokenClient::new(&e, &tokens.get(i).unwrap())
                .balance(&e.current_contract_address()) as u128;
            if reserves.get_unchecked(i) + protocol_fees.get_unchecked(i) > balance {
                panic_with_error!(&e, LiquidityPoolValidationError::InsufficientBalance);
            }
        }

        RewardEvents::new(&e).claim(user, reward_token, reward);

        reward
    }

    fn get_reward_state(e: Env, user: Address) -> bool {
        get_rewards_manager(&e)
            .manager()
            .get_user_rewards_state(&user)
    }

    fn set_rewards_state(e: Env, user: Address, state: bool) {
        user.require_auth();

        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        let mut rewards_manager = get_rewards_manager(&e).manager();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            rewards_manager.get_working_balance(&user, user_shares),
            rewards_manager.get_working_supply(total_shares),
        );
        rewards_manager.set_user_rewards_state(&user, state);
        rewards_manager.checkpoint_user(&user, total_shares, user_shares);
    }

    fn admin_set_rewards_state(e: Env, admin: Address, user: Address, state: bool) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);

        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        let mut rewards_manager = get_rewards_manager(&e).manager();
        rewards_gauge::operations::checkpoint_user(
            &e,
            &user,
            rewards_manager.get_working_balance(&user, user_shares),
            rewards_manager.get_working_supply(total_shares),
        );
        rewards_manager.set_user_rewards_state(&user, state);
        rewards_manager.checkpoint_user(&user, total_shares, user_shares);
    }
}

#[contractimpl]
impl RewardsGaugeInterface for LiquidityPool {
    fn gauge_add(e: Env, admin: Address, gauge_address: Address) {
        admin.require_auth();

        // operations admin, owner and router are privileged to add gauges
        if admin != get_router(&e) {
            require_operations_admin_or_owner(&e, &admin);
        }

        rewards_gauge::operations::add(&e, gauge_address);
    }

    fn gauge_remove(e: Env, admin: Address, reward_token: Address) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);

        rewards_gauge::operations::remove(&e, reward_token);
    }

    fn gauge_schedule_reward(
        e: Env,
        router: Address,
        distributor: Address,
        gauge: Address,
        start_at: Option<u64>,
        duration: u64,
        tps: u128,
    ) {
        router.require_auth();
        distributor.require_auth();

        if router != get_router(&e) {
            panic_with_error!(e, AccessControlError::Unauthorized)
        }

        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let rewards_manager = rewards.manager();

        rewards_gauge::operations::schedule_rewards_config(
            &e,
            gauge,
            distributor,
            start_at,
            duration,
            tps,
            rewards_manager.get_working_supply(total_shares),
        );
    }

    fn kill_gauges_claim(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_or_emergency_pause_admin_or_owner(&e, &admin);

        rewards_gauge::operations::kill_claim(&e);
    }

    fn unkill_gauges_claim(e: Env, admin: Address) {
        admin.require_auth();
        require_pause_admin_or_owner(&e, &admin);

        rewards_gauge::operations::unkill_claim(&e);
    }

    fn get_gauges(e: Env) -> Map<Address, Address> {
        rewards_gauge::operations::list(&e)
    }

    fn gauges_claim(e: Env, user: Address) -> Map<Address, u128> {
        user.require_auth();

        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        let rewards_manager = rewards.manager();

        rewards_gauge::operations::claim(
            &e,
            &user,
            rewards_manager.get_working_balance(&user, user_shares),
            rewards_manager.get_working_supply(total_shares),
        )
    }

    fn gauges_get_reward_info(e: Env, user: Address) -> Map<Address, Map<Symbol, i128>> {
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        let rewards_manager = rewards.manager();

        rewards_gauge::operations::get_rewards_info(
            &e,
            &user,
            rewards_manager.get_working_balance(&user, user_shares),
            rewards_manager.get_working_supply(total_shares),
        )
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
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);

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

// The `TransferableContract` trait provides the interface for transferring ownership of the contract.
#[contractimpl]
impl TransferableContract for LiquidityPool {
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

#[contractimpl]
impl ConfigStorageInterface for LiquidityPool {
    fn init_config_storage(e: Env, admin: Address, config_storage: Address) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        config_storage::operations::init_config_storage(&e, &config_storage);
    }
}
