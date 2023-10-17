use crate::admin::{has_admin, set_admin};
use crate::pool_constants::{
    FEE_DENOMINATOR, KILL_DEADLINE_DT, LENDING_PRECISION, N_COINS, PRECISION, PRECISION_MUL, RATES,
};
use crate::token::create_contract;
use crate::{storage, token};
use soroban_sdk::{contract, contractimpl, contractmeta, Address, BytesN, Env, IntoVal, Vec};

contractmeta!(
    key = "Description",
    val = "Stable Swap AMM for two pairs of tokens"
);

#[contract]
pub struct LiquidityPool;

pub trait LiquidityPoolTrait {
    // Sets the token contract addresses for this pool
    fn initialize(
        e: Env,
        admin: Address,
        token_wasm_hash: BytesN<32>,
        coins: Vec<Address>,
        a: u128,
        fee: u128,
        admin_fee: u128,
        reward_token: Address,
        reward_storage: Address,
    );
    // fn initialize_fee_fraction(e: Env, fee_fraction: u32);

    // Returns the token contract address for the pool share token
    fn share_id(e: Env) -> Address;
    fn a(e: Env) -> u128;
    fn xp(e: Env) -> Vec<u128>;
    fn xp_mem(e: Env, balances: Vec<u128>) -> Vec<u128>;
    fn get_d(e: Env, xp: Vec<u128>, amp: u128) -> u128;
    fn get_d_mem(e: Env, balances: Vec<u128>, amp: u128) -> u128;
    fn get_virtual_price(e: Env) -> u128;
    fn calc_token_amount(e: Env, amounts: Vec<u128>, deposit: bool) -> u128;
    fn add_liquidity(e: Env, user: Address, amounts: Vec<u128>, min_mint_amount: u128);
    fn get_y(e: Env, i: i128, j: i128, x: u128, xp_: Vec<u128>) -> u128;
    fn get_dy(e: Env, i: i128, j: i128, dx: u128) -> u128;
    fn get_dy_underlying(e: Env, i: i128, j: i128, dx: u128) -> u128;
    fn exchange(e: Env, user: Address, i: i128, j: i128, dx: u128, min_dy: u128);
    fn remove_liquidity(e: Env, user: Address, share_amount: u128, min_amounts: Vec<u128>);

    // Deposits token_a and token_b. Also mints pool shares for the "to" Identifier. The amount minted
    // is determined based on the difference between the reserves stored by this contract, and
    // the actual balance of token_a and token_b for this contract.
    // fn deposit(
    //     e: Env,
    //     to: Address,
    //     desired_a: i128,
    //     min_a: i128,
    //     desired_b: i128,
    //     min_b: i128,
    // ) -> (i128, i128);

    // If "buy_a" is true, the swap will buy token_a and sell token_b. This is flipped if "buy_a" is false.
    // "out" is the amount being bought, with in_max being a safety to make sure you receive at least that amount.
    // swap will transfer the selling token "to" to this contract, and then the contract will transfer the buying token to "to".
    // fn swap(e: Env, to: Address, buy_a: bool, out: i128, in_max: i128) -> i128;
    // fn estimate_swap_out(e: Env, buy_a: bool, out: i128) -> i128;

    // transfers share_amount of pool share tokens to this contract, burns all pools share tokens in this contracts, and sends the
    // corresponding amount of token_a and token_b to "to".
    // Returns amount of both tokens withdrawn
    // fn withdraw(e: Env, to: Address, share_amount: i128, min_a: i128, min_b: i128) -> (i128, i128);

    // fn get_rsrvs(e: Env) -> Vec<i128>;

    fn version() -> u32;
    // fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
    // fn set_rewards_config(e: Env, admin: Address, expired_at: u64, amount: i128);
    // fn get_rewards_info(e: Env, user: Address) -> Map<Symbol, i128>;
    // fn get_user_reward(e: Env, user: Address) -> i128;
    // fn claim(e: Env, user: Address) -> i128;
    // fn get_fee_fraction(e: Env) -> u32;
    // xp size = N_COINS
}

