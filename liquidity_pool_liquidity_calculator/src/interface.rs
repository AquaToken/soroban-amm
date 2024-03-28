use soroban_sdk::{Address, Env, Vec, U256};

pub trait Calculator {
    fn init_admin(e: Env, account: Address);
    fn set_pools_plane(e: Env, admin: Address, plane: Address);
    fn get_pools_plane(e: Env) -> Address;
    fn get_liquidity(e: Env, pools: Vec<Address>) -> Vec<U256>;
}
