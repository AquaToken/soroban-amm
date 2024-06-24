use crate::pool_constants::{
    ADMIN_ACTIONS_DELAY, FEE_DENOMINATOR, KILL_DEADLINE_DT, MAX_A, MAX_ADMIN_FEE, MAX_A_CHANGE,
    MAX_FEE, MIN_RAMP_TIME, PRICE_PRECISION,
};
use crate::pool_interface::{
    AdminInterfaceTrait, InternalInterfaceTrait, LiquidityPoolInterfaceTrait, LiquidityPoolTrait,
    ManagedLiquidityPool, RewardsTrait, UpgradeableContractTrait,
};
use crate::storage::{
    get_admin_actions_deadline, get_admin_fee, get_fee, get_future_a, get_future_a_time,
    get_future_admin_fee, get_future_fee, get_initial_a, get_initial_a_time, get_is_killed,
    get_kill_deadline, get_plane, get_reserves, get_router, get_tokens,
    get_transfer_ownership_deadline, has_plane, put_admin_actions_deadline, put_admin_fee, put_fee,
    put_future_a, put_future_a_time, put_future_admin_fee, put_future_fee, put_initial_a,
    put_initial_a_time, put_is_killed, put_kill_deadline, put_reserves, put_tokens,
    put_transfer_ownership_deadline, set_plane, set_router,
};
use crate::token::create_contract;
use token_share::{
    burn_shares, get_token_share, get_total_shares, get_user_balance_shares, mint_shares,
    put_token_share, Client as LPToken,
};

use crate::errors::LiquidityPoolError;
use crate::plane::update_plane;
use crate::plane_interface::Plane;
use crate::rewards::get_rewards_manager;
use access_control::access::{AccessControl, AccessControlTrait};
use cast::i128 as to_i128;
use liquidity_pool_events::{Events as PoolEvents, LiquidityPoolEvents};
use liquidity_pool_validation_errors::LiquidityPoolValidationError;
use rewards::storage::RewardsStorageTrait;
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::token::Client as SorobanTokenClient;
use soroban_sdk::{
    contract, contractimpl, contractmeta, panic_with_error, symbol_short, Address, BytesN, Env,
    IntoVal, Map, Symbol, Val, Vec, U256,
};
use utils::math_errors::MathError;
use utils::storage_errors::StorageError;

contractmeta!(
    key = "Description",
    val = "Stable Swap AMM for set of tokens"
);

#[contract]
pub struct LiquidityPool;

#[contractimpl]
impl LiquidityPoolTrait for LiquidityPool {
    // Returns the amplification coefficient `A` in its raw form.
    // The bigger A is, the more the pool is concentrated around the initial price.
    //
    // # Returns
    //
    // * The amplification coefficient `A`.
    fn a(e: Env) -> u128 {
        // Handle ramping A up or down
        let t1 = get_future_a_time(&e) as u128;
        let a1 = get_future_a(&e);
        let now = e.ledger().timestamp() as u128;

        if now < t1 {
            let a0 = get_initial_a(&e);
            let t0 = get_initial_a_time(&e) as u128;
            // Expressions in u128 cannot have negative numbers, thus "if"
            if a1 > a0 {
                a0 + (a1 - a0).fixed_mul_floor(&e, now - t0, t1 - t0)
            } else {
                a0 - (a0 - a1).fixed_mul_floor(&e, now - t0, t1 - t0)
            }
        } else {
            // when t1 == 0 or block.timestamp >= t1
            a1
        }
    }

    // Returns the virtual price for 1 LP token in underlying tokens, scaled by 1e7.
    //
    // # Returns
    //
    // * The virtual price for 1 LP token.
    fn get_virtual_price(e: Env) -> u128 {
        let d = Self::get_d(e.clone(), Self::get_reserves(e.clone()), Self::a(e.clone()));
        // D is in the units similar to DAI (e.g. converted to precision 1e7)
        // When balanced, D = n * x_u - total virtual value of the portfolio
        let token_supply = get_total_shares(&e);
        d.fixed_mul_floor(&e, PRICE_PRECISION, token_supply)
    }

