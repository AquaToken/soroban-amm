use soroban_sdk::{contracttype, Address, Env};

#[derive(Clone)]
#[contracttype]
enum DataKey {
    Allowance(AllowanceDataKey),
}

#[derive(Clone)]
#[contracttype]
struct AllowanceDataKey {
    from: Address,
    spender: Address,
}

#[derive(Clone)]
#[contracttype]
#[derive(Default)]
pub struct AllowanceValue {
    pub amount: i128,
    pub expiration_ledger: u32,
}

pub fn read_allowance(e: &Env, from: Address, spender: Address) -> AllowanceValue {
    let key = DataKey::Allowance(AllowanceDataKey { from, spender });
    match e.storage().temporary().get::<_, AllowanceValue>(&key) {
        Some(allowance) if allowance.expiration_ledger < e.ledger().sequence() => AllowanceValue {
            amount: 0,
            expiration_ledger: allowance.expiration_ledger,
        },
        Some(allowance) => allowance,
        None => AllowanceValue::default(),
    }
}

pub fn write_allowance(
    e: &Env,
    from: Address,
    spender: Address,
    amount: i128,
    expiration_ledger: u32,
) {
    let allowance = AllowanceValue {
        amount,
        expiration_ledger,
    };

    if amount > 0 && expiration_ledger < e.ledger().sequence() {
        panic!("expiration_ledger is less than ledger seq when amount > 0")
    }

    let key = DataKey::Allowance(AllowanceDataKey { from, spender });
    e.storage().temporary().set(&key.clone(), &allowance);

    if amount > 0 {
        let live_for = expiration_ledger
            .checked_sub(e.ledger().sequence())
            .unwrap();

        e.storage().temporary().extend_ttl(&key, live_for, live_for)
    }
}

pub fn spend_allowance(e: &Env, from: Address, spender: Address, amount: i128) {
    let allowance = read_allowance(e, from.clone(), spender.clone());
    if allowance.amount < amount {
        panic!("insufficient allowance");
    }
    if amount > 0 {
        write_allowance(
            e,
            from,
            spender,
            allowance.amount - amount,
            allowance.expiration_ledger,
        );
    }
}
