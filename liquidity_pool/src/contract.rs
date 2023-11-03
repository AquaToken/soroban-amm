use crate::admin::{check_admin, has_admin, require_admin, set_admin};
use crate::pool_interface::{LiquidityPoolTrait, RewardsTrait, UpgradeableContractTrait};
use crate::rewards::storage::get_pool_reward_config;
use crate::token::create_contract;
use crate::{pool, rewards, storage, token};
use cast::i128 as to_i128;
use num_integer::Roots;
use soroban_sdk::{contract, contractimpl, contractmeta, symbol_short, IntoVal, Vec};
use soroban_sdk::{Address, BytesN, Env, Map, Symbol};

// Metadata that is added on to the WASM custom section
contractmeta!(
    key = "Description",
    val = "Constant product AMM with configurable swap fee"
);

#[contract]
pub struct LiquidityPool;

#[contractimpl]
impl LiquidityPoolTrait for LiquidityPool {
    fn initialize(
        e: Env,
        admin: Address,
        lp_token_wasm_hash: BytesN<32>,
        tokens: Vec<Address>,
        fee_fraction: u32,
    ) {
        if has_admin(&e) {
            panic!("already initialized")
        }

        set_admin(&e, &admin);

        let token_a = tokens.get(0).unwrap();
        let token_b = tokens.get(1).unwrap();

        if token_a >= token_b {
            panic!("token_a must be less than token_b");
        }

        let share_contract = create_contract(&e, lp_token_wasm_hash, &token_a, &token_b);
        token::Client::new(&e, &share_contract).initialize(
            &e.current_contract_address(),
            &7u32,
            &"Pool Share Token".into_val(&e),
            &"POOL".into_val(&e),
        );

        // 0.01% = 1; 1% = 100; 0.3% = 30
        if fee_fraction > 9999 {
            panic!("fee cannot be equal or greater than 100%");
        }
        storage::put_fee_fraction(&e, fee_fraction);

        storage::put_token_a(&e, token_a);
        storage::put_token_b(&e, token_b);
        storage::put_token_share(&e, share_contract.try_into().unwrap());
        storage::put_reserve_a(&e, 0);
        storage::put_reserve_b(&e, 0);

        rewards::manager::set_reward_inv(&e, &Map::from_array(&e, [(0_u64, 0_u64)]));
        rewards::storage::set_pool_reward_config(
            &e,
            &rewards::storage::PoolRewardConfig {
                tps: 0,
                expired_at: 0,
            },
        );
        rewards::storage::set_pool_reward_data(
            &e,
            &rewards::storage::PoolRewardData {
                block: 0,
                accumulated: 0,
                last_time: 0,
            },
        );
    }

    fn share_id(e: Env) -> Address {
        storage::get_token_share(&e)
    }

    fn get_tokens(e: Env) -> Vec<Address> {
        Vec::from_array(&e, [storage::get_token_a(&e), storage::get_token_b(&e)])
    }