    // Calculate the amount of LP tokens to mint from a deposit.
    //
    // # Arguments
    //
    // * `amounts` - The amounts of tokens being deposited.
    // * `deposit` - Flag indicating if the tokens are being deposited (true), or withdrawn (false).
    //
    // # Returns
    //
    // * The amount of LP tokens to mint.
    fn calc_token_amount(e: Env, amounts: Vec<u128>, deposit: bool) -> u128 {
        let tokens = Self::get_tokens(e.clone());
        let n_coins = tokens.len();

        if amounts.len() != n_coins {
            panic_with_error!(e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        let mut balances = get_reserves(&e);
        let amp = Self::a(e.clone());
        let d0 = Self::get_d(e.clone(), balances.clone(), amp);
        for i in 0..n_coins {
            if deposit {
                balances.set(i, balances.get(i).unwrap() + amounts.get(i).unwrap());
            } else {
                balances.set(i, balances.get(i).unwrap() - amounts.get(i).unwrap());
            }
        }
        let d1 = Self::get_d(e.clone(), balances, amp);
        let token_amount = get_total_shares(&e);
        let diff = if deposit { d1 - d0 } else { d0 - d1 };
        diff.fixed_mul_floor(&e, token_amount, d0)
    }

    // Calculate the amount of token `j` that will be received for swapping `dx` of token `i`.
    //
    // # Arguments
    //
    // * `i` - The index of the token being swapped.
    // * `j` - The index of the token being received.
    // * `dx` - The amount of token `i` being swapped.
    //
    // # Returns
    //
    // * The amount of token `j` that will be received.
    fn get_dy(e: Env, i: u32, j: u32, dx: u128) -> u128 {
        // dx and dy in c-units
        let xp = Self::get_reserves(e.clone());

        let x = xp.get(i).unwrap() + dx;
        let y = Self::get_y(e.clone(), i, j, x, xp.clone());

        if y == 0 {
            // pool is empty
            return 0;
        }

        let dy = xp.get(j).unwrap() - y - 1;
        // The `fixed_mul_ceil` function is used to perform the multiplication
        //  to ensure user cannot exploit rounding errors.
        let fee = (get_fee(&e) as u128).fixed_mul_ceil(&e, dy, FEE_DENOMINATOR as u128);
        dy - fee
    }

    // Withdraw coins from the pool in an imbalanced amount.
    //
    // # Arguments
    //
    // * `user` - The address of the user withdrawing funds.
    // * `amounts` - The amounts of tokens to withdraw.
    // * `max_burn_amount` - The maximum amount of LP tokens to burn.
    //
    // # Returns
    //
    // * The actual amount of LP tokens burned.
    fn remove_liquidity_imbalance(
        e: Env,
        user: Address,
        amounts: Vec<u128>,
        max_burn_amount: u128,
    ) -> u128 {
        user.require_auth();

        let tokens = Self::get_tokens(e.clone());
        let n_coins = tokens.len();

        if amounts.len() != n_coins {
            panic_with_error!(e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        // Before actual changes were made to the pool, update total rewards data and refresh user reward
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let pool_data = rewards.manager().update_rewards_data(total_shares);
        let user_shares = get_user_balance_shares(&e, &user);
        rewards
            .manager()
            .update_user_reward(&pool_data, &user, user_shares);
        rewards.storage().bump_user_reward_data(&user);

        if get_is_killed(&e) {
            panic_with_error!(e, LiquidityPoolError::PoolKilled);
        }

        let token_supply = get_total_shares(&e);
        if token_supply == 0 {
            panic_with_error!(&e, LiquidityPoolValidationError::EmptyPool);
        }
        let admin_fee = get_admin_fee(&e) as u128;
        let amp = Self::a(e.clone());
        let mut reserves = get_reserves(&e);

        let old_balances = reserves.clone();
        let mut new_balances = old_balances.clone();

        let d0 = Self::get_d(e.clone(), old_balances.clone(), amp);
        for i in 0..n_coins {
            new_balances.set(i, new_balances.get(i).unwrap() - amounts.get(i).unwrap());
        }

        let d1 = Self::get_d(e.clone(), new_balances.clone(), amp);
        let mut fees = Vec::new(&e);

        for i in 0..n_coins {
            let new_balance = new_balances.get(i).unwrap();
            let ideal_balance = d1.fixed_mul_floor(&e, old_balances.get(i).unwrap(), d0);
            let difference = if ideal_balance > new_balance {
                ideal_balance - new_balance
            } else {
                new_balance - ideal_balance
            };
            // This formula ensures that the fee is proportionally distributed
            //  among the different coins in the pool. The denominator (4 * (N_COINS - 1)) is used
            //  to adjust the fee based on the number of coins. As the number of coins increases,
            //  the fee for each individual coin decreases.
            let fee = difference.fixed_mul_ceil(
                &e,
                get_fee(&e) as u128 * n_coins as u128,
                4 * (n_coins as u128 - 1) * FEE_DENOMINATOR as u128,
            );
            fees.push_back(fee);
            // Admin fee is deducted from pool available reserves
            reserves.set(
                i,
                new_balance - (fee.fixed_mul_ceil(&e, admin_fee, FEE_DENOMINATOR as u128)),
            );
            new_balances.set(i, new_balance - fee);
        }
        put_reserves(&e, &reserves);

        let d2 = Self::get_d(e.clone(), new_balances, amp);

        let mut token_amount = (d0 - d2).fixed_mul_floor(&e, token_supply, d0);
        if token_amount == 0 {
            panic_with_error!(&e, LiquidityPoolValidationError::ZeroSharesBurned);
        }
        token_amount += 1; // In case of rounding errors - make it unfavorable for the "attacker"
        if token_amount > max_burn_amount {
            panic_with_error!(&e, LiquidityPoolValidationError::TooManySharesBurned);
        }

        // First transfer the pool shares that need to be redeemed
        // Transfer max amount and return back change to avoid auth race condition
        let share_token_client = SorobanTokenClient::new(&e, &get_token_share(&e));
        share_token_client.transfer(
            &user,
            &e.current_contract_address(),
            &(max_burn_amount as i128),
        );
        if max_burn_amount > token_amount {
            share_token_client.transfer(
                &e.current_contract_address(),
                &user,
                &((max_burn_amount - token_amount) as i128),
            );
        }
        burn_shares(&e, token_amount as i128);

        for i in 0..n_coins {
            if amounts.get(i).unwrap() != 0 {
                let coins = get_tokens(&e);
                let token_client = SorobanTokenClient::new(&e, &coins.get(i).unwrap());
                token_client.transfer(
                    &e.current_contract_address(),
                    &user,
                    &(amounts.get(i).unwrap() as i128),
                );
            }
        }

        // update plane data for every pool update
        update_plane(&e);

        token_amount
    }

    // Calculate the amount received when withdrawing a single coin.
    //
    // # Arguments
    //
    // * `share_amount` - Amount of LP tokens to burn in the withdrawal
    // * `i` - The index of the token to withdraw.
    //
    // # Returns
    //
    // * The amounts of tokens withdrawn.
    fn calc_withdraw_one_coin(e: Env, share_amount: u128, i: u32) -> u128 {
        Self::internal_calc_withdraw_one_coin(e, share_amount, i).0
    }

    // Withdraws a single token from the pool.
    //
    // # Arguments
    //
    // * `user` - The address of the user withdrawing funds.
    // * `share_amount` - The amount of LP tokens to burn.
    // * `i` - The index of the token to withdraw.
    // * `min_amount` - The minimum amount of token to withdraw.
    //
    // # Returns
    //
    // * The amounts of tokens withdrawn.
    fn withdraw_one_coin(
        e: Env,
        user: Address,
        share_amount: u128,
        i: u32,
        min_amount: u128,
    ) -> Vec<u128> {
        user.require_auth();

        // Before actual changes were made to the pool, update total rewards data and refresh user reward
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let pool_data = rewards.manager().update_rewards_data(total_shares);
        let user_shares = get_user_balance_shares(&e, &user);
        rewards
            .manager()
            .update_user_reward(&pool_data, &user, user_shares);
        rewards.storage().bump_user_reward_data(&user);

        if get_is_killed(&e) {
            panic_with_error!(e, LiquidityPoolError::PoolKilled);
        }

        let (dy, dy_fee) = Self::internal_calc_withdraw_one_coin(e.clone(), share_amount, i);
        if dy < min_amount {
            panic_with_error!(&e, LiquidityPoolValidationError::InMinNotSatisfied);
        }

        let mut reserves = get_reserves(&e);
        reserves.set(
            i,
            reserves.get(i).unwrap()
                - (dy
                    + dy_fee.fixed_mul_floor(
                        &e,
                        get_admin_fee(&e) as u128,
                        FEE_DENOMINATOR as u128,
                    )),
        );
        put_reserves(&e, &reserves);

        // First transfer the pool shares that need to be redeemed
        let share_token_client = SorobanTokenClient::new(&e, &get_token_share(&e));
        share_token_client.transfer(
            &user,
            &e.current_contract_address(),
            &(share_amount as i128),
        );
        burn_shares(&e, share_amount as i128);

        let coins = get_tokens(&e);
        let token_client = SorobanTokenClient::new(&e, &coins.get(i).unwrap());
        token_client.transfer(&e.current_contract_address(), &user, &(dy as i128));

        // update plane data for every pool update
        update_plane(&e);

        let mut amounts: Vec<u128> = Vec::new(&e);
        for token_idx in 0..coins.len() {
            if token_idx == i {
                amounts.push_back(dy);
            } else {
                amounts.push_back(0);
            }
        }
        PoolEvents::new(&e).withdraw_liquidity(coins, amounts.clone(), share_amount);
        amounts
    }
}

impl InternalInterfaceTrait for LiquidityPool {
    // Calculates the invariant `D` for the given token balances.
    //
    // # Arguments
    //
    // * `xp` - The balances of each token in the pool.
    // * `amp` - The amplification coefficient.
    //
    // # Returns
    //
    // * The invariant `D`.
    fn get_d(e: Env, xp: Vec<u128>, amp: u128) -> u128 {
        let tokens = Self::get_tokens(e.clone());
        let n_coins = tokens.len();

        let mut s = 0;
        for x in xp.clone() {
            s += x;
        }
        if s == 0 {
            return 0;
        }

        let mut d_prev;
        let mut d = s;
        let ann = amp * n_coins as u128;
        for _i in 0..255 {
            let mut d_p = d.clone();
            for x1 in xp.clone() {
                d_p = d_p.fixed_mul_floor(&e, d, x1 * n_coins as u128);
            }
            d_prev = d.clone();
            d = (ann * s + d_p * n_coins as u128).fixed_mul_floor(
                &e,
                d,
                (ann - 1) * d + (n_coins as u128 + 1) * d_p,
            );

            // // Equality with the precision of 1
            if d > d_prev {
                if d - d_prev <= 1 {
                    break;
                }
            } else if d_prev - d <= 1 {
                break;
            }
        }
        d
    }

    // Calculates the amount of token `j` that will be received for swapping `dx` of token `i`.
    //
    // # Arguments
    //
    // * `i` - The index of the token being swapped.
    // * `j` - The index of the token being received.
    // * `x` - The amount of token `i` being swapped.
    // * `xp_` - The balances of each token in the pool.
    //
    // # Returns
    //
    // * The amount of token `j` that will be received.
    fn get_y(e: Env, in_idx: u32, out_idx: u32, x: u128, xp: Vec<u128>) -> u128 {
        // x in the input is converted to the same price/precision
        let tokens = Self::get_tokens(e.clone());
        let n_coins = tokens.len();

        if in_idx == out_idx {
            panic_with_error!(e, LiquidityPoolValidationError::CannotSwapSameToken);
        }
        if out_idx >= n_coins {
            panic_with_error!(e, LiquidityPoolValidationError::OutTokenOutOfBounds);
        }

        if in_idx >= n_coins {
            panic_with_error!(e, LiquidityPoolValidationError::InTokenOutOfBounds);
        }

        let amp = Self::a(e.clone());
        let d = Self::get_d(e.clone(), xp.clone(), amp);
        let mut c = d;
        let mut s = 0;
        let ann = amp * n_coins as u128;

        let mut x1;
        for i in 0..n_coins {
            if i == in_idx {
                x1 = x;
            } else if i != out_idx {
                x1 = xp.get(i).unwrap();
            } else {
                continue;
            }
            s += x1;
            c = c.fixed_mul_floor(&e, d, x1 * n_coins as u128);
        }
        let c_256 = U256::from_u128(&e, c)
            .mul(&U256::from_u128(&e, d))
            .div(&U256::from_u128(&e, ann * n_coins as u128));
        let b = s + d / ann; // - D
        let mut y_prev;
        let mut y = d;
        for _i in 0..255 {
            y_prev = y;
            let y_256 = U256::from_u128(&e, y);
            y = match y_256
                .mul(&y_256)
                .add(&c_256)
                .div(&U256::from_u128(&e, 2 * y + b - d))
                .to_u128()
            {
                Some(v) => v,
                None => panic_with_error!(&e, MathError::NumberOverflow),
            };

            // Equality with the precision of 1
            if y > y_prev {
                if y - y_prev <= 1 {
                    break;
                }
            } else if y_prev - y <= 1 {
                break;
            }
        }
        y
    }

    // Calculates the amount of token `j` that will be received for swapping `dx` of token `i`.
    //
    // # Arguments
    //
    // * `a` - The amplification coefficient.
    // * `i` - The index of the token being swapped.
    // * `xp` - The balances of each token in the pool.
    // * `d` - The invariant `D`.
    //
    // # Returns
    //
    // * The amount of token `j` that will be received.
    fn get_y_d(e: Env, a: u128, in_idx: u32, xp: Vec<u128>, d: u128) -> u128 {
        // Calculate x[i] if one reduces D from being calculated for xp to D
        //
        // Done by solving quadratic equation iteratively.
        // x_1**2 + x1 * (sum' - (A*n**n - 1) * D / (A * n**n)) = D ** (n + 1) / (n ** (2 * n) * prod' * A)
        // x_1**2 + b*x_1 = c
        //
        // x_1 = (x_1**2 + c) / (2*x_1 + b)

        // x in the input is converted to the same price/precision

        let tokens = Self::get_tokens(e.clone());
        let n_coins = tokens.len();

        if in_idx >= n_coins {
            panic_with_error!(&e, LiquidityPoolValidationError::InTokenOutOfBounds);
        }

        let mut c = d;
        let mut s = 0;
        let ann = a * n_coins as u128;

        let mut x;
        for i in 0..n_coins {
            if i != in_idx {
                x = xp.get(i).unwrap();
            } else {
                continue;
            }
            s += x;
            c = c.fixed_mul_floor(&e, d, x * n_coins as u128);
        }
        let c_256 = U256::from_u128(&e, c)
            .mul(&U256::from_u128(&e, d))
            .div(&U256::from_u128(&e, ann * n_coins as u128));

        let b = s + d / ann;
        let mut y_prev;
        let mut y = d;

        for _i in 0..255 {
            y_prev = y;
            let y_256 = U256::from_u128(&e, y);
            y = match y_256
                .mul(&y_256)
                .add(&c_256)
                .div(&U256::from_u128(&e, 2 * y + b - d))
                .to_u128()
            {
                Some(v) => v,
                None => panic_with_error!(&e, MathError::NumberOverflow),
            };

            // Equality with the precision of 1
            if y > y_prev {
                if y - y_prev <= 1 {
                    break;
                }
            } else if y_prev - y <= 1 {
                break;
            }
        }
        y
    }

    // Calculate the amount received when withdrawing a single coin.
    //
    // # Arguments
    //
    // * `share_amount` - The amount of LP tokens to burn.
    // * `i` - The index of the token to withdraw.
    //
    // # Returns
    //
    // * (The amount of token that can be withdrawn, Fee amount)
    fn internal_calc_withdraw_one_coin(e: Env, token_amount: u128, token_idx: u32) -> (u128, u128) {
        // First, need to calculate
        // * Get current D
        // * Solve Eqn against y_i for D - token_amount

        let tokens = Self::get_tokens(e.clone());
        let n_coins = tokens.len();

        let amp = Self::a(e.clone());
        let total_supply = get_total_shares(&e);

        let xp = Self::get_reserves(e.clone());

        let d0 = Self::get_d(e.clone(), xp.clone(), amp);
        let d1 = d0 - token_amount * d0 / total_supply;
        let mut xp_reduced = xp.clone();

        let new_y = Self::get_y_d(e.clone(), amp, token_idx, xp.clone(), d1);
        let dy_0 = xp.get(token_idx).unwrap() - new_y; // w/o fees;

        for j in 0..n_coins {
            let dx_expected = if j == token_idx {
                xp.get(j).unwrap() * d1 / d0 - new_y
            } else {
                xp.get(j).unwrap() - xp.get(j).unwrap() * d1 / d0
            };
            let fee = dx_expected.fixed_mul_ceil(
                &e,
                (get_fee(&e) * n_coins as u32) as u128,
                (FEE_DENOMINATOR * 4 * (n_coins as u32 - 1)) as u128,
            );
            xp_reduced.set(j, xp_reduced.get(j).unwrap() - fee);
        }

        let mut dy = xp_reduced.get(token_idx).unwrap()
            - Self::get_y_d(e.clone(), amp, token_idx, xp_reduced.clone(), d1);
        dy = dy - 1; // Withdraw less to account for rounding errors

        (dy, dy_0 - dy)
    }
}

#[contractimpl]
impl AdminInterfaceTrait for LiquidityPool {
    // Starts ramping A to target value in future timestamp.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `future_a` - The target value for A.
    // * `future_time` - The future timestamp when the target value should be reached.
    fn ramp_a(e: Env, admin: Address, future_a: u128, future_time: u64) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);
        if e.ledger().timestamp() < get_initial_a_time(&e) + MIN_RAMP_TIME {
            panic_with_error!(&e, LiquidityPoolError::RampTooEarly);
        };
        if future_time < e.ledger().timestamp() + MIN_RAMP_TIME {
            panic_with_error!(&e, LiquidityPoolError::RampTimeLessThanMinimum);
        };

        let initial_a = Self::a(e.clone());
        if !((future_a > 0) && (future_a < MAX_A)) {
            panic_with_error!(&e, LiquidityPoolError::RampOverMax);
        }
        if !(((future_a >= initial_a) && (future_a <= initial_a * MAX_A_CHANGE))
            || ((future_a < initial_a) && (future_a * MAX_A_CHANGE >= initial_a)))
        {
            panic_with_error!(&e, LiquidityPoolError::RampTooFast);
        }
        put_initial_a(&e, &initial_a);
        put_future_a(&e, &future_a);
        put_initial_a_time(&e, &e.ledger().timestamp());
        put_future_a_time(&e, &future_time);

        // update plane data for every pool update
        update_plane(&e);
    }

    // Stops ramping A.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn stop_ramp_a(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        let current_a = Self::a(e.clone());
        put_initial_a(&e, &current_a);
        put_future_a(&e, &current_a);
        put_initial_a_time(&e, &e.ledger().timestamp());
        put_future_a_time(&e, &e.ledger().timestamp());

        // now (block.timestamp < t1) is always False, so we return saved A

        // update plane data for every pool update
        update_plane(&e);
    }

    // Sets a new fee to be applied in the future.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `new_fee` - The new fee to be applied.
    // * `new_admin_fee` - The new admin fee to be applied.
    fn commit_new_fee(e: Env, admin: Address, new_fee: u32, new_admin_fee: u32) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        if get_admin_actions_deadline(&e) != 0 {
            panic_with_error!(&e, LiquidityPoolError::AnotherActionActive);
        }
        if new_fee > MAX_FEE {
            panic_with_error!(e, LiquidityPoolValidationError::FeeOutOfBounds);
        }
        if new_admin_fee > MAX_ADMIN_FEE {
            panic_with_error!(e, LiquidityPoolValidationError::AdminFeeOutOfBounds);
        }

        let deadline = e.ledger().timestamp() + ADMIN_ACTIONS_DELAY;
        put_admin_actions_deadline(&e, &deadline);
        put_future_fee(&e, &new_fee);
        put_future_admin_fee(&e, &new_admin_fee);
    }