#[contractimpl]
impl LiquidityPoolTrait for LiquidityPool {
    fn initialize(
        e: Env,
        admin: Address,
        token_wasm_hash: BytesN<32>,
        coins: Vec<Address>,
        a: u128,
        fee: u128,
        admin_fee: u128,
        _reward_token: Address,
        _reward_storage: Address,
    ) {
        if has_admin(&e) {
            panic!("already initialized")
        }

        set_admin(&e, &admin);
        // do we need admin fee?
        storage::put_admin_fee(&e, admin_fee);

        // todo: assert non zero addresses
        storage::put_tokens(&e, &coins);

        // LP token
        // let share_contract = create_contract(&e, token_wasm_hash, &token_a, &token_b);
        let share_contract = create_contract(&e, token_wasm_hash);
        token::Client::new(&e, &share_contract).initialize(
            &e.current_contract_address(),
            &7u32,
            &"Pool Share Token".into_val(&e),
            &"POOL".into_val(&e),
        );
        storage::put_token_share(&e, share_contract.try_into().unwrap());
        let initial_reserves = Vec::from_array(&e, [0_u128; N_COINS]);
        storage::put_reserves(&e, &initial_reserves);

        // pool config
        storage::put_initial_a(&e, a);
        storage::put_initial_a_time(&e, e.ledger().timestamp()); // todo: is it correct value?
        storage::put_future_a(&e, a);
        storage::put_future_a_time(&e, e.ledger().timestamp()); // todo: is it correct value?
        storage::put_fee(&e, fee);
        storage::put_kill_deadline(&e, e.ledger().timestamp() + KILL_DEADLINE_DT);
        // todo: do we need kill deadline?

        // rewards_manager::set_reward_inv(&e, &Map::from_array(&e, [(0_u64, 0_u64)]));
        // rewards_storage::set_pool_reward_config(
        //     &e,
        //     &rewards_storage::PoolRewardConfig {
        //         tps: 0,
        //         expired_at: 0,
        //     },
        // );
        // rewards_storage::set_pool_reward_data(
        //     &e,
        //     &rewards_storage::PoolRewardData {
        //         block: 0,
        //         accumulated: 0,
        //         last_time: 0,
        //     },
        // );
    }

    // fn initialize_fee_fraction(e: Env, fee_fraction: u32) {
    //     // 0.01% = 1; 1% = 100; 0.3% = 30
    //     if fee_fraction > 9999 {
    //         panic!("fee cannot be equal or greater than 100%");
    //     }
    //     storage::put_fee_fraction(&e, fee_fraction);
    // }

    fn share_id(e: Env) -> Address {
        storage::get_token_share(&e)
    }

    fn a(e: Env) -> u128 {
        // Handle ramping A up or down
        let t1 = storage::get_future_a_time(&e) as u128;
        let a1 = storage::get_future_a(&e);
        let now = e.ledger().timestamp() as u128;

        if now < t1 {
            let a0 = storage::get_initial_a(&e);
            let t0 = storage::get_initial_a_time(&e) as u128;
            // Expressions in u128 cannot have negative numbers, thus "if"
            if a1 > a0 {
                return a0 + (a1 - a0) * (now - t0) / (t1 - t0);
            } else {
                return a0 - (a0 - a1) * (now - t0) / (t1 - t0);
            }
        } else {
            // when t1 == 0 or block.timestamp >= t1
            return a1;
        }
    }

