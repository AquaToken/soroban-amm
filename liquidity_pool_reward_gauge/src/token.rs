use soroban_sdk::{Address, Env, Symbol, Vec};

pub fn get_reward_token(e: &Env, gauge: &Address) -> Address {
    e.invoke_contract(gauge, &Symbol::new(&e, "get_reward_token"), Vec::new(e))
}
