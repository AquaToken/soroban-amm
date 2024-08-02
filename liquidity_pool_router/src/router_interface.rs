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

    // Set operator address which can perform some restricted actions
    fn set_operator(e: Env, operator: Address);

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

    // Getters for init pool payment info
    fn get_init_pool_payment_token(e: Env) -> Address;
    fn get_init_pool_payment_address(e: Env) -> Address;
    fn get_stable_pool_payment_amount(e: Env) -> u128;
    fn get_standard_pool_payment_amount(e: Env) -> u128;

    // Set reward token address
    fn set_reward_token(e: Env, reward_token: Address);

    // Get operator address
    fn get_operator(e: Env) -> Address;
}