    fn xp(e: Env) -> Vec<u128> {
        let reserves = storage::get_reserves(&e);
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

    fn get_virtual_price(e: Env) -> u128 {
        // Returns portfolio virtual price (for calculating profit) scaled up by 1e7

        let d = Self::get_d(e.clone(), Self::xp(e.clone()), Self::a(e.clone()));
        // D is in the units similar to DAI (e.g. converted to precision 1e7)
        // When balanced, D = n * x_u - total virtual value of the portfolio
        // let token_supply = self.token.totalSupply()
        let token_supply = token::get_total_shares(&e);
        return d * PRECISION / token_supply as u128;
    }

    fn calc_token_amount(e: Env, amounts: Vec<u128>, deposit: bool) -> u128 {
        // Simplified method to calculate addition or reduction in token supply at
        // deposit or withdrawal without taking fees into account (but looking at
        // slippage).
        // Needed to prevent front-running, not for precise calculations!

        let mut balances = storage::get_reserves(&e);
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
        let token_amount = token::get_total_shares(&e);
        let diff = if deposit { d1 - d0 } else { d0 - d1 };
        return diff * token_amount as u128 / d0;
    }

    fn add_liquidity(e: Env, user: Address, amounts: Vec<u128>, min_mint_amount: u128) {
        user.require_auth();
        // assert not self.is_killed  // dev: is killed

        let mut fees: Vec<u128> = Vec::new(&e);
        let fee = storage::get_fee(&e) * N_COINS as u128 / (4 * (N_COINS as u128 - 1));
        let admin_fee = storage::get_admin_fee(&e);
        let amp = Self::a(e.clone());

        let token_supply = token::get_total_shares(&e) as u128;
        // Initial invariant
        let mut d0 = 0;
        let old_balances = storage::get_reserves(&e);
        if token_supply > 0 {
            d0 = Self::get_d_mem(e.clone(), old_balances.clone(), amp);
        }
        let mut new_balances: Vec<u128> = old_balances.clone();
        let coins = storage::get_tokens(&e);

        for i in 0..N_COINS as u32 {
            let in_amount = amounts.get(i).unwrap();
            if token_supply == 0 {
                // assert in_amount > 0  // dev: initial deposit requires all coins
            }
            let in_coin = coins.get(i).unwrap();

            // Take coins from the sender
            if in_amount > 0 {
                let token_client = token::Client::new(&e, &in_coin);
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
        storage::put_reserves(&e, &balances);

        // Calculate, how much pool tokens to mint
        let mint_amount = if token_supply == 0 {
            d1 // Take the dust if there was any
        } else {
            token_supply * (d2 - d0) / d0
        };

        if mint_amount < min_mint_amount {
            panic!("Slippage screwed you");
        }

        // Mint pool tokens
        token::mint_shares(&e, user, mint_amount as i128);
    }

    fn get_y(e: Env, i: i128, j: i128, x: u128, xp_: Vec<u128>) -> u128 {
        // x in the input is converted to the same price/precision

        if !(i != j) {
            panic!("same coin")
        } // dev: same coin
        if !(j >= 0) {
            panic!("j below zero")
        } // dev: j below zero
        if !(j < N_COINS as i128) {
            panic!("j above N_COINS")
        } // dev: j above N_COINS

        // should be unreachable, but good for safety
        if !(i >= 0) {
            panic!("bad arguments")
        }
        if !(i < N_COINS as i128) {
            panic!("bad arguments")
        }

        let amp = Self::a(e.clone());
        let d = Self::get_d(e.clone(), xp_.clone(), amp);
        let mut c = d;
        let mut s = 0;
        let ann = amp * N_COINS as u128;

        let mut _x = 0;
        for _i in 0..N_COINS as i128 {
            if _i == i {
                _x = x;
            } else if _i != j {
                _x = xp_.get(_i as u32).unwrap();
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

    fn get_dy(e: Env, i: i128, j: i128, dx: u128) -> u128 {
        // dx and dy in c-units
        let rates = RATES;
        let xp = Self::xp(e.clone());

        let x = xp.get(i as u32).unwrap() + (dx * rates[i as usize] / PRECISION);
        let y = Self::get_y(e.clone(), i, j, x, xp.clone());
        let dy = (xp.get(j as u32).unwrap() - y - 1) * PRECISION / rates[j as usize];
        let _fee = storage::get_fee(&e) * dy / FEE_DENOMINATOR;
        return dy - _fee;
    }

    fn get_dy_underlying(e: Env, i: i128, j: i128, dx: u128) -> u128 {
        // dx and dy in underlying units
        let xp = Self::xp(e.clone());
        let precisions = PRECISION_MUL;

        let x = xp.get(i as u32).unwrap() + dx * precisions[i as usize];
        let y = Self::get_y(e.clone(), i, j, x, xp.clone());
        let dy = (xp.get(j as u32).unwrap() - y - 1) / precisions[j as usize];
        let fee = storage::get_fee(&e) * dy / FEE_DENOMINATOR;
        return dy - fee;
    }

    fn exchange(e: Env, user: Address, i: i128, j: i128, dx: u128, min_dy: u128) {
        user.require_auth();
        // assert not self.is_killed  // dev: is killed
        let rates = RATES;

        let old_balances = storage::get_reserves(&e);
        let xp = Self::xp_mem(e.clone(), old_balances.clone());

        // Handling an unexpected charge of a fee on transfer (USDT, PAXG)
        let dx_w_fee = dx;
        let coins = storage::get_tokens(&e);
        let input_coin = coins.get(i as u32).unwrap();

        let token_client = token::Client::new(&e, &input_coin);
        token_client.transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &(dx as i128),
        );

        let x = xp.get(i as u32).unwrap() + dx_w_fee * rates[i as usize] / PRECISION;
        let y = Self::get_y(e.clone(), i, j, x, xp.clone());

        let dy = xp.get(j as u32).unwrap() - y - 1; // -1 just in case there were some rounding errors
        let dy_fee = dy * storage::get_fee(&e) / FEE_DENOMINATOR;

        // Convert all to real units
        let dy = (dy - dy_fee) * PRECISION / rates[j as usize];
        if !(dy >= min_dy) {
            panic!("Exchange resulted in fewer coins than expected")
        }

        let mut dy_admin_fee = dy_fee * storage::get_admin_fee(&e) / FEE_DENOMINATOR;
        dy_admin_fee = dy_admin_fee * PRECISION / rates[j as usize];

        // Change balances exactly in same way as we change actual ERC20 coin amounts
        let mut reserves = storage::get_reserves(&e);
        reserves.set(i as u32, old_balances.get(i as u32).unwrap() + dx_w_fee);
        // When rounding errors happen, we undercharge admin fee in favor of LP
        reserves.set(
            j as u32,
            old_balances.get(j as u32).unwrap() - dy - dy_admin_fee,
        );
        storage::put_reserves(&e, &reserves);

        let token_client = token::Client::new(&e, &coins.get(j as u32).unwrap());
        token_client.transfer(&e.current_contract_address(), &user, &(dy as i128));
    }

    fn remove_liquidity(e: Env, user: Address, share_amount: u128, min_amounts: Vec<u128>) {
        user.require_auth();
        let total_supply = token::get_total_shares(&e) as u128;
        let mut amounts = Vec::from_array(&e, [0; N_COINS]);
        let mut reserves = storage::get_reserves(&e);
        let coins = storage::get_tokens(&e);

        for i in 0..N_COINS as u32 {
            let value = reserves.get(i).unwrap() * share_amount / total_supply;
            if !(value >= min_amounts.get(i).unwrap()) {
                panic!("Withdrawal resulted in fewer coins than expected")
            }
            reserves.set(i, reserves.get(i).unwrap() - value);
            amounts.set(i, value);

            let token_client = token::Client::new(&e, &coins.get(i).unwrap());
            token_client.transfer(&e.current_contract_address(), &user, &(value as i128));
        }

        // First transfer the pool shares that need to be redeemed
        let share_token_client = token::Client::new(&e, &storage::get_token_share(&e));
        share_token_client.transfer_from(
            &e.current_contract_address(),
            &user,
            &e.current_contract_address(),
            &(share_amount as i128),
        );
        token::burn_shares(&e, share_amount as i128);
    }

    // fn deposit(
    //     e: Env,
    //     to: Address,
    //     desired_a: i128,
    //     min_a: i128,
    //     desired_b: i128,
    //     min_b: i128,
    // ) -> (i128, i128) {
    //     // Depositor needs to authorize the deposit
    //     to.require_auth();
    //
    //     let (reserve_a, reserve_b) = (storage::get_reserve_a(&e), storage::get_reserve_b(&e));
    //
    //     // Before actual changes were made to the pool, update total rewards data and refresh/initialize user reward
    //     let pool_data = rewards_manager::update_rewards_data(&e);
    //     rewards_manager::update_user_reward(&e, &pool_data, &to);
    //     rewards_storage::bump_user_reward_data(&e, &to);
    //
    //     // Calculate deposit amounts
    //     let amounts =
    //         pool::get_deposit_amounts(desired_a, min_a, desired_b, min_b, reserve_a, reserve_b);
    //
    //     let token_a_client = token::Client::new(&e, &storage::get_token_a(&e));
    //     let token_b_client = token::Client::new(&e, &storage::get_token_b(&e));
    //
    //     token_a_client.transfer_from(
    //         &e.current_contract_address(),
    //         &to,
    //         &e.current_contract_address(),
    //         &amounts.0,
    //     );
    //     token_b_client.transfer_from(
    //         &e.current_contract_address(),
    //         &to,
    //         &e.current_contract_address(),
    //         &amounts.1,
    //     );
    //
    //     // Now calculate how many new pool shares to mint
    //     let (balance_a, balance_b) = (token::get_balance_a(&e), token::get_balance_b(&e));
    //     let total_shares = token::get_total_shares(&e);
    //
    //     let zero = 0;
    //     let new_total_shares = if reserve_a > zero && reserve_b > zero {
    //         let shares_a = (balance_a * total_shares) / reserve_a;
    //         let shares_b = (balance_b * total_shares) / reserve_b;
    //         shares_a.min(shares_b)
    //     } else {
    //         (balance_a * balance_b).sqrt()
    //     };
    //
    //     token::mint_shares(&e, to, new_total_shares - total_shares);
    //     storage::put_reserve_a(&e, balance_a);
    //     storage::put_reserve_b(&e, balance_b);
    //     (amounts.0, amounts.1)
    // }
    //
    // fn swap(e: Env, to: Address, buy_a: bool, out: i128, in_max: i128) -> i128 {
    //     to.require_auth();
    //
    //     let (reserve_a, reserve_b) = (storage::get_reserve_a(&e), storage::get_reserve_b(&e));
    //     let (reserve_sell, reserve_buy) = if buy_a {
    //         (reserve_b, reserve_a)
    //     } else {
    //         (reserve_a, reserve_b)
    //     };
    //
    //     let fee_fraction = storage::get_fee_fraction(&e);
    //
    //     // First calculate how much needs to be sold to buy amount out from the pool
    //     let n = reserve_sell * out * 10000;
    //     let d = (reserve_buy - out) * (10000 - fee_fraction as i128);
    //     let sell_amount = (n / d) + 1;
    //     if sell_amount > in_max {
    //         panic!("in amount is over max")
    //     }
    //
    //     // Transfer the amount being sold to the contract
    //     let sell_token = if buy_a {
    //         storage::get_token_b(&e)
    //     } else {
    //         storage::get_token_a(&e)
    //     };
    //     let sell_token_client = token::Client::new(&e, &sell_token);
    //     sell_token_client.transfer_from(
    //         &e.current_contract_address(),
    //         &to,
    //         &e.current_contract_address(),
    //         &sell_amount,
    //     );
    //
    //     let (balance_a, balance_b) = (token::get_balance_a(&e), token::get_balance_b(&e));
    //
    //     // residue_numerator and residue_denominator are the amount that the invariant considers after
    //     // deducting the fee, scaled up by 1000 to avoid fractions
    //     let residue_numerator = 10000 - fee_fraction as i128;
    //     let residue_denominator = 10000;
    //     let zero = 0;
    //
    //     let new_invariant_factor = |balance: i128, reserve: i128, out: i128| {
    //         let delta = balance - reserve - out;
    //         let adj_delta = if delta > zero {
    //             residue_numerator * delta
    //         } else {
    //             residue_denominator * delta
    //         };
    //         residue_denominator * reserve + adj_delta
    //     };
    //
    //     let (out_a, out_b) = if buy_a { (out, 0) } else { (0, out) };
    //
    //     let new_inv_a = new_invariant_factor(balance_a, reserve_a, out_a);
    //     let new_inv_b = new_invariant_factor(balance_b, reserve_b, out_b);
    //     let old_inv_a = residue_denominator * reserve_a;
    //     let old_inv_b = residue_denominator * reserve_b;
    //
    //     if new_inv_a * new_inv_b < old_inv_a * old_inv_b {
    //         panic!("constant product invariant does not hold");
    //     }
    //
    //     if buy_a {
    //         token::transfer_a(&e, to, out_a);
    //     } else {
    //         token::transfer_b(&e, to, out_b);
    //     }
    //
    //     storage::put_reserve_a(&e, balance_a - out_a);
    //     storage::put_reserve_b(&e, balance_b - out_b);
    //     sell_amount
    // }
    //
    // fn estimate_swap_out(e: Env, buy_a: bool, out: i128) -> i128 {
    //     let (reserve_a, reserve_b) = (storage::get_reserve_a(&e), storage::get_reserve_b(&e));
    //     let (reserve_sell, reserve_buy) = if buy_a {
    //         (reserve_b, reserve_a)
    //     } else {
    //         (reserve_a, reserve_b)
    //     };
    //
    //     let fee_fraction = storage::get_fee_fraction(&e);
    //
    //     // Calculate how much needs to be sold to buy amount out from the pool
    //     let n = reserve_sell * out * 10000;
    //     let d = (reserve_buy - out) * (10000 - fee_fraction as i128);
    //     let sell_amount = (n / d) + 1;
    //     sell_amount
    // }
    //
    // fn withdraw(e: Env, to: Address, share_amount: i128, min_a: i128, min_b: i128) -> (i128, i128) {
    //     to.require_auth();
    //
    //     // Before actual changes were made to the pool, update total rewards data and refresh user reward
    //     let pool_data = rewards_manager::update_rewards_data(&e);
    //     rewards_manager::update_user_reward(&e, &pool_data, &to);
    //     rewards_storage::bump_user_reward_data(&e, &to);
    //
    //     // First transfer the pool shares that need to be redeemed
    //     let share_token_client = token::Client::new(&e, &storage::get_token_share(&e));
    //     share_token_client.transfer_from(
    //         &e.current_contract_address(),
    //         &to,
    //         &e.current_contract_address(),
    //         &share_amount,
    //     );
    //
    //     let (balance_a, balance_b) = (token::get_balance_a(&e), token::get_balance_b(&e));
    //     let balance_shares = token::get_balance_shares(&e);
    //
    //     let total_shares = token::get_total_shares(&e);
    //
    //     // Now calculate the withdraw amounts
    //     let out_a = (balance_a * balance_shares) / total_shares;
    //     let out_b = (balance_b * balance_shares) / total_shares;
    //
    //     if out_a < min_a || out_b < min_b {
    //         panic!("min not satisfied");
    //     }
    //
    //     token::burn_shares(&e, balance_shares);
    //     token::transfer_a(&e, to.clone(), out_a);
    //     token::transfer_b(&e, to, out_b);
    //     storage::put_reserve_a(&e, balance_a - out_a);
    //     storage::put_reserve_b(&e, balance_b - out_b);
    //
    //     (out_a, out_b)
    // }
    //
    // fn get_rsrvs(e: Env) -> (i128, i128) {
    //     (storage::get_reserve_a(&e), storage::get_reserve_b(&e))
    // }

    fn version() -> u32 {
        1
    }

    // fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
    //     require_admin(&e);
    //     e.deployer().update_current_contract_wasm(new_wasm_hash);
    // }
    //
    // fn set_rewards_config(
    //     e: Env,
    //     admin: Address,
    //     expired_at: u64, // timestamp
    //     amount: i128,    // value with 7 decimal places. example: 600_0000000
    // ) {
    //     admin.require_auth();
    //     check_admin(&e, &admin);
    //
    //     rewards_manager::update_rewards_data(&e);
    //
    //     let config = rewards_storage::PoolRewardConfig {
    //         tps: amount / to_i128(expired_at - e.ledger().timestamp()),
    //         expired_at,
    //     };
    //     storage::bump_instance(&e);
    //     rewards_storage::set_pool_reward_config(&e, &config);
    // }
    //
    // fn get_rewards_info(e: Env, user: Address) -> Map<Symbol, i128> {
    //     let config = get_pool_reward_config(&e);
    //     let pool_data = rewards_manager::update_rewards_data(&e);
    //     let user_data = rewards_manager::update_user_reward(&e, &pool_data, &user);
    //     let mut result = Map::new(&e);
    //     result.set(symbol_short!("tps"), to_i128(config.tps));
    //     result.set(symbol_short!("exp_at"), to_i128(config.expired_at));
    //     result.set(symbol_short!("acc"), to_i128(pool_data.accumulated));
    //     result.set(symbol_short!("last_time"), to_i128(pool_data.last_time));
    //     result.set(
    //         symbol_short!("pool_acc"),
    //         to_i128(user_data.pool_accumulated),
    //     );
    //     result.set(symbol_short!("block"), to_i128(pool_data.block));
    //     result.set(symbol_short!("usr_block"), to_i128(user_data.last_block));
    //     result.set(symbol_short!("to_claim"), to_i128(user_data.to_claim));
    //     result
    // }
    //
    // fn get_user_reward(e: Env, user: Address) -> i128 {
    //     rewards_manager::get_amount_to_claim(&e, &user)
    // }
    //
    // fn claim(e: Env, user: Address) -> i128 {
    //     let reward = rewards_manager::claim_reward(&e, &user);
    //     rewards_storage::bump_user_reward_data(&e, &user);
    //     reward
    // }
    //
    // fn get_fee_fraction(e: Env) -> u32 {
    //     // returns fee fraction. 0.01% = 1; 1% = 100; 0.3% = 30
    //     storage::get_fee_fraction(&e)
    // }
}
