#![no_std]

use soroban_sdk::xdr::ScValType;
use soroban_sdk::{
    contract, contracterror, contractimpl, panic_with_error, Address, Bytes, Env, String, Symbol,
    TryFromVal, Val, Vec, I256, U256,
};

#[contracterror]
pub enum GuardError {
    ResultsMismatch = 101,
    UnsupportedType = 102,
    InvalidValue = 103,
    TypesMismatch = 104,
}

#[contract]
pub struct ContractGuard;

pub trait GuardInterface {
    fn assert_result(
        e: Env,
        auth_users: Vec<Address>,
        contract: Address,
        fn_name: Symbol,
        args: Vec<Val>,
        expected_result: Val,
    ) -> Val;
}

fn extract<T: TryFromVal<Env, Val>>(e: &Env, val: &Val) -> T
where
    Val: TryFromVal<Env, T>,
{
    match T::try_from_val(&e, val) {
        Ok(t) => t,
        Err(_) => {
            panic_with_error!(e, GuardError::InvalidValue);
        }
    }
}

fn recursive_assert(e: &Env, val_a: &Val, val_b: &Val) -> bool {
    let type_a = match val_a.get_tag().get_scval_type() {
        Some(t) => t,
        None => {
            panic_with_error!(e, GuardError::InvalidValue);
        }
    };
    let type_b = match val_b.get_tag().get_scval_type() {
        Some(t) => t,
        None => {
            panic_with_error!(e, GuardError::InvalidValue);
        }
    };
    if type_a != type_b {
        panic_with_error!(e, GuardError::TypesMismatch);
    }

    match type_a {
        ScValType::Bool => extract::<bool>(e, val_a) == extract::<bool>(e, val_b),
        ScValType::Void => true, // we've already checked that types match
        ScValType::U32 => extract::<u32>(e, val_a) == extract::<u32>(e, val_b),
        ScValType::I32 => extract::<i32>(e, val_a) == extract::<i32>(e, val_b),
        ScValType::U64 => extract::<u64>(e, val_a) == extract::<u64>(e, val_b),
        ScValType::I64 => extract::<i64>(e, val_a) == extract::<i64>(e, val_b),
        ScValType::U128 => extract::<u128>(e, val_a) == extract::<u128>(e, val_b),
        ScValType::I128 => extract::<i128>(e, val_a) == extract::<i128>(e, val_b),
        ScValType::U256 => extract::<U256>(e, val_a) == extract::<U256>(e, val_b),
        ScValType::I256 => extract::<I256>(e, val_a) == extract::<I256>(e, val_b),
        ScValType::Bytes => extract::<Bytes>(e, val_a) == extract::<Bytes>(e, val_b),
        ScValType::String => extract::<String>(e, val_a) == extract::<String>(e, val_b),
        ScValType::Symbol => extract::<Symbol>(e, val_a) == extract::<Symbol>(e, val_b),
        ScValType::Vec => {
            let vec_a = extract::<Vec<Val>>(e, val_a);
            let vec_b = extract::<Vec<Val>>(e, val_b);
            if vec_a.len() != vec_b.len() {
                return false;
            }
            for i in 0..vec_a.len() {
                if !recursive_assert(e, &vec_a.get_unchecked(i), &vec_b.get_unchecked(i)) {
                    return false;
                }
            }
            true
        }
        ScValType::Map => {
            let map_a = extract::<Vec<(Val, Val)>>(e, val_a);
            let map_b = extract::<Vec<(Val, Val)>>(e, val_b);
            if map_a.len() != map_b.len() {
                return false;
            }
            for ((key_a, val_a), (key_b, val_b)) in map_a.iter().zip(map_b) {
                if !recursive_assert(e, &key_a, &key_b) || !recursive_assert(e, &val_a, &val_b) {
                    return false;
                }
            }
            true
        }
        ScValType::Address => extract::<Address>(e, val_a) == extract::<Address>(e, val_b),
        _ => {
            panic_with_error!(e, GuardError::UnsupportedType);
        }
    }
}

#[contractimpl]
impl GuardInterface for ContractGuard {
    fn assert_result(
        e: Env,
        auth_users: Vec<Address>,
        contract: Address,
        fn_name: Symbol,
        args: Vec<Val>,
        expected_result: Val,
    ) -> Val {
        for user in auth_users {
            user.require_auth();
        }
        let result = e.invoke_contract(&contract, &fn_name, args);
        if !recursive_assert(&e, &result, &expected_result) {
            panic_with_error!(&e, GuardError::ResultsMismatch);
        }
        result
    }
}

mod test;
