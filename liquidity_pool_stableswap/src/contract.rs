use crate::pool_constants::{
    ADMIN_ACTIONS_DELAY, FEE_DENOMINATOR, KILL_DEADLINE_DT, LENDING_PRECISION, MAX_A,
    MAX_ADMIN_FEE, MAX_A_CHANGE, MAX_FEE, MIN_RAMP_TIME, N_COINS, PRECISION, PRECISION_MUL, RATES,
};
use crate::pool_interface::{
    AdminInterfaceTrait, InternalInterfaceTrait, LiquidityPoolInterfaceTrait, LiquidityPoolTrait,
    RewardsTrait, UpgradeableContractTrait,
};
use crate::storage::{
    get_admin_actions_deadline, get_admin_fee, get_fee, get_future_a, get_future_a_time,
    get_future_admin_fee, get_future_fee, get_initial_a, get_initial_a_time, get_is_killed,
    get_kill_deadline, get_reserves, get_tokens, get_transfer_ownership_deadline,
    put_admin_actions_deadline, put_admin_fee, put_fee, put_future_a, put_future_a_time,
    put_future_admin_fee, put_future_fee, put_initial_a, put_initial_a_time, put_is_killed,
    put_kill_deadline, put_reserves, put_tokens, put_transfer_ownership_deadline,
};
use crate::token::create_contract;
use token_share::{
    burn_shares, get_token_share, get_total_shares, get_user_balance_shares, mint_shares,
    put_token_share, Client as LPToken,
};

use crate::rewards::get_rewards_manager;
use access_control::access::{AccessControl, AccessControlTrait};
use cast::i128 as to_i128;
use rewards::{storage::PoolRewardConfig, storage::RewardsStorageTrait};
use soroban_sdk::{
    contract, contractimpl, contractmeta, symbol_short, token::Client, Address, BytesN, Env,
    IntoVal, Map, Symbol, Vec,
};
use utils::bump::bump_instance;

contractmeta!(
    key = "Description",
    val = "Stable Swap AMM for two pairs of tokens"
);

#[contract]
pub struct LiquidityPool;

#[contractimpl]
impl LiquidityPoolTrait for LiquidityPool {
    fn a(e: Env) -> u128 {
        // Handle ramping A up or down
        let t1 = get_future_a_time(&e) as u128;
        let a1 = get_future_a(&e);
        let now = e.ledger().timestamp() as u128;

        return if now < t1 {
            let a0 = get_initial_a(&e);
            let t0 = get_initial_a_time(&e) as u128;
            // Expressions in u128 cannot have negative numbers, thus "if"
            if a1 > a0 {
                a0 + (a1 - a0) * (now - t0) / (t1 - t0)
            } else {
                a0 - (a0 - a1) * (now - t0) / (t1 - t0)
            }
        } else {
            // when t1 == 0 or block.timestamp >= t1
            a1
        };
    }

    fn get_virtual_price(e: Env) -> u128 {
        let d = Self::get_d(e.clone(), Self::xp(e.clone()), Self::a(e.clone()));
        // D is in the units similar to DAI (e.g. converted to precision 1e7)
        // When balanced, D = n * x_u - total virtual value of the portfolio
        let token_supply = get_total_shares(&e);
        return d * PRECISION / token_supply as u128;
    }

    fn calc_token_amount(e: Env, amounts: Vec<u128>, deposit: bool) -> u128 {
        let mut balances = get_reserves(&e);
        let amp = Self::a(e.clone());
        let d0 = Self::get_d_mem(e.clone(), balances.clone(), amp);
        for i in 0..N_COINS as u32 {
            if deposit {
                balances.set(i, balances.get(i).unwrap() + amounts.get(i).unwrap());
            } else {
                balances.set(i, balances.get(i).unwrap() - amounts.get(i).unwrap());
            }
        }
        let d1 = Self::get_d_mem(e.clone(), balances, amp);
        let token_amount = get_total_shares(&e);
        let diff = if deposit { d1 - d0 } else { d0 - d1 };
        return diff * token_amount as u128 / d0;
    }

