//! Mock rebasing token for regression testing.
//!
//! Simulates a time-rebasing token where:
//! - Internal storage uses "shares"
//! - `transfer(amount)` converts to shares via `ceil(amount * K / K_SCALE)`
//! - `balance()` converts back via `floor(shares * K_SCALE / K)`
//!
//! With K > K_SCALE (e.g. K=103, K_SCALE=100), the ceil-rounding on transfer
//! means splitting a single transfer into two parts can lose a share:
//!   `ceil(A*r) - ceil(B*r)` can be `< ceil((A-B)*r)`
#![cfg(any(test, feature = "testutils"))]

use soroban_sdk::{contract, contractimpl, contracttype, Address, Env, String};

#[contracttype]
enum Key {
    Admin,
    Shares(Address),
    K,
    KScale,
}

#[contract]
pub struct RebasingToken;

fn get_shares(env: &Env, id: &Address) -> i128 {
    env.storage()
        .persistent()
        .get(&Key::Shares(id.clone()))
        .unwrap_or(0)
}

fn set_shares(env: &Env, id: &Address, shares: i128) {
    env.storage()
        .persistent()
        .set(&Key::Shares(id.clone()), &shares);
}

fn amount_to_shares(env: &Env, amount: i128) -> i128 {
    let k: i128 = env.storage().instance().get(&Key::K).unwrap();
    let k_scale: i128 = env.storage().instance().get(&Key::KScale).unwrap();
    (amount * k + k_scale - 1) / k_scale // ceil
}

fn shares_to_amount(env: &Env, shares: i128) -> i128 {
    let k: i128 = env.storage().instance().get(&Key::K).unwrap();
    let k_scale: i128 = env.storage().instance().get(&Key::KScale).unwrap();
    shares * k_scale / k // floor
}

fn do_transfer(env: &Env, from: &Address, to: &Address, amount: i128) {
    let shares = amount_to_shares(env, amount);
    let from_shares = get_shares(env, from);
    if from_shares < shares {
        panic!("InsufficientBalance");
    }
    set_shares(env, from, from_shares - shares);
    set_shares(env, to, get_shares(env, to) + shares);
}

#[contractimpl]
impl RebasingToken {
    pub fn initialize(env: Env, admin: Address, k: i128, k_scale: i128) {
        env.storage().instance().set(&Key::Admin, &admin);
        env.storage().instance().set(&Key::K, &k);
        env.storage().instance().set(&Key::KScale, &k_scale);
    }

    pub fn mint(env: Env, to: Address, amount: i128) {
        let admin: Address = env.storage().instance().get(&Key::Admin).unwrap();
        admin.require_auth();
        let shares = amount_to_shares(&env, amount);
        set_shares(&env, &to, get_shares(&env, &to) + shares);
    }

    pub fn balance(env: Env, id: Address) -> i128 {
        shares_to_amount(&env, get_shares(&env, &id))
    }

    pub fn transfer(env: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();
        do_transfer(&env, &from, &to, amount);
    }

    pub fn transfer_from(env: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();
        do_transfer(&env, &from, &to, amount);
    }

    pub fn approve(_env: Env, from: Address, _spender: Address, _amount: i128, _exp: u32) {
        from.require_auth();
    }

    pub fn allowance(_env: Env, _from: Address, _spender: Address) -> i128 {
        0
    }

    pub fn burn(env: Env, from: Address, amount: i128) {
        from.require_auth();
        let shares = amount_to_shares(&env, amount);
        set_shares(&env, &from, get_shares(&env, &from) - shares);
    }

    pub fn burn_from(env: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();
        let shares = amount_to_shares(&env, amount);
        set_shares(&env, &from, get_shares(&env, &from) - shares);
    }

    pub fn decimals(_env: Env) -> u32 {
        7
    }
    pub fn name(env: Env) -> String {
        String::from_str(&env, "RebasingToken")
    }
    pub fn symbol(env: Env) -> String {
        String::from_str(&env, "RBT")
    }
}
