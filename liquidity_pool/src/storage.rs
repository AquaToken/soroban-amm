use crate::constants::{
    INSTANCE_BUMP_AMOUNT, INSTANCE_LIFETIME_THRESHOLD, PERSISTENT_BUMP_AMOUNT,
    PERSISTENT_LIFETIME_THRESHOLD,
};
use soroban_sdk::{contracttype, Address, Env, IntoVal, Val};

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

pub fn get_token_a(e: &Env) -> Address {
    bump_instance(e);
    e.storage().instance().get(&DataKey::TokenA).unwrap()
}

pub fn get_token_b(e: &Env) -> Address {
    bump_instance(e);
    e.storage().instance().get(&DataKey::TokenB).unwrap()
}

pub fn get_reward_token(e: &Env) -> Address {
    bump_instance(e);
    e.storage().instance().get(&DataKey::RewardToken).unwrap()
}

pub fn get_reward_storage(e: &Env) -> Address {
    bump_instance(e);
    e.storage().instance().get(&DataKey::RewardStorage).unwrap()
}

pub fn get_token_share(e: &Env) -> Address {
    bump_instance(e);
    e.storage().instance().get(&DataKey::TokenShare).unwrap()
}

pub fn get_reserve_a(e: &Env) -> i128 {
    bump_instance(e);
    e.storage().instance().get(&DataKey::ReserveA).unwrap()
}

pub fn get_reserve_b(e: &Env) -> i128 {
    bump_instance(e);
    e.storage().instance().get(&DataKey::ReserveB).unwrap()
}

pub fn put_token_a(e: &Env, contract: Address) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::TokenA, &contract);
}

pub fn put_token_b(e: &Env, contract: Address) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::TokenB, &contract);
}

pub fn put_reward_token(e: &Env, contract: Address) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::RewardToken, &contract);
}

pub fn put_reward_storage(e: &Env, contract: Address) {
    bump_instance(e);
    e.storage()
        .instance()
        .set(&DataKey::RewardStorage, &contract);
}

pub fn put_token_share(e: &Env, contract: Address) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::TokenShare, &contract);
}

pub fn put_reserve_a(e: &Env, amount: i128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::ReserveA, &amount)
}

pub fn put_reserve_b(e: &Env, amount: i128) {
    bump_instance(e);
    e.storage().instance().set(&DataKey::ReserveB, &amount)
}