    fn get_dy(e: Env, i: u32, j: u32, dx: u128) -> u128 {
        // dx and dy in c-units
        let rates = RATES;
        let xp = Self::xp(e.clone());

        let x = xp.get(i as u32).unwrap() + (dx * rates[i as usize] / PRECISION);
        let y = Self::get_y(e.clone(), i, j, x, xp.clone());
        let dy = (xp.get(j as u32).unwrap() - y - 1) * PRECISION / rates[j as usize];
        let fee = get_fee(&e) * dy / FEE_DENOMINATOR;
        return dy - fee;
    }

    fn get_dy_underlying(e: Env, i: u32, j: u32, dx: u128) -> u128 {
        // dx and dy in underlying units
        let xp = Self::xp(e.clone());
        let precisions = PRECISION_MUL;

        let x = xp.get(i as u32).unwrap() + dx * precisions[i as usize];
        let y = Self::get_y(e.clone(), i, j, x, xp.clone());
        let dy = (xp.get(j as u32).unwrap() - y - 1) / precisions[j as usize];
        let fee = get_fee(&e) * dy / FEE_DENOMINATOR;
        return dy - fee;
    }

    fn remove_liquidity_imbalance(
        e: Env,
        user: Address,
        amounts: Vec<u128>,
        max_burn_amount: u128,
    ) -> u128 {
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
            panic!("is killed")
        }

        let token_supply = get_total_shares(&e) as u128;
        if token_supply == 0 {
            panic!("zero total supply")
        }
        let _fee = get_fee(&e) * N_COINS as u128 / (4 * (N_COINS as u128 - 1));
        let _admin_fee = get_admin_fee(&e);
        let amp = Self::a(e.clone());
        let mut reserves = get_reserves(&e);

        let old_balances = reserves.clone();
        let mut new_balances = old_balances.clone();

        let d0 = Self::get_d_mem(e.clone(), old_balances.clone(), amp);
        for i in 0..N_COINS as u32 {
            new_balances.set(i, new_balances.get(i).unwrap() - amounts.get(i).unwrap());
        }

        let d1 = Self::get_d_mem(e.clone(), new_balances.clone(), amp);
        let mut fees = Vec::from_array(&e, [0; N_COINS]);

        for i in 0..N_COINS as u32 {
            let ideal_balance = d1 * old_balances.get(i).unwrap() / d0;
            let difference = if ideal_balance > new_balances.get(i).unwrap() {
                ideal_balance - new_balances.get(i).unwrap()
            } else {
                new_balances.get(i).unwrap() - ideal_balance
            };
            fees.set(i, _fee * difference / FEE_DENOMINATOR);
            reserves.set(
                i,
                new_balances.get(i).unwrap()
                    - (fees.get(i).unwrap() * _admin_fee / FEE_DENOMINATOR),
            );
            new_balances.set(i, new_balances.get(i).unwrap() - fees.get(i).unwrap());
        }
        put_reserves(&e, &reserves);

        let d2 = Self::get_d_mem(e.clone(), new_balances, amp);

        let mut token_amount = (d0 - d2) * token_supply / d0;
        if token_amount == 0 {
            panic!("zero tokens burned")
        }
        token_amount += 1; // In case of rounding errors - make it unfavorable for the "attacker"
        if token_amount > max_burn_amount {
            panic!("Slippage screwed you")
        }

