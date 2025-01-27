use soroban_sdk::{Address, Env, Symbol, Vec};

pub trait PlaneInterface {
    // Initializes the admin user.
    fn init_admin(e: Env, account: Address);

    // update pool stored data. any pool can use it to store it's information
    fn update(e: Env, pool: Address, pool_type: Symbol, init_args: Vec<u128>, reserves: Vec<u128>);

    // get details for many pools: type string representation, pool parameters and reserves amount
    fn get(e: Env, pools: Vec<Address>) -> Vec<(Symbol, Vec<u128>, Vec<u128>)>;
}
