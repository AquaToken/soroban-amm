use soroban_sdk::token::TokenClient as SorobanTokenClient;
use soroban_sdk::{symbol_short, Address, Env, Symbol, Vec};

pub fn get_token_share(e: &Env, pool_id: &Address) -> Address {
    e.invoke_contract(pool_id, &symbol_short!("share_id"), Vec::from_array(&e, []))
}

pub fn get_user_shares(e: &Env, pool_id: &Address, user: &Address) -> u128 {
    SorobanTokenClient::new(e, &get_token_share(e, pool_id)).balance(user) as u128
}

pub fn get_total_shares(e: &Env, pool_id: &Address) -> u128 {
    e.invoke_contract(
        pool_id,
        &Symbol::new(e, "get_total_shares"),
        Vec::from_array(&e, []),
    )
}