        // First transfer the pool shares that need to be redeemed
        let share_token_client = LPToken::new(&e, &get_token_share(&e));
        share_token_client.transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &(token_amount as i128),
        );
        burn_shares(&e, token_amount as i128);

        for i in 0..N_COINS as u32 {
            if amounts.get(i).unwrap() != 0 {
                let coins = get_tokens(&e);
                let token_client = Client::new(&e, &coins.get(i).unwrap());
                token_client.transfer(
                    &e.current_contract_address(),
                    &user,
                    &(amounts.get(i).unwrap() as i128),
                );
            }
        }

        token_amount
    }

    fn calc_withdraw_one_coin(e: Env, token_amount: u128, i: u32) -> u128 {
        return Self::internal_calc_withdraw_one_coin(e, token_amount, i).0;
    }

    fn withdraw_one_coin(e: Env, user: Address, token_amount: u128, i: u32, min_amount: u128) {
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
            panic!("is killed")
        }

        let (dy, dy_fee) = Self::internal_calc_withdraw_one_coin(e.clone(), token_amount, i);
        if !(dy >= min_amount) {
            panic!("Not enough coins removed")
        }

        let mut reserves = get_reserves(&e);
        reserves.set(
            i as u32,
            reserves.get(i as u32).unwrap() - (dy + dy_fee * get_admin_fee(&e) / FEE_DENOMINATOR),
        );
        put_reserves(&e, &reserves);

        // First transfer the pool shares that need to be redeemed
        let share_token_client = LPToken::new(&e, &get_token_share(&e));
        share_token_client.transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &(token_amount as i128),
        );
        burn_shares(&e, token_amount as i128);

        let coins = get_tokens(&e);
        let token_client = Client::new(&e, &coins.get(i as u32).unwrap());
        token_client.transfer(&e.current_contract_address(), &user, &(dy as i128));
    }
}

impl InternalInterfaceTrait for LiquidityPool {
    fn xp(e: Env) -> Vec<u128> {
        let reserves = get_reserves(&e);
        let mut result = Vec::from_array(&e, RATES);
        for i in 0..N_COINS as u32 {
            result.set(
                i,
                result.get(i).unwrap() * reserves.get(i).unwrap() / LENDING_PRECISION,
            );
        }
        return result;
    }

    // balances size = N_COINS
    fn xp_mem(e: Env, reserves: Vec<u128>) -> Vec<u128> {
        let mut result = Vec::from_array(&e, RATES);
        for i in 0..N_COINS as u32 {
            result.set(
                i,
                result.get(i).unwrap() * reserves.get(i).unwrap() / PRECISION,
            );
        }
        return result;
    }

    // xp size = N_COINS
    fn get_d(_e: Env, xp: Vec<u128>, amp: u128) -> u128 {
        let mut s = 0;
        for x in xp.clone() {
            s += x;
        }
        if s == 0 {
            return 0;
        }

        let mut d_prev;
        let mut d = s;
        let ann = amp * N_COINS as u128;
        for _i in 0..255 {
            let mut d_p = d;
            for _x in xp.clone() {
                d_p = d_p * d / (_x * N_COINS as u128) // If division by 0, this will be borked: only withdrawal will work. And that is good
            }
            d_prev = d;
            d = (ann * s + d_p * N_COINS as u128) * d
                / ((ann - 1) * d + (N_COINS as u128 + 1) * d_p);
            // // Equality with the precision of 1
            if d > d_prev {
                if d - d_prev <= 1 {
                    break;
                }
            } else {
                if d_prev - d <= 1 {
                    break;
                }
            }
        }
        return d;
    }

    fn get_d_mem(e: Env, balances: Vec<u128>, amp: u128) -> u128 {
        Self::get_d(e.clone(), Self::xp_mem(e.clone(), balances), amp)
    }