    // Applies the committed fee.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn apply_new_fee(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        if e.ledger().timestamp() < get_admin_actions_deadline(&e) {
            panic_with_error!(&e, LiquidityPoolError::ActionNotReadyYet);
        }
        if get_admin_actions_deadline(&e) == 0 {
            panic_with_error!(&e, LiquidityPoolError::NoActionActive);
        }

        put_admin_actions_deadline(&e, &0);
        let fee = get_future_fee(&e);
        let admin_fee = get_future_admin_fee(&e);
        put_fee(&e, &fee);
        put_admin_fee(&e, &admin_fee);

        // update plane data for every pool update
        update_plane(&e);
    }

    // Reverts the committed parameters to their current values.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn revert_new_parameters(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        put_admin_actions_deadline(&e, &0);
    }

    // Commits an ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `new_admin` - The address of the new admin.
    fn commit_transfer_ownership(e: Env, admin: Address, new_admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        if get_transfer_ownership_deadline(&e) != 0 {
            panic_with_error!(&e, LiquidityPoolError::AnotherActionActive);
        }

        let deadline = e.ledger().timestamp() + ADMIN_ACTIONS_DELAY;
        put_transfer_ownership_deadline(&e, &deadline);
        access_control.set_future_admin(&new_admin);
    }

    // Applies the committed ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn apply_transfer_ownership(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        if e.ledger().timestamp() < get_transfer_ownership_deadline(&e) {
            panic_with_error!(&e, LiquidityPoolError::ActionNotReadyYet);
        }
        if get_transfer_ownership_deadline(&e) == 0 {
            panic_with_error!(&e, LiquidityPoolError::NoActionActive);
        }

        put_transfer_ownership_deadline(&e, &0);
        let future_admin = match access_control.get_future_admin() {
            Some(v) => v,
            None => panic_with_error!(&e, StorageError::ValueNotInitialized),
        };
        access_control.set_admin(&future_admin);
    }

    // Reverts the committed ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn revert_transfer_ownership(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        put_transfer_ownership_deadline(&e, &0);
    }

    // Gets the amount of collected admin fees.
    //
    // # Arguments
    //
    // * `i` - The index of the token.
    //
    // # Returns
    //
    // * The amount of collected admin fees for the token.
    fn admin_balances(e: Env, i: u32) -> u128 {
        let coins = get_tokens(&e);
        let token_client = SorobanTokenClient::new(&e, &coins.get(i).unwrap());
        let balance = token_client.balance(&e.current_contract_address()) as u128;
        let reserves = get_reserves(&e);

        balance - reserves.get(i).unwrap()
    }

    // Withdraws the collected admin fees.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn withdraw_admin_fees(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        let coins = get_tokens(&e);
        let reserves = get_reserves(&e);

        for i in 0..coins.len() {
            let token_client = SorobanTokenClient::new(&e, &coins.get(i).unwrap());
            let balance = token_client.balance(&e.current_contract_address()) as u128;

            let value = balance - reserves.get(i).unwrap();
            if value > 0 {
                token_client.transfer(&e.current_contract_address(), &admin, &(value as i128));
            }
        }
    }

    // Donates the collected admin fees to the common fee pool.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn donate_admin_fees(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        let coins = get_tokens(&e);
        let mut reserves = get_reserves(&e);

        for i in 0..coins.len() {
            let token_client = SorobanTokenClient::new(&e, &coins.get(i).unwrap());
            let balance = token_client.balance(&e.current_contract_address());
            reserves.set(i, balance as u128);
        }
        put_reserves(&e, &reserves);

        // update plane data for every pool update
        update_plane(&e);
    }

    // Stops the pool instantly.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn kill_me(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        if get_kill_deadline(&e) <= e.ledger().timestamp() {
            panic_with_error!(&e, LiquidityPoolError::ActionNotReadyYet);
        }
        put_is_killed(&e, &true);
    }

    // Resumes the pool.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn unkill_me(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        put_is_killed(&e, &false);
    }
}

