//! This contract demonstrates a sample implementation of the Soroban token
//! interface.
use crate::allowance::{read_allowance, spend_allowance, write_allowance};
use crate::balance::{
    decrease_total_balance, increase_total_balance, read_balance, read_total_balance,
    receive_balance, spend_balance, write_total_balance,
};
use crate::metadata::{read_decimal, read_name, read_symbol, write_metadata};
use access_control::access::{AccessControl, AccessControlTrait};
use soroban_sdk::token::{self, Interface as _};
use soroban_sdk::{contract, contractimpl, Address, Env, String};
use soroban_token_sdk::metadata::TokenMetadata;
use soroban_token_sdk::TokenUtils;
use utils::bump::bump_instance;

fn check_nonnegative_amount(amount: i128) {
    if amount < 0 {
        panic!("negative amount is not allowed: {}", amount)
    }
}

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String) {
        let access_control = AccessControl::new(&e);
        if access_control.has_admin() {
            panic!("already initialized")
        }
        access_control.set_admin(&admin);
        if decimal > u8::MAX.into() {
            panic!("Decimal must fit in a u8");
        }
        write_total_balance(&e, 0);

        write_metadata(
            &e,
            TokenMetadata {
                decimal,
                name,
                symbol,
            },
        )
    }

    pub fn mint(e: Env, to: Address, amount: i128) {
        check_nonnegative_amount(amount);
        let access_control = AccessControl::new(&e);
        let admin = access_control.get_admin().unwrap();
        admin.require_auth();

        bump_instance(&e);

        receive_balance(&e, to.clone(), amount);
        increase_total_balance(&e, amount);
        TokenUtils::new(&e).events().mint(admin, to, amount);
    }

    pub fn set_admin(e: Env, new_admin: Address) {
        let access_control = AccessControl::new(&e);
        let admin = access_control.get_admin().unwrap();
        admin.require_auth();

        bump_instance(&e);

        access_control.set_admin(&admin);
        TokenUtils::new(&e).events().set_admin(admin, new_admin);
    }

    pub fn total_balance(e: Env) -> i128 {
        read_total_balance(&e)
    }
}

#[contractimpl]
impl token::Interface for Token {
    fn allowance(e: Env, from: Address, spender: Address) -> i128 {
        bump_instance(&e);
        read_allowance(&e, from, spender).amount
    }

    fn approve(e: Env, from: Address, spender: Address, amount: i128, expiration_ledger: u32) {
        from.require_auth();

        check_nonnegative_amount(amount);

        bump_instance(&e);

        write_allowance(&e, from.clone(), spender.clone(), amount, expiration_ledger);
        TokenUtils::new(&e)
            .events()
            .approve(from, spender, amount, expiration_ledger);
    }

    fn balance(e: Env, id: Address) -> i128 {
        bump_instance(&e);
        read_balance(&e, id)
    }

    fn transfer(e: Env, from: Address, to: Address, amount: i128) {
        from.require_auth();

        check_nonnegative_amount(amount);

        bump_instance(&e);

        spend_balance(&e, from.clone(), amount);
        receive_balance(&e, to.clone(), amount);
        TokenUtils::new(&e).events().transfer(from, to, amount);
    }

    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();

        check_nonnegative_amount(amount);

        bump_instance(&e);

        spend_allowance(&e, from.clone(), spender, amount);
        spend_balance(&e, from.clone(), amount);
        receive_balance(&e, to.clone(), amount);
        TokenUtils::new(&e).events().transfer(from, to, amount)
    }

    fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();

        check_nonnegative_amount(amount);

        bump_instance(&e);

        spend_balance(&e, from.clone(), amount);
        decrease_total_balance(&e, amount);
        TokenUtils::new(&e).events().burn(from, amount);
    }

    fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();

        check_nonnegative_amount(amount);

        bump_instance(&e);

        spend_allowance(&e, from.clone(), spender, amount);
        spend_balance(&e, from.clone(), amount);
        decrease_total_balance(&e, amount);
        TokenUtils::new(&e).events().burn(from, amount)
    }

    fn decimals(e: Env) -> u32 {
        read_decimal(&e)
    }

    fn name(e: Env) -> String {
        read_name(&e)
    }

    fn symbol(e: Env) -> String {
        read_symbol(&e)
    }
}
