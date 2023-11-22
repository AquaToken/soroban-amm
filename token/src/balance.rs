use soroban_sdk::{Address, Env, contracttype};
use utils::bump::{bump_instance, bump_persistent};

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Balance(Address),
    TotalBalance,
}

fn write_balance(e: &Env, addr: Address, amount: i128) {
    let key = DataKey::Balance(addr);
    e.storage().persistent().set(&key, &amount);
    bump_persistent(&e, &key);
}

pub fn read_balance(e: &Env, addr: Address) -> i128 {
    let key = DataKey::Balance(addr);
    match e.storage().persistent().get::<DataKey, i128>(&key) {
        Some(balance) => {
            bump_persistent(&e, &key);
            balance
        }
        None => 0,
    }
}

pub fn receive_balance(e: &Env, addr: Address, amount: i128) {
    let balance = read_balance(e, addr.clone());
    write_balance(e, addr, balance + amount);
}

pub fn spend_balance(e: &Env, addr: Address, amount: i128) {
    let balance = read_balance(e, addr.clone());
    if balance < amount {
        panic!("insufficient balance");
    }
    write_balance(e, addr, balance - amount);
}

pub fn read_total_balance(e: &Env) -> i128 {
    bump_instance(&e);
    e.storage().instance().get(&DataKey::TotalBalance).unwrap()
}

pub fn write_total_balance(e: &Env, amount: i128) {
    bump_instance(&e);
    e.storage().instance().set(&DataKey::TotalBalance, &amount);
}

pub fn increase_total_balance(e: &Env, amount: i128) {
    let mut total_balance = read_total_balance(e);
    total_balance += amount;
    write_total_balance(e, total_balance);
}

pub fn decrease_total_balance(e: &Env, amount: i128) {
    let mut total_balance = read_total_balance(e);
    total_balance -= amount;
    write_total_balance(e, total_balance);
}