#[contractimpl]
impl ManagedLiquidityPool for LiquidityPool {
    // Initializes all the necessary parameters for the liquidity pool.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `router` - The address of the router.
    // * `token_wasm_hash` - The hash of the token's WASM code.
    // * `coins` - The addresses of the coins.
    // * `a` - The amplification coefficient.
    // * `fee` - The fee to be applied.
    // * `admin_fee` - The admin fee to be applied.
    // * `reward_token` - The address of the reward token.
    // * `plane` - The address of the plane.
    fn initialize_all(
        e: Env,
        admin: Address,
        router: Address,
        token_wasm_hash: BytesN<32>,
        coins: Vec<Address>,
        a: u128,
        fee: u32,
        admin_fee: u32,
        reward_token: Address,
        plane: Address,
    ) {
        // merge whole initialize process into one because lack of caching of VM components
        // https://github.com/stellar/rs-soroban-env/issues/827
        Self::set_pools_plane(e.clone(), plane);
        Self::initialize(
            e.clone(),
            admin,
            router,
            token_wasm_hash,
            coins,
            a,
            fee,
            admin_fee,
        );
        Self::initialize_rewards_config(e.clone(), reward_token);
    }
}

#[contractimpl]
impl LiquidityPoolInterfaceTrait for LiquidityPool {
    // Returns the type of the liquidity pool.
    //
    // # Returns
    //
    // * The type of the liquidity pool as a `Symbol`.
    fn pool_type(e: Env) -> Symbol {
        Symbol::new(&e, "stable")
    }

