use crate::pool_constants::{FEE_DENOMINATOR, MAX_A, MAX_A_CHANGE, MIN_RAMP_TIME};
use crate::pool_interface::{
    AdminInterfaceTrait, LiquidityPoolInterfaceTrait, LiquidityPoolTrait, ManagedLiquidityPool,
    RewardsTrait, UpgradeableContract, UpgradeableLPTokenTrait,
};
use crate::storage::{
    get_admin_actions_deadline, get_decimals, get_fee, get_future_a, get_future_a_time,
    get_future_fee, get_initial_a, get_initial_a_time, get_is_killed_claim, get_is_killed_deposit,
    get_is_killed_swap, get_plane, get_precision, get_precision_mul, get_reserves, get_router,
    get_token_future_wasm, get_tokens, has_plane, put_admin_actions_deadline, put_decimals,
    put_fee, put_future_a, put_future_a_time, put_future_fee, put_initial_a, put_initial_a_time,
    put_reserves, put_tokens, set_is_killed_claim, set_is_killed_deposit, set_is_killed_swap,
    set_plane, set_router, set_token_future_wasm,
};
use crate::token::create_contract;
use token_share::{
    burn_shares, get_token_share, get_total_shares, get_user_balance_shares, mint_shares,
    put_token_share, Client as LPToken,
};

use crate::errors::LiquidityPoolError;
use crate::events::Events;
use crate::normalize::{read_decimals, xp};
use crate::plane::update_plane;
use crate::plane_interface::Plane;
use crate::rewards::get_rewards_manager;
use access_control::access::{AccessControl, AccessControlTrait};
use access_control::constants::ADMIN_ACTIONS_DELAY;
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
};
use liquidity_pool_events::{Events as PoolEvents, LiquidityPoolEvents};
use liquidity_pool_validation_errors::LiquidityPoolValidationError;
use rewards::events::Events as RewardEvents;
use rewards::storage::{
    BoostFeedStorageTrait, BoostTokenStorageTrait, PoolRewardsStorageTrait, RewardTokenStorageTrait,
};
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::token::Client as SorobanTokenClient;
use soroban_sdk::{
    contract, contractimpl, contractmeta, panic_with_error, symbol_short, Address, BytesN, Env,
    IntoVal, Map, Symbol, Val, Vec, U256,
};
use upgrade::events::Events as UpgradeEvents;
use upgrade::{apply_upgrade, commit_upgrade, revert_upgrade};

contractmeta!(
    key = "Description",
    val = "Stable Swap AMM for set of tokens"
);

#[contract]
pub struct LiquidityPool;