    fn deposit(
        e: Env,
        user: Address,
        desired_amounts: Vec<i128>,
        // min_amounts: Vec<i128>,
    ) -> (Vec<i128>, i128) {
        // Depositor needs to authorize the deposit
        user.require_auth();

        let (reserve_a, reserve_b) = (storage::get_reserve_a(&e), storage::get_reserve_b(&e));

        // Before actual changes were made to the pool, update total rewards data and refresh/initialize user reward
        let pool_data = rewards::manager::update_rewards_data(&e);
        rewards::manager::update_user_reward(&e, &pool_data, &user);
        rewards::storage::bump_user_reward_data(&e, &user);

        let desired_a = desired_amounts.get(0).unwrap();
        let desired_b = desired_amounts.get(1).unwrap();

        // todo: return back after interface unify
        // let min_a = min_amounts.get(0).unwrap();
        // let min_b = min_amounts.get(1).unwrap();
        let (min_a, min_b) = (0, 0);

        // Calculate deposit amounts
        let amounts = pool::get_deposit_amounts(
            desired_a as i128,
            min_a,
            desired_b as i128,
            min_b,
            reserve_a,
            reserve_b,
        );

        let token_a_client = token::Client::new(&e, &storage::get_token_a(&e));
        let token_b_client = token::Client::new(&e, &storage::get_token_b(&e));

        token_a_client.transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &amounts.0,
        );
        token_b_client.transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &amounts.1,
        );

        // Now calculate how many new pool shares to mint
        let (balance_a, balance_b) = (token::get_balance_a(&e), token::get_balance_b(&e));
        let total_shares = token::get_total_shares(&e);

        let zero = 0;
        let new_total_shares = if reserve_a > zero && reserve_b > zero {
            let shares_a = (balance_a * total_shares) / reserve_a;
            let shares_b = (balance_b * total_shares) / reserve_b;
            shares_a.min(shares_b)
        } else {
            (balance_a * balance_b).sqrt()
        };

        let shares_to_mint = new_total_shares - total_shares;
        token::mint_shares(&e, user, shares_to_mint);
        storage::put_reserve_a(&e, balance_a);
        storage::put_reserve_b(&e, balance_b);
        (Vec::from_array(&e, [amounts.0, amounts.1]), shares_to_mint)
    }

    fn swap(
        e: Env,
        user: Address,
        in_idx: u32,
        out_idx: u32,
        in_amount: i128,
        out_min: i128,
    ) -> i128 {
        user.require_auth();

        if in_idx == out_idx {
            panic!("cannot swap token to same one")
        }

        if in_idx > 1 {
            panic!("in_idx out of bounds");
        }

        if out_idx > 1 {
            panic!("in_idx out of bounds");
        }

        let reserve_a = storage::get_reserve_a(&e);
        let reserve_b = storage::get_reserve_b(&e);
        let reserves = Vec::from_array(&e, [reserve_a, reserve_b]);
        let tokens = Self::get_tokens(e.clone());
        let reserve_sell = reserves.get(in_idx).unwrap();
        let reserve_buy = reserves.get(out_idx).unwrap();

        let fee_fraction = storage::get_fee_fraction(&e);

        // First calculate how much we can get with in_amount from the pool
        let multiplier_with_fee = 10000 - fee_fraction as i128;
        let n = in_amount * reserve_buy * multiplier_with_fee;
        let d = reserve_sell * 10000 + in_amount * multiplier_with_fee;
        let out = n / d;
        if out < out_min {
            panic!("out amount is less than min")
        }

        // Transfer the amount being sold to the contract
        let sell_token = tokens.get(in_idx).unwrap();
        let sell_token_client = token::Client::new(&e, &sell_token);
        sell_token_client.transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &in_amount,
        );

        let (balance_a, balance_b) = (token::get_balance_a(&e), token::get_balance_b(&e));

        // residue_numerator and residue_denominator are the amount that the invariant considers after
        // deducting the fee, scaled up by 1000 to avoid fractions
        let residue_numerator = 10000 - fee_fraction as i128;
        let residue_denominator = 10000;
        let zero = 0;

        let new_invariant_factor = |balance: i128, reserve: i128, out: i128| {
            let delta = balance - reserve - out;
            let adj_delta = if delta > zero {
                residue_numerator * delta
            } else {
                residue_denominator * delta
            };
            residue_denominator * reserve + adj_delta
        };

        let (out_a, out_b) = if out_idx == 0 { (out, 0) } else { (0, out) };

        let new_inv_a = new_invariant_factor(balance_a, reserve_a, out_a);
        let new_inv_b = new_invariant_factor(balance_b, reserve_b, out_b);
        let old_inv_a = residue_denominator * reserve_a;
        let old_inv_b = residue_denominator * reserve_b;

        if new_inv_a * new_inv_b < old_inv_a * old_inv_b {
            panic!("constant product invariant does not hold");
        }

        if out_idx == 0 {
            token::transfer_a(&e, user, out_a);
        } else {
            token::transfer_b(&e, user, out_b);
        }

        storage::put_reserve_a(&e, balance_a - out_a);
        storage::put_reserve_b(&e, balance_b - out_b);
        out
    }

    fn estimate_swap(e: Env, in_idx: u32, out_idx: u32, in_amount: i128) -> i128 {
        if in_idx == out_idx {
            panic!("cannot swap token to same one")
        }

        if in_idx > 1 {
            panic!("in_idx out of bounds");
        }

        if out_idx > 1 {
            panic!("in_idx out of bounds");
        }

        let reserve_a = storage::get_reserve_a(&e);
        let reserve_b = storage::get_reserve_b(&e);
        let reserves = Vec::from_array(&e, [reserve_a, reserve_b]);
        let reserve_sell = reserves.get(in_idx).unwrap();
        let reserve_buy = reserves.get(out_idx).unwrap();

        let fee_fraction = storage::get_fee_fraction(&e);

        // First calculate how much needs to be sold to buy amount out from the pool
        let multiplier_with_fee = 10000 - fee_fraction as i128;
        let n = in_amount * reserve_buy * multiplier_with_fee;
        let d = reserve_sell * 10000 + in_amount * multiplier_with_fee;
        let out = n / d;
        out
    }

    fn withdraw(e: Env, user: Address, share_amount: i128, min_amounts: Vec<i128>) -> Vec<i128> {
        user.require_auth();

        // Before actual changes were made to the pool, update total rewards data and refresh user reward
        let pool_data = rewards::manager::update_rewards_data(&e);
        rewards::manager::update_user_reward(&e, &pool_data, &user);
        rewards::storage::bump_user_reward_data(&e, &user);

        // First transfer the pool shares that need to be redeemed
        let share_token_client = token::Client::new(&e, &storage::get_token_share(&e));
        share_token_client.transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &share_amount,
        );

        let (balance_a, balance_b) = (token::get_balance_a(&e), token::get_balance_b(&e));
        let balance_shares = token::get_balance_shares(&e);

        let total_shares = token::get_total_shares(&e);

        // Now calculate the withdraw amounts
        let out_a = (balance_a * balance_shares) / total_shares;
        let out_b = (balance_b * balance_shares) / total_shares;

        let min_a = min_amounts.get(0).unwrap();
        let min_b = min_amounts.get(1).unwrap();

        if out_a < min_a || out_b < min_b {
            panic!("min not satisfied");
        }

        token::burn_shares(&e, balance_shares);
        token::transfer_a(&e, user.clone(), out_a);
        token::transfer_b(&e, user, out_b);
        storage::put_reserve_a(&e, balance_a - out_a);
        storage::put_reserve_b(&e, balance_b - out_b);

        Vec::from_array(&e, [out_a, out_b])
    }

    fn get_reserves(e: Env) -> Vec<i128> {
        Vec::from_array(&e, [storage::get_reserve_a(&e), storage::get_reserve_b(&e)])
    }

    fn get_fee_fraction(e: Env) -> u32 {
        // returns fee fraction. 0.01% = 1; 1% = 100; 0.3% = 30
        storage::get_fee_fraction(&e)
    }
}