    fn get_y(e: Env, i: u32, j: u32, x: u128, xp_: Vec<u128>) -> u128 {
        // x in the input is converted to the same price/precision

        if !(i != j) {
            panic!("same coin")
        } // dev: same coin
          // if !(j >= 0) {
          //     panic!("j below zero")
          // } // dev: j below zero
        if !(j < N_COINS as u32) {
            panic!("j above N_COINS")
        } // dev: j above N_COINS

        // should be unreachable, but good for safety
        // if !(i >= 0) {
        //     panic!("bad arguments")
        // }
        if !(i < N_COINS as u32) {
            panic!("bad arguments")
        }

        let amp = Self::a(e.clone());
        let d = Self::get_d(e.clone(), xp_.clone(), amp);
        let mut c = d;
        let mut s = 0;
        let ann = amp * N_COINS as u128;

        let mut _x = 0;
        for _i in 0..N_COINS as u32 {
            if _i == i {
                _x = x;
            } else if _i != j {
                _x = xp_.get(_i).unwrap();
            } else {
                continue;
            }
            s += _x;
            c = c * d / (_x * N_COINS as u128);
        }
        c = c * d / (ann * N_COINS as u128);
        let b = s + d / ann; // - D
        let mut y_prev;
        let mut y = d;
        for _i in 0..255 {
            y_prev = y;
            y = (y * y + c) / (2 * y + b - d);
            // Equality with the precision of 1
            if y > y_prev {
                if y - y_prev <= 1 {
                    break;
                }
            } else {
                if y_prev - y <= 1 {
                    break;
                }
            }
        }
        return y;
    }

    fn get_y_d(_e: Env, a: u128, i: u32, xp: Vec<u128>, d: u128) -> u128 {
        // Calculate x[i] if one reduces D from being calculated for xp to D
        //
        // Done by solving quadratic equation iteratively.
        // x_1**2 + x1 * (sum' - (A*n**n - 1) * D / (A * n**n)) = D ** (n + 1) / (n ** (2 * n) * prod' * A)
        // x_1**2 + b*x_1 = c
        //
        // x_1 = (x_1**2 + c) / (2*x_1 + b)

        // x in the input is converted to the same price/precision

        // if !(i >= 0) {
        //     panic!("i below zero")
        // }
        if !(i < N_COINS as u32) {
            panic!("i above N_COINS")
        }

        let mut c = d;
        let mut s = 0;
        let ann = a * N_COINS as u128;

        let mut _x = 0;
        for _i in 0..N_COINS as u32 {
            if _i != i as u32 {
                _x = xp.get(_i).unwrap();
            } else {
                continue;
            }
            s += _x;
            c = c * d / (_x * N_COINS as u128);
        }
        c = c * d / (ann * N_COINS as u128);

        let b = s + d / ann;
        let mut y_prev;
        let mut y = d;

        for _i in 0..255 {
            y_prev = y;
            y = (y * y + c) / (2 * y + b - d);

            // Equality with the precision of 1
            if y > y_prev {
                if y - y_prev <= 1 {
                    break;
                }
            } else {
                if y_prev - y <= 1 {
                    break;
                }
            }
        }
        return y;
    }

    fn internal_calc_withdraw_one_coin(e: Env, token_amount: u128, i: u32) -> (u128, u128) {
        // First, need to calculate
        // * Get current D
        // * Solve Eqn against y_i for D - token_amount

        let amp = Self::a(e.clone());
        let _fee = get_fee(&e) * N_COINS as u128 / (4 * (N_COINS as u128 - 1));
        let precisions = PRECISION_MUL;
        let total_supply = get_total_shares(&e) as u128;

        let xp = Self::xp(e.clone());

        let d0 = Self::get_d(e.clone(), xp.clone(), amp);
        let d1 = d0 - token_amount * d0 / total_supply;
        let mut xp_reduced = xp.clone();

        let new_y = Self::get_y_d(e.clone(), amp, i, xp.clone(), d1);
        let dy_0 = (xp.get(i as u32).unwrap() - new_y) / precisions[i as usize]; // w/o fees;

        for j in 0..N_COINS as u32 {
            let dx_expected = if j == i as u32 {
                xp.get(j).unwrap() * d1 / d0 - new_y
            } else {
                xp.get(j).unwrap() - xp.get(j).unwrap() * d1 / d0
            };
            xp_reduced.set(
                j,
                xp_reduced.get(j).unwrap() - _fee * dx_expected / FEE_DENOMINATOR,
            );
        }

        let mut dy = xp_reduced.get(i as u32).unwrap()
            - Self::get_y_d(e.clone(), amp, i, xp_reduced.clone(), d1);
        dy = (dy - 1) / precisions[i as usize]; // Withdraw less to account for rounding errors

        return (dy, dy_0 - dy);
    }
}

