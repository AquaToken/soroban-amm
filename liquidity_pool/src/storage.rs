use crate::constants::{
    INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT,
    PERSISTENT_LIFETIME_THRESHOLD,
};
use soroban_sdk::{contracttype, Address, Env, IntoVal, TryFromVal, Val};

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    TokenA,
    TokenB,
    RewardToken,
    RewardStorage,
    TokenShare,
    ReserveA,
    ReserveB,
    Admin,
    PoolRewardConfig,
    PoolRewardData,
    UserRewardData(Address),
    RewardInvData,
    FeeFraction, // 1 = 0.01%
}

pub fn bump_instance(e: &Env) {
    e.storage()
        .instance()
        .bump(INSTANCE_LIFETIME_THRESHOLD, INSTANCE_BUMP_AMOUNT);
}

pub fn bump_persistent<K>(e: &Env, key: &K)
where
    K: IntoVal<Env, Val>,
{
    e.storage()
        .persistent()
        .bump(key, PERSISTENT_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT);
}

fn get_instance_value<K, V>(e: &Env, key: &K) -> Option<V>
where
    K: IntoVal<Env, Val>,
    V: TryFromVal<Env, Val>,
{
    bump_instance(e);
    e.storage().instance().get(key)
}

fn has_instance_value<K>(e: &Env, key: &K) -> bool
where
    K: IntoVal<Env, Val>,
{
    e.storage().instance().has(key)
}

fn put_instance_value<K, V>(e: &Env, key: &K, value: &V)
where
    K: IntoVal<Env, Val>,
    V: IntoVal<Env, Val>,
{
    bump_instance(e);
    e.storage().instance().set(key, value)
}

pub fn get_token_a(e: &Env) -> Address {
    get_instance_value(e, &DataKey::TokenA).unwrap()
}

pub fn get_token_b(e: &Env) -> Address {
    get_instance_value(e, &DataKey::TokenB).unwrap()
}

pub fn has_reward_token(e: &Env) -> bool {
    has_instance_value(e, &DataKey::RewardToken)
}

pub fn get_reward_token(e: &Env) -> Address {
    get_instance_value(e, &DataKey::RewardToken).unwrap()
}

pub fn get_reward_storage(e: &Env) -> Address {
    get_instance_value(e, &DataKey::RewardStorage).unwrap()
}

pub fn get_token_share(e: &Env) -> Address {
    get_instance_value(e, &DataKey::TokenShare).unwrap()
}

pub fn get_reserve_a(e: &Env) -> i128 {
    get_instance_value(e, &DataKey::ReserveA).unwrap()
}

pub fn get_reserve_b(e: &Env) -> i128 {
    get_instance_value(e, &DataKey::ReserveB).unwrap()
}

pub fn put_token_a(e: &Env, contract: Address) {
    put_instance_value(e, &DataKey::TokenA, &contract);
}

pub fn put_token_b(e: &Env, contract: Address) {
    put_instance_value(e, &DataKey::TokenB, &contract);
}

pub fn put_reward_token(e: &Env, contract: Address) {
    put_instance_value(e, &DataKey::RewardToken, &contract);
}

pub fn put_reward_storage(e: &Env, contract: Address) {
    put_instance_value(e, &DataKey::RewardStorage, &contract);
}

pub fn put_token_share(e: &Env, contract: Address) {
    put_instance_value(e, &DataKey::TokenShare, &contract);
}

pub fn put_reserve_a(e: &Env, amount: i128) {
    put_instance_value(e, &DataKey::ReserveA, &amount)
}

pub fn put_reserve_b(e: &Env, amount: i128) {
    put_instance_value(e, &DataKey::ReserveB, &amount)
}

pub fn get_fee_fraction(e: &Env) -> u32 {
    match get_instance_value(e, &DataKey::FeeFraction) {
        Some(value) => value,
        None => panic!("please initialize fee fraction"),
    }
}

pub fn put_fee_fraction(e: &Env, value: u32) {
    put_instance_value(e, &DataKey::FeeFraction, &value);
}