impl UpgradeableContractTrait for LiquidityPool {
    fn version() -> u32 {
        1
    }

    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) -> bool {
        require_admin(&e);
        e.deployer().update_current_contract_wasm(new_wasm_hash);
        true
    }
}

#[contractimpl]
impl RewardsTrait for LiquidityPool {
    fn initialize_rewards_config(e: Env, reward_token: Address, reward_storage: Address) {
        // admin.require_auth();
        // check_admin(&e, &admin);

        if storage::has_reward_token(&e) {
            panic!("rewards config already initialized")
        }

        storage::put_reward_token(&e, reward_token);
        storage::put_reward_storage(&e, reward_storage);
    }

    fn set_rewards_config(
        e: Env,
        admin: Address,
        expired_at: u64, // timestamp
        amount: i128,    // value with 7 decimal places. example: 600_0000000
    ) -> bool {
        admin.require_auth();
        check_admin(&e, &admin);

        rewards::manager::update_rewards_data(&e);

        let config = rewards::storage::PoolRewardConfig {
            tps: amount / to_i128(expired_at - e.ledger().timestamp()),
            expired_at,
        };
        storage::bump_instance(&e);
        rewards::storage::set_pool_reward_config(&e, &config);
        true
    }

    fn get_rewards_info(e: Env, user: Address) -> Map<Symbol, i128> {
        let config = get_pool_reward_config(&e);
        let pool_data = rewards::manager::update_rewards_data(&e);
        let user_data = rewards::manager::update_user_reward(&e, &pool_data, &user);
        let mut result = Map::new(&e);
        result.set(symbol_short!("tps"), to_i128(config.tps));
        result.set(symbol_short!("exp_at"), to_i128(config.expired_at));
        result.set(symbol_short!("acc"), to_i128(pool_data.accumulated));
        result.set(symbol_short!("last_time"), to_i128(pool_data.last_time));
        result.set(
            symbol_short!("pool_acc"),
            to_i128(user_data.pool_accumulated),
        );
        result.set(symbol_short!("block"), to_i128(pool_data.block));
        result.set(symbol_short!("usr_block"), to_i128(user_data.last_block));
        result.set(symbol_short!("to_claim"), to_i128(user_data.to_claim));
        result
    }

    fn get_user_reward(e: Env, user: Address) -> i128 {
        rewards::manager::get_amount_to_claim(&e, &user)
    }

    fn claim(e: Env, user: Address) -> i128 {
        let reward = rewards::manager::claim_reward(&e, &user);
        rewards::storage::bump_user_reward_data(&e, &user);
        reward
    }
}