#[contractimpl]
impl AdminInterfaceTrait for LiquidityPool {
    fn ramp_a(e: Env, admin: Address, future_a: u128, future_time: u64) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);
        if !(e.ledger().timestamp() >= get_initial_a_time(&e) + MIN_RAMP_TIME) {
            panic!("")
        };
        if !(future_time >= e.ledger().timestamp() + MIN_RAMP_TIME) {
            panic!("insufficient time")
        };

        let initial_a = Self::a(e.clone());
        if !((future_a > 0) && (future_a < MAX_A)) {
            panic!("")
        }
        if !(((future_a >= initial_a) && (future_a <= initial_a * MAX_A_CHANGE))
            || ((future_a < initial_a) && (future_a * MAX_A_CHANGE >= initial_a)))
        {
            panic!("")
        }
        put_initial_a(&e, &initial_a);
        put_future_a(&e, &future_a);
        put_initial_a_time(&e, &e.ledger().timestamp());
        put_future_a_time(&e, &future_time);
    }

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
    }

    fn commit_new_fee(e: Env, admin: Address, new_fee: u128, new_admin_fee: u128) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        if !(get_admin_actions_deadline(&e) == 0) {
            panic!("active action")
        }
        if !(new_fee <= MAX_FEE) {
            panic!("fee exceeds maximum")
        }
        if !(new_admin_fee <= MAX_ADMIN_FEE) {
            panic!("admin fee exceeds maximum")
        }

        let _deadline = e.ledger().timestamp() + ADMIN_ACTIONS_DELAY;
        put_admin_actions_deadline(&e, &_deadline);
        put_future_fee(&e, &new_fee);
        put_future_admin_fee(&e, &new_admin_fee);
    }

    fn apply_new_fee(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        if e.ledger().timestamp() >= get_admin_actions_deadline(&e) {
            panic!("insufficient time")
        }
        if get_admin_actions_deadline(&e) != 0 {
            panic!("no active action")
        }

        put_admin_actions_deadline(&e, &0);
        let fee = get_future_fee(&e);
        let admin_fee = get_future_admin_fee(&e);
        put_fee(&e, &fee);
        put_admin_fee(&e, &admin_fee);
    }

    fn revert_new_parameters(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        put_admin_actions_deadline(&e, &0);
    }

    fn commit_transfer_ownership(e: Env, admin: Address, new_admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        // assert self.transfer_ownership_deadline == 0  # dev: active transfer

        let deadline = e.ledger().timestamp() + ADMIN_ACTIONS_DELAY;
        put_transfer_ownership_deadline(&e, &deadline);
        access_control.set_future_admin(&new_admin);
    }

    fn apply_transfer_ownership(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        if e.ledger().timestamp() >= get_transfer_ownership_deadline(&e) {
            panic!("insufficient time")
        }
        if get_transfer_ownership_deadline(&e) != 0 {
            panic!("no active transfer")
        }

        put_transfer_ownership_deadline(&e, &0);
        let future_admin = access_control
            .get_future_admin()
            .expect("Try get future admin");
        access_control.set_admin(&future_admin);
    }

    fn revert_transfer_ownership(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        put_transfer_ownership_deadline(&e, &0);
    }

    fn admin_balances(e: Env, i: u32) -> u128 {
        let coins = get_tokens(&e);
        let token_client = Client::new(&e, &coins.get(i).unwrap());
        let balance = token_client.balance(&e.current_contract_address()) as u128;
        let reserves = get_reserves(&e);

        balance - reserves.get(i).unwrap()
    }

    fn withdraw_admin_fees(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        let coins = get_tokens(&e);
        let reserves = get_reserves(&e);

        for i in 0..N_COINS as u32 {
            let token_client = Client::new(&e, &coins.get(i).unwrap());
            let balance = token_client.balance(&e.current_contract_address()) as u128;

            let value = balance - reserves.get(i).unwrap();
            if value > 0 {
                token_client.transfer(&e.current_contract_address(), &admin, &(value as i128));
            }
        }
    }

    fn donate_admin_fees(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        let coins = get_tokens(&e);
        let mut reserves = get_reserves(&e);

        for i in 0..N_COINS as u32 {
            let token_client = Client::new(&e, &coins.get(i).unwrap());
            let balance = token_client.balance(&e.current_contract_address());
            reserves.set(i, balance as u128);
        }
        put_reserves(&e, &reserves);
    }

    fn kill_me(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        if !(get_kill_deadline(&e) > e.ledger().timestamp()) {
            panic!("deadline has passed")
        }
        put_is_killed(&e, &true);
    }

    fn unkill_me(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        put_is_killed(&e, &false);
    }
}

