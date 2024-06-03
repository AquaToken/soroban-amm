use crate::constant::DAY_IN_LEDGERS;
use soroban_sdk::{Env, IntoVal, Val};

pub fn bump_instance(e: &Env) {
    let max_ttl = e.storage().max_ttl();
    e.storage()
        .instance()
        .extend_ttl(max_ttl - DAY_IN_LEDGERS, max_ttl);
}

pub fn bump_persistent<K>(e: &Env, key: &K)
where
    K: IntoVal<Env, Val>,
{
    let max_ttl = e.storage().max_ttl();
    e.storage()
        .persistent()
        .extend_ttl(key, max_ttl - DAY_IN_LEDGERS, max_ttl);
}
