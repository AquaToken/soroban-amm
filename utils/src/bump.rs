use crate::constant::{MAX_TEMPORARY_TTL, TEMPORARY_TTL_THRESHOLD};
use soroban_sdk::{Env, IntoVal, Val};

pub fn bump_temporary<K>(e: &Env, key: &K)
where
    K: IntoVal<Env, Val>,
{
    e.storage()
        .temporary()
        .extend_ttl(key, TEMPORARY_TTL_THRESHOLD, MAX_TEMPORARY_TTL);
}