#[contractimpl]
impl LiquidityPoolInterfaceTrait for LiquidityPool {
    fn pool_type(e: Env) -> Symbol {
        match N_COINS {
            2 => Symbol::new(&e, "stable"),
            3 => Symbol::new(&e, "stable_3"),
            4 => Symbol::new(&e, "stable_4"),
            _ => panic!("unable to calculate pool type"),
        }
    }

    fn initialize(
        e: Env,
        admin: Address,
        token_wasm_hash: BytesN<32>,
        coins: Vec<Address>,
        a: u128,
        fee: u128,
        admin_fee: u128,
    ) -> bool {
        let access_control = AccessControl::new(&e);
        if access_control.has_admin() {
            panic!("already initialized")
        }

        access_control.set_admin(&admin);
        // do we need admin fee?
        put_admin_fee(&e, &admin_fee);

        // todo: assert non zero addresses
        put_tokens(&e, &coins);

        // LP token
        // let share_contract = create_contract(&e, token_wasm_hash, &token_a, &token_b);
        let share_contract = create_contract(&e, token_wasm_hash);
        LPToken::new(&e, &share_contract).initialize(
            &e.current_contract_address(),
            &7u32,
            &"Pool Share Token".into_val(&e),
            &"POOL".into_val(&e),
        );
        put_token_share(&e, share_contract.try_into().unwrap());
        let initial_reserves = Vec::from_array(&e, [0_u128; N_COINS]);
        put_reserves(&e, &initial_reserves);

        // pool config
        put_initial_a(&e, &a);
        put_initial_a_time(&e, &e.ledger().timestamp()); // todo: is it correct value?
        put_future_a(&e, &a);
        put_future_a_time(&e, &e.ledger().timestamp()); // todo: is it correct value?
        put_fee(&e, &fee);
        put_kill_deadline(&e, &(e.ledger().timestamp() + KILL_DEADLINE_DT));
        put_admin_actions_deadline(&e, &0);
        put_transfer_ownership_deadline(&e, &0);
        put_is_killed(&e, &false);

        let rewards = get_rewards_manager(&e);
        rewards.manager().initialize();

        true
    }

    fn get_fee_fraction(e: Env) -> u32 {
        get_fee(&e) as u32
    }

    fn get_admin_fee(e: Env) -> u32 {
        get_admin_fee(&e) as u32
    }

    fn share_id(e: Env) -> Address {
        get_token_share(&e)
    }

    fn get_reserves(e: Env) -> Vec<u128> {
        get_reserves(&e)
    }

    fn get_tokens(e: Env) -> Vec<Address> {
        get_tokens(&e)
    }

