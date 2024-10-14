use crate::balance::read_balance;
use access_control::access::{AccessControl, AccessControlTrait};
use soroban_sdk::{Address, Env, IntoVal, Symbol, Vec};

pub fn checkpoint_user_rewards(e: &Env, user: Address) {
    let access_control = AccessControl::new(&e);
    let pool_address = access_control.get_admin().unwrap();

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
