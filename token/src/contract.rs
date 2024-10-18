//! Implementation of the Soroban token interface.
use crate::allowance::{read_allowance, spend_allowance, write_allowance};
use crate::balance::{read_balance, receive_balance, spend_balance};
use crate::errors::TokenError;
use crate::interface::UpgradeableContract;
use crate::metadata::{read_decimal, read_name, read_symbol, write_metadata};
use crate::pool::checkpoint_user_rewards;
use access_control::access::{AccessControl, AccessControlTrait, Role, TransferOwnershipTrait};
use access_control::interface::TransferableContract;
use soroban_sdk::token::{self, Interface as _};
use soroban_sdk::{contract, contractimpl, panic_with_error, Address, BytesN, Env, String};
use soroban_token_sdk::metadata::TokenMetadata;
use soroban_token_sdk::TokenUtils;
use utils::bump::bump_instance;

fn check_nonnegative_amount(e: &Env, amount: i128) {
    if amount < 0 {
        panic_with_error!(&e, TokenError::NegativeNotAllowed);
    }
}

#[contract]
pub struct Token;

#[contractimpl]
impl Token {
    pub fn initialize(e: Env, admin: Address, decimal: u32, name: String, symbol: String) {
        let access_control = AccessControl::new(&e);
        if access_control.get_role_safe(Role::Admin).is_some() {
            panic_with_error!(&e, TokenError::AlreadyInitialized);
        }
        access_control.set_role_address(Role::Admin, &admin);
        if decimal > u8::MAX.into() {
            panic_with_error!(&e, TokenError::DecimalTooLarge);
        }

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
        check_nonnegative_amount(&e, amount);
        let admin = AccessControl::new(&e).get_role(Role::Admin);
        admin.require_auth();

        bump_instance(&e);

        receive_balance(&e, to.clone(), amount);
        TokenUtils::new(&e).events().mint(admin, to, amount);
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

        check_nonnegative_amount(&e, amount);

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

        check_nonnegative_amount(&e, amount);

        bump_instance(&e);

        checkpoint_user_rewards(&e, from.clone());
        checkpoint_user_rewards(&e, to.clone());

        spend_balance(&e, from.clone(), amount);
        receive_balance(&e, to.clone(), amount);
        TokenUtils::new(&e).events().transfer(from, to, amount);
    }

    fn transfer_from(e: Env, spender: Address, from: Address, to: Address, amount: i128) {
        spender.require_auth();

        check_nonnegative_amount(&e, amount);

        bump_instance(&e);

        checkpoint_user_rewards(&e, from.clone());
        checkpoint_user_rewards(&e, to.clone());

        spend_allowance(&e, from.clone(), spender, amount);
        spend_balance(&e, from.clone(), amount);
        receive_balance(&e, to.clone(), amount);
        TokenUtils::new(&e).events().transfer(from, to, amount)
    }

    fn burn(e: Env, from: Address, amount: i128) {
        from.require_auth();

        check_nonnegative_amount(&e, amount);

        bump_instance(&e);

        spend_balance(&e, from.clone(), amount);
        TokenUtils::new(&e).events().burn(from, amount);
    }

    fn burn_from(e: Env, spender: Address, from: Address, amount: i128) {
        spender.require_auth();

        check_nonnegative_amount(&e, amount);

        bump_instance(&e);

        spend_allowance(&e, from.clone(), spender, amount);
        spend_balance(&e, from.clone(), amount);
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

// The `UpgradeableContract` trait provides the interface for upgrading the contract.
#[contractimpl]
impl UpgradeableContract for Token {
    // Returns the version of the contract.
    //
    // # Returns
    //
    // The version of the contract as a u32.
    fn version() -> u32 {
        130
    }

    // Upgrades the contract to a new version.
    //
    // # Arguments
    //
    // * `e` - The environment.
    // * `new_wasm_hash` - The hash of the new contract version.
    fn upgrade(e: Env, admin: Address, new_wasm_hash: BytesN<32>) {
        admin.require_auth();
        AccessControl::new(&e).assert_address_has_role(&admin, Role::Admin);
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}

// The `TransferableContract` trait provides the interface for transferring ownership of the contract.
#[contractimpl]
impl TransferableContract for Token {
    // Commits an ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    // * `new_admin` - The address of the new admin.
    fn commit_transfer_ownership(e: Env, admin: Address, new_admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, Role::Admin);
        access_control.commit_transfer_ownership(new_admin);
    }

    // Applies the committed ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn apply_transfer_ownership(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, Role::Admin);
        access_control.apply_transfer_ownership();
    }

    // Reverts the committed ownership transfer.
    //
    // # Arguments
    //
    // * `admin` - The address of the admin.
    fn revert_transfer_ownership(e: Env, admin: Address) {
        admin.require_auth();
        let access_control = AccessControl::new(&e);
        access_control.assert_address_has_role(&admin, Role::Admin);
        access_control.revert_transfer_ownership();
    }
}