#[contractimpl]
impl LiquidityPoolTrait for LiquidityPool {
    // Returns the actual value of amplification coefficient `Amp = A*N**(N-1)`.
    // The bigger Amp is, the more the pool is concentrated around the initial price.
    //
    // # Returns
    //
    // * The amplification coefficient `Amp`.
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
                a0 + (a1 - a0).fixed_mul_floor(&e, &(now - t0), &(t1 - t0))
            } else {
                a0 - (a0 - a1).fixed_mul_floor(&e, &(now - t0), &(t1 - t0))
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
        let d = Self::_get_d(&e, &Self::_xp(&e, &get_reserves(&e)), Self::a(e.clone()));
        // D is in the units similar to DAI (e.g. converted to precision 1e7)
        // When balanced, D = n * x_u - total virtual value of the portfolio
        let token_supply = get_total_shares(&e);
        d.fixed_mul_floor(
            &e,
            &U256::from_u128(&e, get_precision(&e)),
            &U256::from_u128(&e, token_supply),
        )
        .to_u128()
        .unwrap()
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
        let tokens = get_tokens(&e);
        let n_coins = tokens.len();

        if amounts.len() != n_coins {
            panic_with_error!(e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        let mut reserves = get_reserves(&e);
        let amp = Self::a(e.clone());
        let d0 = Self::_get_d(&e, &Self::_xp(&e, &reserves), amp);
        for i in 0..n_coins {
            if deposit {
                reserves.set(i, reserves.get(i).unwrap() + amounts.get(i).unwrap());
            } else {
                reserves.set(i, reserves.get(i).unwrap() - amounts.get(i).unwrap());
            }
        }
        let d1 = Self::_get_d(&e, &Self::_xp(&e, &reserves), amp);
        let token_amount = get_total_shares(&e);
        let diff = if deposit { d1.sub(&d0) } else { d0.sub(&d1) };
        diff.fixed_mul_floor(&e, &U256::from_u128(&e, token_amount), &d0)
            .to_u128()
            .unwrap()
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
        let precision_mul = get_precision_mul(&e);
        let xp = Self::_xp(&e, &get_reserves(&e));

        let x = xp.get(i).unwrap() + dx * precision_mul.get(i).unwrap();
        let y = Self::_get_y(&e, i, j, x, &xp);

        if y == 0 {
            // pool is empty
            return 0;
        }

        let dy = (xp.get(j).unwrap() - y - 1) / precision_mul.get(j).unwrap();
        // The `fixed_mul_ceil` function is used to perform the multiplication
        //  to ensure user cannot exploit rounding errors.
        let fee = (get_fee(&e) as u128).fixed_mul_ceil(&e, &dy, &(FEE_DENOMINATOR as u128));
        dy - fee
    }

    // Calculate the amount of token `i` that will be sent for swapping `dy` of token `j`.
    //
    // # Arguments
    //
    // * `i` - The index of the token being swapped.
    // * `j` - The index of the token being received.
    // * `dy` - The amount of token `j` being swapped.
    //
    // # Returns
    //
    // * The amount of token `i` that will be swapped.
    fn get_dx(e: Env, i: u32, j: u32, dy: u128) -> u128 {
        // dx and dy in c-units
        let precision_mul = get_precision_mul(&e);
        let xp = Self::_xp(&e, &get_reserves(&e));
        let xp_buy = xp.get(j).unwrap();

        // apply fee to dy to keep swap symmetrical
        let dy_w_fee_scaled = dy.fixed_mul_ceil(
            &e,
            &(FEE_DENOMINATOR as u128),
            &((FEE_DENOMINATOR - get_fee(&e)) as u128),
        ) * precision_mul.get(j).unwrap();

        // if total value including fee is more than the reserve, math can't be done properly
        if dy_w_fee_scaled >= xp_buy {
            panic_with_error!(e, LiquidityPoolValidationError::InsufficientBalance);
        }

        let y_w_fee = match xp_buy.checked_sub(dy_w_fee_scaled) {
            Some(y) => y,
            None => panic_with_error!(e, LiquidityPoolValidationError::InsufficientBalance),
        };
        let x = Self::_get_y(&e, j, i, y_w_fee, &xp);

        if x == 0 {
            // pool is empty
            return 0;
        }

        let dx = (x - xp.get(i).unwrap() + 1) / precision_mul.get(i).unwrap();
        dx
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

        let tokens = get_tokens(&e);
        let n_coins = tokens.len();

        if amounts.len() != n_coins {
            panic_with_error!(e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        // Before actual changes were made to the pool, update total rewards data and refresh user reward
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        rewards
            .manager()
            .checkpoint_user(&user, total_shares, user_shares);

        let token_supply = get_total_shares(&e);
        if token_supply == 0 {
            panic_with_error!(&e, LiquidityPoolValidationError::EmptyPool);
        }
        let amp = Self::a(e.clone());
        let mut reserves = get_reserves(&e);

        let old_balances = reserves.clone();
        let mut new_balances = old_balances.clone();

        let d0 = Self::_get_d(&e, &Self::_xp(&e, &old_balances), amp);
        for i in 0..n_coins {
            new_balances.set(i, new_balances.get(i).unwrap() - amounts.get(i).unwrap());
        }

        let d1 = Self::_get_d(&e, &Self::_xp(&e, &new_balances), amp);

        for i in 0..n_coins {
            let new_balance = new_balances.get(i).unwrap();
            let ideal_balance = d1
                .fixed_mul_floor(&e, &U256::from_u128(&e, old_balances.get(i).unwrap()), &d0)
                .to_u128()
                .unwrap();
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
                &(get_fee(&e) as u128 * n_coins as u128),
                &(4 * (n_coins as u128 - 1) * FEE_DENOMINATOR as u128),
            );

            reserves.set(i, new_balance);
            new_balances.set(i, new_balance - fee);
        }
        put_reserves(&e, &reserves);

        let d2 = Self::_get_d(&e, &Self::_xp(&e, &new_balances), amp);

        let mut share_amount = d0
            .sub(&d2)
            .fixed_mul_floor(&e, &U256::from_u128(&e, token_supply), &d0)
            .to_u128()
            .unwrap();
        if share_amount == 0 {
            panic_with_error!(&e, LiquidityPoolValidationError::ZeroSharesBurned);
        }
        share_amount += 1; // In case of rounding errors - make it unfavorable for the "attacker"
        if share_amount > max_burn_amount {
            panic_with_error!(&e, LiquidityPoolValidationError::TooManySharesBurned);
        }

        // First transfer the pool shares that need to be redeemed
        // Burn max amount and mint back change to avoid auth race condition
        burn_shares(&e, &user, max_burn_amount);
        if max_burn_amount > share_amount {
            mint_shares(&e, &user, (max_burn_amount - share_amount) as i128);
        }

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

        // Checkpoint resulting working balance
        rewards.manager().update_working_balance(
            &user,
            total_shares - share_amount,
            user_shares - share_amount,
        );

        // update plane data for every pool update
        update_plane(&e);

        PoolEvents::new(&e).withdraw_liquidity(tokens, amounts, share_amount);

        share_amount
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
        Self::_calc_withdraw_one_coin(&e, share_amount, i).0
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
        let user_shares = get_user_balance_shares(&e, &user);
        rewards
            .manager()
            .checkpoint_user(&user, total_shares, user_shares);

        let (dy, _) = Self::_calc_withdraw_one_coin(&e, share_amount, i);
        if dy < min_amount {
            panic_with_error!(&e, LiquidityPoolValidationError::InMinNotSatisfied);
        }

        let mut reserves = get_reserves(&e);
        reserves.set(i, reserves.get(i).unwrap() - dy);
        put_reserves(&e, &reserves);

        // Redeem shares
        burn_shares(&e, &user, share_amount);

        let coins = get_tokens(&e);
        let token_client = SorobanTokenClient::new(&e, &coins.get(i).unwrap());
        token_client.transfer(&e.current_contract_address(), &user, &(dy as i128));

        // Checkpoint resulting working balance
        rewards.manager().update_working_balance(
            &user,
            total_shares - share_amount,
            user_shares - share_amount,
        );

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

impl LiquidityPool {
    fn _xp(e: &Env, reserves: &Vec<u128>) -> Vec<u128> {
        xp(e, reserves)
    }

    // Calculates the invariant `D` for the given token balances.
    //
    // # Arguments
    //
    // * `xp` - The balances of each token in the pool.
    // * `amp` - The amplification coefficient in the form of A*N**(N-1).
    //
    // # Returns
    //
    // * The invariant `D`.
    fn _get_d(e: &Env, xp: &Vec<u128>, amp: u128) -> U256 {
        let zero = U256::from_u32(e, 0);
        let one = U256::from_u32(e, 1);

        let tokens = get_tokens(e);
        let n_coins = tokens.len();
        let n_coins_256 = U256::from_u32(e, n_coins);

        let mut s = zero.clone();
        for x in xp.iter() {
            s = s.add(&U256::from_u128(e, x));
        }
        if s == zero {
            return zero;
        }

        let mut d_prev;
        let mut d = s.clone();
        let ann = U256::from_u128(e, amp * n_coins as u128);
        for _i in 0..255 {
            let mut d_p = d.clone();
            for x1 in xp.iter() {
                d_p = d_p.fixed_mul_floor(e, &d, &U256::from_u128(e, x1 * n_coins as u128));
            }
            d_prev = d.clone();
            d = ((ann.clone().mul(&s)).add(&(d_p.mul(&n_coins_256)))).fixed_mul_floor(
                e,
                &d,
                &(((ann.clone().sub(&one)).mul(&d)).add(&((n_coins_256.add(&one)).mul(&d_p)))),
            );

            // // Equality with the precision of 1
            if d.clone() > d_prev {
                if d.sub(&d_prev) <= one {
                    return d;
                }
            } else if d_prev.sub(&d) <= one {
                return d;
            }
        }

        // convergence typically occurs in 4 rounds or less, this should be unreachable!
        // if it does happen the pool is borked and LPs can withdraw via `withdraw`
        panic_with_error!(e, LiquidityPoolError::MaxIterationsReached);
    }

    // Calculate x[out_idx] if one makes x[in_idx] = x
    // Done by solving quadratic equation iteratively.
    // x_1**2 + x_1 * (sum' - (A*n**n - 1) * D / (A * n**n)) = D ** (n + 1) / (n ** (2 * n) * prod' * A)
    // x_1**2 + b*x_1 = c
    //
    // x_1 = (x_1**2 + c) / (2*x_1 + b)
    //
    // # Arguments
    //
    // * `i` - The index of the updated token with known balance.
    // * `j` - The index of the updated token with balance to be found.
    // * `x` - The known balance of token x[i].
    // * `xp_` - The balances of each token in the pool.
    //
    // # Returns
    //
    // * The amount of token `j` that will be received.
    fn _get_y(e: &Env, in_idx: u32, out_idx: u32, x: u128, xp: &Vec<u128>) -> u128 {
        // x in the input is converted to the same price/precision
        let tokens = get_tokens(e);
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
        let d = Self::_get_d(e, &xp, amp);
        let mut c = d.clone();
        let mut s = U256::from_u32(e, 0);
        let ann = U256::from_u128(e, amp * n_coins as u128);
        let n_coins_256 = U256::from_u32(e, n_coins);

        let mut x1;
        for i in 0..n_coins {
            if i == in_idx {
                x1 = U256::from_u128(e, x);
            } else if i != out_idx {
                x1 = U256::from_u128(e, xp.get(i).unwrap());
            } else {
                continue;
            }
            s = s.add(&x1);
            c = c.fixed_mul_floor(e, &d, &x1.mul(&n_coins_256));
        }
        let c = c.mul(&d).div(&ann.mul(&n_coins_256));
        let b = s.add(&d.div(&ann)); // - D
        let mut y_prev;
        let mut y = d.clone();
        for _i in 0..255 {
            y_prev = y.clone();
            y = y
                .mul(&y)
                .add(&c)
                .div(&(U256::from_u32(e, 2).mul(&y).add(&b).sub(&d)));

            // Equality with the precision of 1
            if y > y_prev {
                if y.sub(&y_prev) <= U256::from_u32(e, 1) {
                    return y.to_u128().unwrap();
                }
            } else if y_prev.sub(&y) <= U256::from_u32(e, 1) {
                return y.to_u128().unwrap();
            }
        }
        panic_with_error!(e, LiquidityPoolError::MaxIterationsReached);
    }

    // Calculates the amount of token `j` that will be received for swapping `dx` of token `i`.
    //
    // # Arguments
    //
    // * `amp` - The amplification coefficient `Amp = A*N**(N-1)`.
    // * `i` - The index of the token being swapped.
    // * `xp` - The balances of each token in the pool.
    // * `d` - The invariant `D`.
    //
    // # Returns
    //
    // * The amount of token `j` that will be received.
    fn _get_y_d(e: &Env, amp: u128, in_idx: u32, xp: &Vec<u128>, d: U256) -> u128 {
        // Calculate x[i] if one reduces D from being calculated for xp to D
        //
        // Done by solving quadratic equation iteratively.
        // x_1**2 + x1 * (sum' - (A*n**n - 1) * D / (A * n**n)) = D ** (n + 1) / (n ** (2 * n) * prod' * A)
        // x_1**2 + b*x_1 = c
        //
        // x_1 = (x_1**2 + c) / (2*x_1 + b)

        // x in the input is converted to the same price/precision

        let tokens = get_tokens(e);
        let n_coins = tokens.len();

        if in_idx >= n_coins {
            panic_with_error!(e, LiquidityPoolValidationError::InTokenOutOfBounds);
        }

        let mut c = d.clone();
        let mut s = U256::from_u32(e, 0);
        let ann = U256::from_u128(e, amp * n_coins as u128);
        let n_coins_256 = U256::from_u32(e, n_coins);

        let mut x;
        for i in 0..n_coins {
            if i != in_idx {
                x = U256::from_u128(e, xp.get(i).unwrap());
            } else {
                continue;
            }
            s = s.add(&x);
            c = c.fixed_mul_floor(e, &d, &x.mul(&n_coins_256));
        }
        let c = c.mul(&d).div(&ann.mul(&n_coins_256));
        let b = s.add(&d.div(&ann)); // - D
        let mut y_prev;
        let mut y = d.clone();

        for _i in 0..255 {
            y_prev = y.clone();
            y = y
                .mul(&y)
                .add(&c)
                .div(&(U256::from_u32(e, 2).mul(&y).add(&b).sub(&d)));

            // Equality with the precision of 1
            if y > y_prev {
                if y.sub(&y_prev) <= U256::from_u32(e, 1) {
                    return y.to_u128().unwrap();
                }
            } else if y_prev.sub(&y) <= U256::from_u32(e, 1) {
                return y.to_u128().unwrap();
            }
        }
        panic_with_error!(e, LiquidityPoolError::MaxIterationsReached);
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
    fn _calc_withdraw_one_coin(e: &Env, token_amount: u128, token_idx: u32) -> (u128, u128) {
        // First, need to calculate
        // * Get current D
        // * Solve Eqn against y_i for D - token_amount

        let tokens = get_tokens(e);
        let n_coins = tokens.len();

        let amp = Self::a(e.clone());
        let total_supply = get_total_shares(e);

        let xp = Self::_xp(&e, &get_reserves(e));

        let d0 = Self::_get_d(e, &xp, amp);
        let d1 = d0.sub(
            &U256::from_u128(&e, token_amount)
                .mul(&d0)
                .div(&U256::from_u128(e, total_supply)),
        );
        let mut xp_reduced = xp.clone();

        let new_y = Self::_get_y_d(e, amp, token_idx, &xp, d1.clone());
        let token_idx_precision_mul = get_precision_mul(&e).get(token_idx).unwrap();
        let dy_0 = (xp.get(token_idx).unwrap() - new_y) / token_idx_precision_mul; // w/o fees;

        for j in 0..n_coins {
            let dx_expected = if j == token_idx {
                U256::from_u128(e, xp.get(j).unwrap())
                    .mul(&d1)
                    .div(&d0)
                    .to_u128()
                    .unwrap()
                    - new_y
            } else {
                xp.get(j).unwrap()
                    - U256::from_u128(e, xp.get(j).unwrap())
                        .mul(&d1)
                        .div(&d0)
                        .to_u128()
                        .unwrap()
            };
            let fee = dx_expected.fixed_mul_ceil(
                e,
                &((get_fee(e) * n_coins) as u128),
                &((FEE_DENOMINATOR * 4 * (n_coins - 1)) as u128),
            );
            xp_reduced.set(j, xp_reduced.get(j).unwrap() - fee);
        }

        let mut dy =
            xp_reduced.get(token_idx).unwrap() - Self::_get_y_d(e, amp, token_idx, &xp_reduced, d1);
        dy = (dy - 1) / token_idx_precision_mul; // Withdraw less to account for rounding errors

        (dy, dy_0 - dy)
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

    // Starts ramping A to target value in future timestamp.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `future_a` - The target value for A.
    // * `future_time` - The future timestamp when the target value should be reached.
    fn ramp_a(e: Env, admin: Address, future_a: u128, future_time: u64) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);

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

        Events::new(&e).ramp_a(future_a, future_time);
    }

    // Stops ramping A.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn stop_ramp_a(e: Env, admin: Address) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);

        let current_a = Self::a(e.clone());
        put_initial_a(&e, &current_a);
        put_future_a(&e, &current_a);
        put_initial_a_time(&e, &e.ledger().timestamp());
        put_future_a_time(&e, &e.ledger().timestamp());

        // now (block.timestamp < t1) is always False, so we return saved A

        // update plane data for every pool update
        update_plane(&e);

        Events::new(&e).stop_ramp_a(current_a);
    }

    // Sets a new fee to be applied in the future.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `new_fee` - The new fee to be applied.
    fn commit_new_fee(e: Env, admin: Address, new_fee: u32) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);

        if get_admin_actions_deadline(&e) != 0 {
            panic_with_error!(&e, LiquidityPoolError::AnotherActionActive);
        }
        if new_fee > FEE_DENOMINATOR - 1 {
            panic_with_error!(e, LiquidityPoolValidationError::FeeOutOfBounds);
        }

        let deadline = e.ledger().timestamp() + ADMIN_ACTIONS_DELAY;
        put_admin_actions_deadline(&e, &deadline);
        put_future_fee(&e, &new_fee);

        Events::new(&e).commit_new_fee(new_fee);
    }

    // Applies the committed fee.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn apply_new_fee(e: Env, admin: Address) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);

        if e.ledger().timestamp() < get_admin_actions_deadline(&e) {
            panic_with_error!(&e, LiquidityPoolError::ActionNotReadyYet);
        }
        if get_admin_actions_deadline(&e) == 0 {
            panic_with_error!(&e, LiquidityPoolError::NoActionActive);
        }

        put_admin_actions_deadline(&e, &0);
        let fee = get_future_fee(&e);
        put_fee(&e, &fee);

        // update plane data for every pool update
        update_plane(&e);

        Events::new(&e).apply_new_fee(fee);
    }

    // Reverts the committed parameters to their current values.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn revert_new_parameters(e: Env, admin: Address) {
        admin.require_auth();
        require_operations_admin_or_owner(&e, &admin);

        put_admin_actions_deadline(&e, &0);

        Events::new(&e).revert_new_parameters();
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
}

