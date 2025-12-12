use crate::balance::read_balance;
use access_control::access::AccessControl;
use access_control::management::SingleAddressManagementTrait;
use access_control::role::Role;
use soroban_sdk::{Address, Env, IntoVal, Symbol, Vec};

pub fn checkpoint_user_rewards(e: &Env, user: Address) {
    let access_control = AccessControl::new(&e);
    let pool_address = access_control.get_role(&Role::Admin);

    if user == pool_address {
        // no need to checkpoint the pool itself
        return;
    }

    e.invoke_contract::<()>(
        &pool_address,
        &Symbol::new(&e, "checkpoint_reward"),
        Vec::from_array(
            &e,
            [
                e.current_contract_address().to_val(),
                user.clone().to_val(),
                (read_balance(&e, user) as u128).into_val(e),
            ],
        ),
    );
}

pub fn checkpoint_user_working_balance(e: &Env, user: Address) {
    let access_control = AccessControl::new(&e);
    let pool_address = access_control.get_role(&Role::Admin);

    if user == pool_address {
        // no need to checkpoint the pool itself
        return;
    }

    e.invoke_contract::<()>(
        &pool_address,
        &Symbol::new(&e, "checkpoint_working_balance"),
        Vec::from_array(
            &e,
            [
                e.current_contract_address().to_val(),
                user.clone().to_val(),
                (read_balance(&e, user) as u128).into_val(e),
            ],
        ),
    );
}