    // Initializes the liquidity pool with the given parameters.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `router` - The address of the router.
    // * `token_wasm_hash` - The hash of the token's WASM code.
    // * `coins` - The addresses of the coins.
    // * `a` - The amplification coefficient.
    // * `fee` - The fee to be applied.
    // * `admin_fee` - The admin fee to be applied.
    fn initialize(
        e: Env,
        admin: Address,
        router: Address,
        token_wasm_hash: BytesN<32>,
        coins: Vec<Address>,
        a: u128,
        fee: u32,
        admin_fee: u32,
    ) {
        let access_control = AccessControl::new(&e);
        if access_control.has_admin() {
            panic_with_error!(&e, LiquidityPoolError::AlreadyInitialized);
        }

        access_control.set_admin(&admin);
        set_router(&e, &router);

        // 0.01% = 1; 1% = 100; 0.3% = 30
        if fee > MAX_FEE || admin_fee > MAX_ADMIN_FEE {
            panic_with_error!(&e, LiquidityPoolValidationError::FeeOutOfBounds);
        }

        put_fee(&e, &fee);
        put_admin_fee(&e, &admin_fee);

        put_tokens(&e, &coins);

        // LP token
        let share_contract = create_contract(&e, token_wasm_hash);
        LPToken::new(&e, &share_contract).initialize(
            &e.current_contract_address(),
            &7u32,
            &"Pool Share Token".into_val(&e),
            &"POOL".into_val(&e),
        );
        put_token_share(&e, share_contract);
        let mut initial_reserves = Vec::new(&e);
        for _i in 0..coins.len() {
            initial_reserves.push_back(0_u128);
        }
        put_reserves(&e, &initial_reserves);

        // pool config
        put_initial_a(&e, &a);
        put_initial_a_time(&e, &e.ledger().timestamp());
        put_future_a(&e, &a);
        put_future_a_time(&e, &e.ledger().timestamp());
        put_kill_deadline(&e, &(e.ledger().timestamp() + KILL_DEADLINE_DT));
        put_admin_actions_deadline(&e, &0);
        put_transfer_ownership_deadline(&e, &0);
        put_is_killed(&e, &false);

        // update plane data for every pool update
        update_plane(&e);
    }

