use soroban_sdk::{Address, BytesN, Env, Map, Symbol, Vec};

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Upgrade contract with new wasm code
    fn commit_upgrade(e: Env, admin: Address, new_wasm_hash: BytesN<32> );
    fn apply_upgrade(e: Env, admin: Address) -> BytesN<32>;
    fn revert_upgrade(e: Env, admin: Address);

    // Emergency mode - bypass upgrade deadline
    fn set_emergency_admin(e: Env, admin: Address, emergency_admin: Address);
    fn set_emergency_mode(e: Env, admin: Address, value: bool);
    fn get_emergency_mode(e: Env) -> bool;
}

pub trait AdminInterface {
    // Initialize admin user. Will panic if called twice
    fn init_admin(e: Env, account: Address);

    // Set privileged addresses
    fn set_privileged_addrs(
        e: Env,
        admin: Address,
        rewards_admin: Address,
        operations_admin: Address,
        pause_admin: Address,
        emergency_pause_admins: Vec<Address>,
    );

    // Get map of privileged roles
    fn get_privileged_addrs(e: Env) -> Map<Symbol, Vec<Address>>;

    // Set liquidity pool token wasm hash
    fn set_token_hash(e: Env, admin: Address, new_hash: BytesN<32>);

    // Set standard pool wasm hash
    fn set_pool_hash(e: Env, admin: Address, new_hash: BytesN<32>);

    // Set stableswap pool wasm hash
    fn set_stableswap_pool_hash(e: Env, admin: Address, new_hash: BytesN<32>);

    // Configure stableswap init payment: token address, amount and destination address
    fn configure_init_pool_payment(
        e: Env,
        admin: Address,
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
    fn set_reward_token(e: Env, admin: Address, reward_token: Address);
}