    fn deposit(
        e: Env,
        user: Address,
        amounts: Vec<u128>,
        // min_mint_amount: u128
    ) -> (Vec<u128>, u128) {
        user.require_auth();
        if get_is_killed(&e) {
            panic!("is killed")
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

        let mut fees: Vec<u128> = Vec::from_array(&e, [0; N_COINS]);
        let fee = get_fee(&e) * N_COINS as u128 / (4 * (N_COINS as u128 - 1));
        let admin_fee = get_admin_fee(&e);
        let amp = Self::a(e.clone());

        let token_supply = get_total_shares(&e) as u128;
        // Initial invariant
        let mut d0 = 0;
        let old_balances = get_reserves(&e);
        if token_supply > 0 {
            d0 = Self::get_d_mem(e.clone(), old_balances.clone(), amp);
        }
        let mut new_balances: Vec<u128> = old_balances.clone();
        let coins = get_tokens(&e);

        for i in 0..N_COINS as u32 {
            let in_amount = amounts.get(i).unwrap();
            if token_supply == 0 {
                if in_amount <= 0 {
                    panic!("initial deposit requires all coins");
                }
            }
            let in_coin = coins.get(i).unwrap();

            // Take coins from the sender
            if in_amount > 0 {
                let token_client = Client::new(&e, &in_coin);
                token_client.transfer_from(
                    &e.current_contract_address(),
                    &user,
                    &e.current_contract_address(),
                    &(amounts.get(i).unwrap() as i128),
                );
            }

            new_balances.set(i, old_balances.get(i).unwrap() + in_amount);
        }

        // Invariant after change
        let d1 = Self::get_d_mem(e.clone(), new_balances.clone(), amp);
        if d1 <= d0 {
            panic!("D1 not greater than D0");
        }

        // We need to recalculate the invariant accounting for fees
        // to calculate fair user's share
        let mut d2 = d1;
        let balances = if token_supply > 0 {
            let mut result = new_balances.clone();
            // Only account for fees if we are not the first to deposit
            for i in 0..N_COINS as u32 {
                let ideal_balance = d1 * old_balances.get(i).unwrap() / d0;
                let difference = if ideal_balance > new_balances.get(i).unwrap() {
                    ideal_balance - new_balances.get(i).unwrap()
                } else {
                    new_balances.get(i).unwrap() - ideal_balance
                };
                fees.set(i, fee * difference / FEE_DENOMINATOR);

                result.set(
                    i,
                    new_balances.get(i).unwrap()
                        - (fees.get(i).unwrap() * admin_fee / FEE_DENOMINATOR),
                );
                new_balances.set(i, new_balances.get(i).unwrap() - fees.get(i).unwrap());
            }
            d2 = Self::get_d_mem(e.clone(), new_balances, amp);
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

        // todo: return back after interface unify
        // if mint_amount < min_mint_amount {
        //     panic!("Slippage screwed you");
        // }

        // Mint pool tokens
        mint_shares(&e, user, mint_amount as i128);

        (amounts, mint_amount)
    }

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
            panic!("is killed")
        }
        let rates = RATES;

        let old_balances = get_reserves(&e);
        let xp = Self::xp_mem(e.clone(), old_balances.clone());

        // Handling an unexpected charge of a fee on transfer (USDT, PAXG)
        let dx_w_fee = in_amount;
        let coins = get_tokens(&e);
        let input_coin = coins.get(in_idx as u32).unwrap();

        let token_client = Client::new(&e, &input_coin);
        token_client.transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &(in_amount as i128),
        );

        let x = xp.get(in_idx as u32).unwrap() + dx_w_fee * rates[in_idx as usize] / PRECISION;
        let y = Self::get_y(e.clone(), in_idx, out_idx, x, xp.clone());

        let dy = xp.get(out_idx as u32).unwrap() - y - 1; // -1 just in case there were some rounding errors
        let dy_fee = dy * get_fee(&e) / FEE_DENOMINATOR;

        // Convert all to real units
        let dy = (dy - dy_fee) * PRECISION / rates[out_idx as usize];
        if !(dy >= out_min) {
            panic!("Exchange resulted in fewer coins than expected")
        }

        let mut dy_admin_fee = dy_fee * get_admin_fee(&e) / FEE_DENOMINATOR;
        dy_admin_fee = dy_admin_fee * PRECISION / rates[out_idx as usize];