    // Returns the pool's fee fraction.
    //
    // # Returns
    //
    // The pool's fee fraction as a u32.
    fn get_fee_fraction(e: Env) -> u32 {
        get_fee(&e)
    }

    // Returns the pool's admin fee percentage fraction.
    //
    // # Returns
    //
    // The pool's fee admin percentage fraction as a u32.
    fn get_admin_fee(e: Env) -> u32 {
        get_admin_fee(&e)
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

    // Returns the pool's reserves.
    //
    // # Returns
    //
    // A vector of the pool's reserves.
    fn get_reserves(e: Env) -> Vec<u128> {
        get_reserves(&e)
    }

    // Returns the pool's tokens.
    //
    // # Returns
    //
    // A vector of token addresses.
    fn get_tokens(e: Env) -> Vec<Address> {
        get_tokens(&e)
    }

    // Deposits tokens into the pool.
    //
    // # Arguments
    //
    // * `user` - The address of the user depositing the tokens.
    // * `amounts` - A vector of desired amounts of each token to deposit.
    // * `min_shares` - The minimum amount of pool tokens to mint.
    //
    // # Returns
    //
    // A tuple containing a vector of actual amounts of each token deposited and a u128 representing the amount of pool tokens minted.
    fn deposit(e: Env, user: Address, amounts: Vec<u128>, min_shares: u128) -> (Vec<u128>, u128) {
        user.require_auth();
        if get_is_killed(&e) {
            panic_with_error!(e, LiquidityPoolError::PoolKilled);
        }

        let tokens = Self::get_tokens(e.clone());
        let n_coins = tokens.len();

        if amounts.len() != n_coins as u32 {
            panic_with_error!(e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        // Before actual changes were made to the pool, update total rewards data and refresh/initialize user reward
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let pool_data = rewards.manager().update_rewards_data(total_shares);
        let user_shares = get_user_balance_shares(&e, &user);
        rewards
            .manager()
            .update_user_reward(&pool_data, &user, user_shares);
        rewards.storage().bump_user_reward_data(&user);

        let mut fees: Vec<u128> = Vec::new(&e);
        let admin_fee = get_admin_fee(&e) as u128;
        let amp = Self::a(e.clone());

        let token_supply = get_total_shares(&e);
        // Initial invariant
        let mut d0 = 0;
        let old_balances = get_reserves(&e);
        if token_supply > 0 {
            d0 = Self::get_d(e.clone(), old_balances.clone(), amp);
        }
        let mut new_balances: Vec<u128> = old_balances.clone();
        let coins = get_tokens(&e);

        for i in 0..n_coins {
            let in_amount = amounts.get(i).unwrap();
            if token_supply == 0 && in_amount == 0 {
                panic_with_error!(e, LiquidityPoolValidationError::AllCoinsRequired);
            }
            let in_coin = coins.get(i).unwrap();

            // Take coins from the sender
            if in_amount > 0 {
                let token_client = SorobanTokenClient::new(&e, &in_coin);
                token_client.transfer(&user, &e.current_contract_address(), &(in_amount as i128));
            }

            new_balances.set(i, old_balances.get(i).unwrap() + in_amount);
        }

        // Invariant after change
        let d1 = Self::get_d(e.clone(), new_balances.clone(), amp);
        if d1 <= d0 {
            panic_with_error!(&e, LiquidityPoolError::InvariantDoesNotHold);
        }

        // We need to recalculate the invariant accounting for fees
        // to calculate fair user's share
        let mut d2 = d1;
        let balances = if token_supply > 0 {
            let mut result = new_balances.clone();
            // Only account for fees if we are not the first to deposit
            for i in 0..n_coins {
                let new_balance = new_balances.get(i).unwrap();
                let ideal_balance = d1 * old_balances.get(i).unwrap() / d0;
                let difference = if ideal_balance > new_balance {
                    ideal_balance - new_balance
                } else {
                    new_balance - ideal_balance
                };

                // This formula ensures that the fee is proportionally distributed
                //  among the different coins in the pool. The denominator (4 * (N_COINS - 1)) is used
                //  to adjust the fee based on the number of coins. As the number of coins increases,
                //  the fee for each individual coin decreases.
                let fee = difference.fixed_mul_ceil(
                    &e,
                    get_fee(&e) as u128 * n_coins as u128,
                    FEE_DENOMINATOR as u128 * 4 * (n_coins as u128 - 1),
                );
                fees.push_back(fee);

                // Admin fee is deducted from pool available reserves
                result.set(
                    i,
                    new_balance - (fee.fixed_mul_ceil(&e, admin_fee, FEE_DENOMINATOR as u128)),
                );
                new_balances.set(i, new_balances.get(i).unwrap() - fee);
            }
            d2 = Self::get_d(e.clone(), new_balances, amp);
            result
        } else {
            new_balances
        };
        put_reserves(&e, &balances);

        // Calculate, how much pool tokens to mint
        let mint_amount = if token_supply == 0 {
            d1 // Take the dust if there was any
        } else {
            token_supply * (d2 - d0) / d0
        };

        if mint_amount < min_shares {
            panic_with_error!(&e, LiquidityPoolValidationError::OutMinNotSatisfied);
        }
        // Mint pool tokens
        mint_shares(&e, user, mint_amount as i128);

        // update plane data for every pool update
        update_plane(&e);

        PoolEvents::new(&e).deposit_liquidity(tokens, amounts.clone(), mint_amount);

        (amounts, mint_amount)
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
        if get_is_killed(&e) {
            panic_with_error!(e, LiquidityPoolError::PoolKilled);
        }

        let old_balances = get_reserves(&e);
        let xp = old_balances.clone();

        // Handling an unexpected charge of a fee on transfer (USDT, PAXG)
        let dx_w_fee = in_amount;
        let coins = get_tokens(&e);
        let input_coin = coins.get(in_idx).unwrap();

        let token_client = SorobanTokenClient::new(&e, &input_coin);
        token_client.transfer(&user, &e.current_contract_address(), &(in_amount as i128));

        let reserve_sell = xp.get(in_idx).unwrap();
        let reserve_buy = xp.get(out_idx).unwrap();
        if reserve_sell == 0 || reserve_buy == 0 {
            panic_with_error!(e, LiquidityPoolValidationError::EmptyPool);
        }

        let x = reserve_sell + dx_w_fee;
        let y = Self::get_y(e.clone(), in_idx, out_idx, x, xp.clone());

        let dy = reserve_buy - y - 1; // -1 just in case there were some rounding errors
        let dy_fee = dy.fixed_mul_ceil(&e, get_fee(&e) as u128, FEE_DENOMINATOR as u128);

        // Convert all to real units
        let dy = dy - dy_fee;
        if dy < out_min {
            panic_with_error!(e, LiquidityPoolValidationError::OutMinNotSatisfied);
        }

        let mut dy_admin_fee =
            dy_fee.fixed_mul_ceil(&e, get_admin_fee(&e) as u128, FEE_DENOMINATOR as u128);
        dy_admin_fee = dy_admin_fee;

        // Change balances exactly in same way as we change actual ERC20 coin amounts
        let mut reserves = get_reserves(&e);
        reserves.set(in_idx, old_balances.get(in_idx).unwrap() + dx_w_fee);
        // When rounding errors happen, we undercharge admin fee in favor of LP
        reserves.set(
            out_idx,
            old_balances.get(out_idx).unwrap() - dy - dy_admin_fee,
        );
        put_reserves(&e, &reserves);

        let token_out = coins.get(out_idx).unwrap();
        let token_client = SorobanTokenClient::new(&e, &token_out);
        token_client.transfer(&e.current_contract_address(), &user, &(dy as i128));

        // update plane data for every pool update
        update_plane(&e);

        PoolEvents::new(&e).trade(user, input_coin, token_out, in_amount, dy, dy_fee);

        dy
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
        Self::get_dy(e, in_idx, out_idx, in_amount)
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

        let tokens = Self::get_tokens(e.clone());
        let n_coins = tokens.len();

        if min_amounts.len() != n_coins {
            panic_with_error!(e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        // Before actual changes were made to the pool, update total rewards data and refresh user reward
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let pool_data = rewards.manager().update_rewards_data(total_shares);
        let user_shares = get_user_balance_shares(&e, &user);
        rewards
            .manager()
            .update_user_reward(&pool_data, &user, user_shares);
        rewards.storage().bump_user_reward_data(&user);

        let total_supply = get_total_shares(&e);
        let mut amounts: Vec<u128> = Vec::new(&e);
        let mut reserves = get_reserves(&e);
        let coins = get_tokens(&e);

        for i in 0..n_coins {
            let value = reserves
                .get(i)
                .unwrap()
                .fixed_mul_floor(&e, share_amount, total_supply);
            if value < min_amounts.get(i).unwrap() {
                panic_with_error!(&e, LiquidityPoolValidationError::OutMinNotSatisfied);
            }
            reserves.set(i, reserves.get(i).unwrap() - value);
            amounts.push_back(value);

            let token_client = SorobanTokenClient::new(&e, &coins.get(i).unwrap());
            token_client.transfer(&e.current_contract_address(), &user, &(value as i128));
        }
        put_reserves(&e, &reserves);

        // First transfer the pool shares that need to be redeemed
        let share_token_client = SorobanTokenClient::new(&e, &get_token_share(&e));
        share_token_client.transfer(
            &user,
            &e.current_contract_address(),
            &(share_amount as i128),
        );
        burn_shares(&e, share_amount as i128);

        // update plane data for every pool update
        update_plane(&e);

        PoolEvents::new(&e).withdraw_liquidity(tokens, amounts.clone(), share_amount);

        amounts
    }

    // Returns information about the pool.
    //
    // # Returns
    //
    // A map of Symbols to Vals representing the pool's information.
    fn get_info(e: Env) -> Map<Symbol, Val> {
        let fee = get_fee(&e);
        let a = Self::a(e.clone());
        let pool_type = Self::pool_type(e.clone());
        let tokens = Self::get_tokens(e.clone());
        let n_coins = tokens.len();
        let mut result = Map::new(&e);
        result.set(symbol_short!("pool_type"), pool_type.into_val(&e));
        result.set(symbol_short!("fee"), fee.into_val(&e));
        result.set(symbol_short!("a"), a.into_val(&e));
        result.set(symbol_short!("n_tokens"), (n_coins as u32).into_val(&e));
        result
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
        104
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
            panic_with_error!(e, LiquidityPoolError::RewardsAlreadyInitialized);
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
        let pool_data = rewards.manager().update_rewards_data(total_shares);
        let user_shares = get_user_balance_shares(&e, &user);
        let user_data = rewards
            .manager()
            .update_user_reward(&pool_data, &user, user_shares);
        let mut result = Map::new(&e);
        result.set(symbol_short!("tps"), to_i128(config.tps).unwrap());
        result.set(symbol_short!("exp_at"), to_i128(config.expired_at));
        result.set(
            symbol_short!("acc"),
            to_i128(pool_data.accumulated).unwrap(),
        );
        result.set(symbol_short!("last_time"), to_i128(pool_data.last_time));
        result.set(
            symbol_short!("pool_acc"),
            to_i128(user_data.pool_accumulated).unwrap(),
        );
        result.set(symbol_short!("block"), to_i128(pool_data.block));
        result.set(symbol_short!("usr_block"), to_i128(user_data.last_block));
        result.set(
            symbol_short!("to_claim"),
            to_i128(user_data.to_claim).unwrap(),
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
    fn set_pools_plane(e: Env, plane: Address) {
        if has_plane(&e) {
            panic_with_error!(&e, LiquidityPoolError::PlaneAlreadyInitialized);
        }

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
}