#[contractimpl]
impl ManagedLiquidityPool for LiquidityPool {
    // Initializes all the necessary parameters for the liquidity pool.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `privileged_addrs` - (
    //      emergency admin,
    //      rewards admin,
    //      operations admin,
    //      pause admin,
    //      emergency pause admins
    //  ).
    // * `router` - The address of the router.
    // * `token_wasm_hash` - The hash of the token's WASM code.
    // * `coins` - The addresses of the coins.
    // * `amp` - The amplification coefficient. Amp = A*N**(N-1)
    // * `fee` - The fee to be applied.
    // * `reward_config` - (
    // *    `reward_token` - The address of the reward token.
    // *    `reward_boost_token` - The address of the reward boost token.
    // *    `reward_boost_feed` - The address of the reward boost feed.
    // * )
    // * `plane` - The address of the plane.
    fn initialize_all(
        e: Env,
        admin: Address,
        privileged_addrs: (Address, Address, Address, Address, Vec<Address>),
        router: Address,
        token_wasm_hash: BytesN<32>,
        coins: Vec<Address>,
        amp: u128,
        fee: u32,
        reward_config: (Address, Address, Address),
        plane: Address,
    ) {
        let (reward_token, reward_boost_token, reward_boost_feed) = reward_config;

        // merge whole initialize process into one because lack of caching of VM components
        // https://github.com/stellar/rs-soroban-env/issues/827
        Self::init_pools_plane(e.clone(), plane);
        Self::initialize(
            e.clone(),
            admin,
            privileged_addrs,
            router,
            token_wasm_hash,
            coins,
            amp,
            fee,
        );
        Self::initialize_boost_config(e.clone(), reward_boost_token, reward_boost_feed);
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
    // * `privileged_addrs` - (
    //      emergency admin,
    //      rewards admin,
    //      operations admin,
    //      pause admin,
    //      emergency pause admins
    //  ).
    // * `router` - The address of the router.
    // * `token_wasm_hash` - The hash of the token's WASM code.
    // * `tokens` - The addresses of the coins.
    // * `amp` - The amplification coefficient. Amp = A*N**(N-1)
    // * `fee` - The fee to be applied.
    fn initialize(
        e: Env,
        admin: Address,
        privileged_addrs: (Address, Address, Address, Address, Vec<Address>),
        router: Address,
        token_wasm_hash: BytesN<32>,
        tokens: Vec<Address>,
        amp: u128,
        fee: u32,
    ) {
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

        set_router(&e, &router);

        // 0.01% = 1; 1% = 100; 0.3% = 30
        if fee > FEE_DENOMINATOR - 1 {
            panic_with_error!(&e, LiquidityPoolValidationError::FeeOutOfBounds);
        }

        put_fee(&e, &fee);

        put_tokens(&e, &tokens);
        put_decimals(&e, &read_decimals(&e, &tokens));

        // LP token
        let share_contract = create_contract(&e, token_wasm_hash, &tokens);
        LPToken::new(&e, &share_contract).initialize(
            &e.current_contract_address(),
            &7u32,
            &"Pool Share Token".into_val(&e),
            &"POOL".into_val(&e),
        );
        put_token_share(&e, share_contract);
        let mut initial_reserves = Vec::new(&e);
        for _i in 0..tokens.len() {
            initial_reserves.push_back(0_u128);
        }
        put_reserves(&e, &initial_reserves);

        // pool config
        put_initial_a(&e, &amp);
        put_initial_a_time(&e, &e.ledger().timestamp());
        put_future_a(&e, &amp);
        put_future_a_time(&e, &e.ledger().timestamp());
        put_admin_actions_deadline(&e, &0);

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

    // Returns the pools's decimals.
    //
    // # Returns
    //
    // A vector of token decimals the same order as the tokens.
    fn get_decimals(e: Env) -> Vec<u32> {
        get_decimals(&e)
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

        if get_is_killed_deposit(&e) {
            panic_with_error!(e, LiquidityPoolError::PoolDepositKilled);
        }

        let tokens = get_tokens(&e);
        let n_coins = tokens.len();

        if amounts.len() != n_coins {
            panic_with_error!(e, LiquidityPoolValidationError::WrongInputVecSize);
        }

        // Before actual changes were made to the pool, update total rewards data and refresh/initialize user reward
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        rewards
            .manager()
            .checkpoint_user(&user, total_shares, user_shares);

        let amp = Self::a(e.clone());

        let token_supply = get_total_shares(&e);
        // Initial invariant
        let mut d0 = U256::from_u32(&e, 0);
        let old_balances = get_reserves(&e);
        if token_supply > 0 {
            d0 = Self::_get_d(&e, &Self::_xp(&e, &old_balances), amp);
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
        let d1 = Self::_get_d(&e, &Self::_xp(&e, &new_balances), amp);
        if d1 <= d0 {
            panic_with_error!(&e, LiquidityPoolError::InvariantDoesNotHold);
        }

        // We need to recalculate the invariant accounting for fees
        // to calculate fair user's share
        let mut d2 = d1.clone();
        let balances = if token_supply > 0 {
            let mut result = new_balances.clone();
            // Only account for fees if we are not the first to deposit
            for i in 0..n_coins {
                let new_balance = new_balances.get(i).unwrap();
                let ideal_balance = d1
                    .mul(&U256::from_u128(&e, old_balances.get(i).unwrap()))
                    .div(&d0)
                    .to_u128()
                    .unwrap();
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
                    &(get_fee(&e) as u128 * n_coins as u128),
                    &(FEE_DENOMINATOR as u128 * 4 * (n_coins as u128 - 1)),
                );

                result.set(i, new_balance);
                new_balances.set(i, new_balances.get(i).unwrap() - fee);
            }
            d2 = Self::_get_d(&e, &Self::_xp(&e, &new_balances), amp);
            result
        } else {
            new_balances
        };
        put_reserves(&e, &balances);

        // Calculate, how much pool tokens to mint
        let mint_amount = if token_supply == 0 {
            d1.to_u128().unwrap() // Take the dust if there was any
        } else {
            U256::from_u128(&e, token_supply)
                .mul(&d2.sub(&d0))
                .div(&d0)
                .to_u128()
                .unwrap()
        };

        if mint_amount < min_shares {
            panic_with_error!(&e, LiquidityPoolValidationError::OutMinNotSatisfied);
        }
        // Mint pool tokens
        mint_shares(&e, &user, mint_amount as i128);

        // Checkpoint resulting working balance
        rewards.manager().update_working_balance(
            &user,
            total_shares + mint_amount,
            user_shares + mint_amount,
        );

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
        if get_is_killed_swap(&e) {
            panic_with_error!(e, LiquidityPoolError::PoolSwapKilled);
        }

        if in_amount == 0 {
            panic_with_error!(e, LiquidityPoolValidationError::ZeroAmount);
        }

        let precision_mul = get_precision_mul(&e);
        let old_balances = get_reserves(&e);
        let xp = Self::_xp(&e, &old_balances);

        let coins = get_tokens(&e);
        let input_coin = coins.get(in_idx).unwrap();

        let token_client = SorobanTokenClient::new(&e, &input_coin);
        token_client.transfer(&user, &e.current_contract_address(), &(in_amount as i128));

        let reserve_sell = old_balances.get(in_idx).unwrap();
        let reserve_buy = old_balances.get(out_idx).unwrap();
        if reserve_sell == 0 || reserve_buy == 0 {
            panic_with_error!(e, LiquidityPoolValidationError::EmptyPool);
        }

        let x = xp.get(in_idx).unwrap() + in_amount * precision_mul.get(in_idx).unwrap();
        let y = Self::_get_y(&e, in_idx, out_idx, x, &xp);

        let dy = xp.get(out_idx).unwrap() - y - 1; // -1 just in case there were some rounding errors
        let dy_fee = dy.fixed_mul_ceil(&e, &(get_fee(&e) as u128), &(FEE_DENOMINATOR as u128));

        // Convert all to real units
        let dy = (dy - dy_fee) / precision_mul.get(out_idx).unwrap();
        if dy < out_min {
            panic_with_error!(e, LiquidityPoolValidationError::OutMinNotSatisfied);
        }

        // Change balances exactly in same way as we change actual ERC20 coin amounts
        let mut reserves = get_reserves(&e);
        reserves.set(in_idx, old_balances.get(in_idx).unwrap() + in_amount);
        reserves.set(out_idx, old_balances.get(out_idx).unwrap() - dy);
        put_reserves(&e, &reserves);

        let token_out = coins.get(out_idx).unwrap();
        let token_client = SorobanTokenClient::new(&e, &token_out);
        token_client.transfer(&e.current_contract_address(), &user, &(dy as i128));

        // update plane data for every pool update
        update_plane(&e);

        // since we need fee in amount sent to the pool, calculate it here
        let dx_fee =
            in_amount.fixed_mul_ceil(&e, &(get_fee(&e) as u128), &(FEE_DENOMINATOR as u128));
        PoolEvents::new(&e).trade(user, input_coin, token_out, in_amount, dy, dx_fee);

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

    // Swaps tokens in the pool, receiving fixed amount of out tokens.
    //
    // # Arguments
    //
    // * `user` - The address of the user swapping the tokens.
    // * `in_idx` - The index of the input token to be swapped.
    // * `out_idx` - The index of the output token to be received.
    // * `out_amount` - The amount of the output token to be received.
    // * `in_max` - The maximum amount of the input token to be sent.
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

        if out_amount == 0 {
            panic_with_error!(e, LiquidityPoolValidationError::ZeroAmount);
        }

        let precision_mul = get_precision_mul(&e);
        let old_balances = get_reserves(&e);
        let xp = Self::_xp(&e, &old_balances);

        let coins = get_tokens(&e);
        let input_coin = coins.get(in_idx).unwrap();

        let token_client = SorobanTokenClient::new(&e, &input_coin);
        token_client.transfer(&user, &e.current_contract_address(), &(in_max as i128));

        let reserve_sell = old_balances.get(in_idx).unwrap();
        let reserve_buy = old_balances.get(out_idx).unwrap();
        if reserve_sell == 0 || reserve_buy == 0 {
            panic_with_error!(e, LiquidityPoolValidationError::EmptyPool);
        }

        // apply fee to dy to keep swap symmetrical
        // we'll transfer to user the amount he wants to receive, however calculation will be done with fee
        let dy_w_fee = out_amount.fixed_mul_ceil(
            &e,
            &(FEE_DENOMINATOR as u128),
            &((FEE_DENOMINATOR - get_fee(&e)) as u128),
        );

        // if total value including fee is more than the reserve, math can't be done properly
        if dy_w_fee >= reserve_buy {
            panic_with_error!(e, LiquidityPoolValidationError::InsufficientBalance);
        }

        let y_w_fee = match xp
            .get(out_idx)
            .unwrap()
            .checked_sub(dy_w_fee * precision_mul.get(out_idx).unwrap())
        {
            Some(y) => y,
            None => panic_with_error!(e, LiquidityPoolValidationError::InsufficientBalance),
        };
        let x = Self::_get_y(&e, out_idx, in_idx, y_w_fee, &xp);

        // +1 just in case there were some rounding errors & convert to real units in place
        let dx = (x - xp.get(in_idx).unwrap() + 1) / precision_mul.get(in_idx).unwrap();
        if dx > in_max {
            panic_with_error!(e, LiquidityPoolValidationError::InMaxNotSatisfied);
        }

        // return excess input tokens
        let tokens_to_return = in_max - dx;
        if tokens_to_return > 0 {
            token_client.transfer(
                &e.current_contract_address(),
                &user,
                &(tokens_to_return as i128),
            );
        }

        // Update reserves
        let mut reserves = get_reserves(&e);
        reserves.set(in_idx, old_balances.get(in_idx).unwrap() + dx);
        reserves.set(out_idx, old_balances.get(out_idx).unwrap() - out_amount);
        put_reserves(&e, &reserves);

        let token_out = coins.get(out_idx).unwrap();
        let token_client = SorobanTokenClient::new(&e, &token_out);
        token_client.transfer(&e.current_contract_address(), &user, &(out_amount as i128));

        // update plane data for every pool update
        update_plane(&e);

        PoolEvents::new(&e).trade(
            user,
            input_coin,
            token_out,
            dx,
            out_amount,
            dy_w_fee - out_amount,
        );

        dx
    }

    // Estimate amount of coins to retrieve using swap_strict_receive function
    //
    // # Arguments
    //
    // * `in_idx` - The index of the input token to be swapped.
    // * `out_idx` - The index of the output token to be received.
    // * `out_amount` - The amount of the output token to be received.
    //
    // # Returns
    //
    // The estimated amount of the input token that would be sent.
    fn estimate_swap_strict_receive(e: Env, in_idx: u32, out_idx: u32, out_amount: u128) -> u128 {
        Self::get_dx(e, in_idx, out_idx, out_amount)
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
        let user_shares = get_user_balance_shares(&e, &user);
        rewards
            .manager()
            .checkpoint_user(&user, total_shares, user_shares);

        let total_supply = get_total_shares(&e);
        let mut amounts: Vec<u128> = Vec::new(&e);
        let mut reserves = get_reserves(&e);
        let coins = get_tokens(&e);

        for i in 0..n_coins {
            let value = reserves
                .get(i)
                .unwrap()
                .fixed_mul_floor(&e, &share_amount, &total_supply);
            if value < min_amounts.get(i).unwrap() {
                panic_with_error!(&e, LiquidityPoolValidationError::OutMinNotSatisfied);
            }
            reserves.set(i, reserves.get(i).unwrap() - value);
            amounts.push_back(value);

            let token_client = SorobanTokenClient::new(&e, &coins.get(i).unwrap());
            token_client.transfer(&e.current_contract_address(), &user, &(value as i128));
        }
        put_reserves(&e, &reserves);

        // Redeem shares
        burn_shares(&e, &user, share_amount);

        // Checkpoint resulting working balance
        rewards.manager().update_working_balance(
            &user,
            total_shares - share_amount,
            user_shares - share_amount,
        );

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

// The `UpgradeableContract` trait provides the interface for upgrading the contract.
#[contractimpl]
impl UpgradeableContract for LiquidityPool {
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
    // * `new_token_wasm_hash` - The new token wasm hash to commit.
    fn commit_upgrade(
        e: Env,
        admin: Address,
        new_wasm_hash: BytesN<32>,
        token_new_wasm_hash: BytesN<32>,
    ) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
        commit_upgrade(&e, &new_wasm_hash);
        // handle token upgrade manually together with pool upgrade
        set_token_future_wasm(&e, &token_new_wasm_hash);

        UpgradeEvents::new(&e).commit_upgrade(Vec::from_array(
            &e,
            [new_wasm_hash.clone(), token_new_wasm_hash.clone()],
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
impl UpgradeableLPTokenTrait for LiquidityPool {
    // legacy upgrade. not compatible with token contract version 140+ due to different arguments
    fn upgrade_token_legacy(e: Env, admin: Address, new_token_wasm: BytesN<32>) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);

        e.invoke_contract::<()>(
            &get_token_share(&e),
            &symbol_short!("upgrade"),
            Vec::from_array(&e, [new_token_wasm.to_val()]),
        );
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

        // either rewards admin or router can set the rewards config
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

        match get_tokens(&e).first_index_of(reward_token) {
            Some(idx) => {
                // since reward token is in the reserves, we need to keep also the reserves value
                reward_balance_to_keep += get_reserves(&e).get(idx).unwrap();
            }
            None => {}
        };

        reward_balance.saturating_sub(reward_balance_to_keep)
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
                (symbol_short!("supply"), total_shares as i128),
                (
                    Symbol::new(&e, "working_balance"),
                    manager.get_working_balance(&user, user_shares) as i128,
                ),
                (
                    Symbol::new(&e, "working_supply"),
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

    // Checkpoints the reward for the user.
    // Useful when user moves funds by itself to avoid re-entrancy issue.
    // Can be called only by the token contract to notify pool external changes happened.
    fn checkpoint_reward(e: Env, token_contract: Address, user: Address, user_shares: u128) {
        // checkpoint reward with provided values to avoid re-entrancy issue
        token_contract.require_auth();
        if token_contract != get_token_share(&e) {
            panic_with_error!(&e, AccessControlError::Unauthorized);
        }
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        rewards
            .manager()
            .checkpoint_user(&user, total_shares, user_shares);
    }

    // Checkpoints total working balance and the working balance for the user.
    // Useful when user moves funds by itself to avoid re-entrancy issue.
    // Can be called only by the token contract to notify pool external changes happened.
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
        rewards
            .manager()
            .update_working_balance(&user, total_shares, user_shares);
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
        let mut rewards_manager = rewards.manager();
        let rewards_storage = rewards.storage();
        let reward = rewards_manager.claim_reward(&user, total_shares, user_shares);

        // validate reserves after claim - they should be less than or equal to the balance
        let tokens = Self::get_tokens(e.clone());
        let reward_token = rewards_storage.get_reward_token();
        let reserves = Self::get_reserves(e.clone());

        for i in 0..reserves.len() {
            let token = tokens.get(i).unwrap();
            if token != reward_token {
                continue;
            }

            let balance = SorobanTokenClient::new(&e, &tokens.get(i).unwrap())
                .balance(&e.current_contract_address()) as u128;
            if reserves.get(i).unwrap() > balance {
                panic_with_error!(&e, LiquidityPoolValidationError::InsufficientBalance);
            }
        }

        RewardEvents::new(&e).claim(user, reward_token, reward);

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