        // Change balances exactly in same way as we change actual ERC20 coin amounts
        let mut reserves = get_reserves(&e);
        reserves.set(
            in_idx as u32,
            old_balances.get(in_idx as u32).unwrap() + dx_w_fee,
        );
        // When rounding errors happen, we undercharge admin fee in favor of LP
        reserves.set(
            out_idx as u32,
            old_balances.get(out_idx as u32).unwrap() - dy - dy_admin_fee,
        );
        put_reserves(&e, &reserves);

        let token_client = Client::new(&e, &coins.get(out_idx as u32).unwrap());
        token_client.transfer(&e.current_contract_address(), &user, &(dy as i128));
        dy
    }

    fn estimate_swap(e: Env, in_idx: u32, out_idx: u32, in_amount: u128) -> u128 {
        Self::get_dy(e, in_idx, out_idx, in_amount)
    }

    fn withdraw(e: Env, user: Address, share_amount: u128, min_amounts: Vec<u128>) -> Vec<u128> {
        user.require_auth();

        if min_amounts.len() != N_COINS as u32 {
            panic!("wrong min_amounts vector size")
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

        let total_supply = get_total_shares(&e) as u128;
        let mut amounts = Vec::from_array(&e, [0; N_COINS]);
        let mut reserves = get_reserves(&e);
        let coins = get_tokens(&e);

        for i in 0..N_COINS as u32 {
            let value = reserves.get(i).unwrap() * share_amount / total_supply;
            if !(value >= min_amounts.get(i).unwrap()) {
                panic!("Withdrawal resulted in fewer coins than expected")
            }
            reserves.set(i, reserves.get(i).unwrap() - value);
            amounts.set(i, value);

            let token_client = Client::new(&e, &coins.get(i).unwrap());
            token_client.transfer(&e.current_contract_address(), &user, &(value as i128));
        }
        put_reserves(&e, &reserves);

        // First transfer the pool shares that need to be redeemed
        let share_token_client = LPToken::new(&e, &get_token_share(&e));
        share_token_client.transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &(share_amount as i128),
        );
        burn_shares(&e, share_amount as i128);
        amounts
    }
}

#[contractimpl]
impl UpgradeableContractTrait for LiquidityPool {
    fn version() -> u32 {
        1
    }

    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) -> bool {
        let access_control = AccessControl::new(&e);
        access_control.require_admin();
        e.deployer().update_current_contract_wasm(new_wasm_hash);
        true
    }
}

#[contractimpl]
impl RewardsTrait for LiquidityPool {
    fn initialize_rewards_config(
        e: Env,
        // admin: Address,
        reward_token: Address,
        reward_storage: Address,
    ) -> bool {
        // admin.require_auth();
        // check_admin(&e, &admin);
        let rewards = get_rewards_manager(&e);
        if rewards.storage().has_reward_token() {
            panic!("rewards config already initialized")
        }
        rewards.storage().put_reward_token(reward_token);
        rewards.storage().put_reward_storage(reward_storage);

        true
    }

    fn set_rewards_config(
        e: Env,
        admin: Address,
        expired_at: u64, // timestamp
        tps: u128,       // value with 7 decimal places. example: 600_0000000
    ) -> bool {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.check_admin(&admin);

        if expired_at < e.ledger().timestamp() {
            panic!("cannot set expiration time to the past");
        }

        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        rewards.manager().update_rewards_data(total_shares);

        let config = PoolRewardConfig { tps, expired_at };
        bump_instance(&e);
        rewards.storage().set_pool_reward_config(&config);
        true
    }

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

    fn get_user_reward(e: Env, user: Address) -> u128 {
        let rewards = get_rewards_manager(&e);
        let total_shares = get_total_shares(&e);
        let user_shares = get_user_balance_shares(&e, &user);
        rewards
            .manager()
            .get_amount_to_claim(&user, total_shares, user_shares)
    }

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
