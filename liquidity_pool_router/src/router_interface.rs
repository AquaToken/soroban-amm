use soroban_sdk::{Address, BytesN, Env};

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Upgrade contract with new wasm code
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
}

pub trait AdminInterface {
    // Initialize admin user. Will panic if called twice
    fn init_admin(e: Env, account: Address);

    // Set liquidity pool token wasm hash
    fn set_token_hash(e: Env, new_hash: BytesN<32>);

    // Set standard pool wasm hash
    fn set_pool_hash(e: Env, new_hash: BytesN<32>);

    // Set stableswap pool wasm hash
    fn set_stableswap_pool_hash(e: Env, new_hash: BytesN<32>);

    // Configure stableswap init payment: token address, amount and destination address
    fn configure_init_pool_payment(
        e: Env,
        token: Address,
        stable_pool_amount: u128,
        standard_pool_amount: u128,
        to: Address,
    );

    // Set reward token address
    fn set_reward_token(e: Env, reward_token: Address);
}
