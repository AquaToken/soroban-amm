use soroban_sdk::{Address, BytesN, Env, Map, Symbol, Val, Vec};

pub trait LiquidityPoolInterfaceTrait {
    fn pool_type(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Symbol;

    fn get_pool(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address;

    // Returns the token contract address for the pool share token
    fn share_id(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address;

    // Getter for the pool balances array.
    fn get_reserves(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Vec<u128>;

    // Getter for the array of swappable coins within the pool.
    fn get_tokens(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Vec<Address>;

    // Deposit coins into the pool.
    // desired_amounts: List of amounts of coins to deposit
    // Returns amounts deposited and the amount of LP tokens received in exchange for the deposited tokens.
    fn deposit(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
        desired_amounts: Vec<u128>,
    ) -> (Vec<u128>, u128);

    // Perform an exchange between two coins.
    // token_in: token to send
    // token_out: token to receive
    // in_amount: Amount of token_in being exchanged
    // out_min: Minimum amount of token_out to receive
    // Returns the actual amount of coin out received
    fn swap(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        token_in: Address,
        token_out: Address,
        pool_index: BytesN<32>,
        in_amount: u128,
        out_min: u128,
    ) -> u128;

    fn estimate_swap(
        e: Env,
        tokens: Vec<Address>,
        token_in: Address,
        token_out: Address,
        pool_index: BytesN<32>,
        in_amount: u128,
    ) -> u128;

    // Withdraw coins from the pool.
    // share_amount: Quantity of LP tokens to burn in the withdrawal
    // min_amounts: Minimum amounts of underlying coins to receive
    // Returns a list of the amounts for each coin that was withdrawn.
    fn withdraw(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
        share_amount: u128,
        min_amounts: Vec<u128>,
    ) -> Vec<u128>;

    fn get_liquidity(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128;
}

pub trait RewardsInterfaceTrait {
    fn get_rewards_config(e: Env) -> Map<Symbol, Val>;
    fn get_total_liquidity(e: Env, tokens: Vec<Address>) -> u128;
    fn config_global_rewards(
        e: Env,
        admin: Address,
        reward_tps: u128,
        expired_at: u64,
        tokens: Vec<(Vec<Address>, u32)>,
    );
    fn fill_liquidity(e: Env, admin: Address, tokens: Vec<Address>);
    fn config_pool_rewards(
        e: Env,
        admin: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
    ) -> bool;

    fn get_rewards_info(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
    ) -> Map<Symbol, i128>;

    fn get_user_reward(e: Env, user: Address, tokens: Vec<Address>, pool_index: BytesN<32>)
        -> u128;

    fn claim(e: Env, user: Address, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128;
}

pub trait PoolsManagementTrait {
    fn init_pool(e: Env, tokens: Vec<Address>) -> (BytesN<32>, Address);

    fn init_standard_pool(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        fee_fraction: u32,
    ) -> (BytesN<32>, Address);

    fn init_stableswap_pool(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        a: u128,
        fee_fraction: u32,
        admin_fee: u32,
    ) -> (BytesN<32>, Address);

    // get pools for given pair
    fn get_pools(e: Env, tokens: Vec<Address>) -> Map<BytesN<32>, Address>;

    // Add initialized custom pool to the list for given pair
    fn add_custom_pool(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_address: Address,
        pool_type: Symbol,
        init_args: Vec<Val>,
    ) -> BytesN<32>;

    // remove pool from the list
    fn remove_pool(e: Env, user: Address, tokens: Vec<Address>, pool_hash: BytesN<32>);
}
