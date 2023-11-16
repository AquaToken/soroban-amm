use soroban_sdk::{Address, BytesN, Env};

pub trait UpgradeableContract {
    fn version() -> u32;
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
}

pub trait AdminInterface {
    fn init_admin(e: Env, account: Address);
    fn set_token_hash(e: Env, new_hash: BytesN<32>);
    fn set_pool_hash(e: Env, new_hash: BytesN<32>);
    fn set_stableswap_pool_hash(e: Env, num_tokens: u32, new_hash: BytesN<32>);
    fn configure_init_pool_payment(e: Env, token: Address, amount: i128);
    fn set_reward_token(e: Env, reward_token: Address);
}
