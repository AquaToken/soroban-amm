use soroban_sdk::{Address, BytesN, Env, Map, Symbol, Vec};

pub trait LiquidityPoolInterfaceTrait {
    fn get_pool(
        e: Env,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
    ) -> (bool, Address);

    // Returns the token contract address for the pool share token
    fn share_id(e: Env, token_a: Address, token_b: Address, pool_index: BytesN<32>) -> Address;

    // Getter for the pool balances array.
    fn get_reserves(
        e: Env,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
    ) -> Vec<i128>;

    // Getter for the array of swappable coins within the pool.
    fn get_tokens(
        e: Env,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
    ) -> Vec<Address>;

    // Deposit coins into the pool.
    // desired_amounts: List of amounts of coins to deposit
    // Returns amounts deposited and the amount of LP tokens received in exchange for the deposited tokens.
    fn deposit(
        e: Env,
        user: Address,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
        desired_amounts: Vec<i128>,
    ) -> (Vec<i128>, i128);

    // Perform an exchange between two coins.
    // token_in: token to send
    // token_out: token to receive
    // in_amount: Amount of token_in being exchanged
    // out_min: Minimum amount of token_out to receive
    // Returns the actual amount of coin out received
    fn swap(
        e: Env,
        user: Address,
        token_in: Address,
        token_out: Address,
        pool_index: BytesN<32>,
        in_amount: i128,
        out_min: i128,
    ) -> i128;

    fn estimate_swap(
        e: Env,
        token_in: Address,
        token_out: Address,
        pool_index: BytesN<32>,
        in_amount: i128,
    ) -> i128;

    // Withdraw coins from the pool.
    // share_amount: Quantity of LP tokens to burn in the withdrawal
    // min_amounts: Minimum amounts of underlying coins to receive
    // Returns a list of the amounts for each coin that was withdrawn.
    fn withdraw(
        e: Env,
        user: Address,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
        share_amount: i128,
        min_amounts: Vec<i128>,
    ) -> Vec<i128>;
}

pub trait RewardsInterfaceTrait {
    fn get_user_reward(
        e: Env,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
        user: Address,
    ) -> i128;

    fn claim(
        e: Env,
        token_a: Address,
        token_b: Address,
        pool_index: BytesN<32>,
        user: Address,
    ) -> BytesN<32>;
}

pub trait PoolsManagementTrait {
    fn init_pool(e: Env, token_a: Address, token_b: Address) -> (BytesN<32>, Address);

    // fn init_standard_pool(e: Env, token_a: Address, token_b: Address, fee_fraction: u32)
    //     -> Address;
    //
    // fn init_stableswap_pool(
    //     e: Env,
    //     token_a: Address,
    //     token_b: Address,
    //     a: u128,
    //     fee_fraction: u128,
    //     admin_fee: u128,
    // ) -> Address;
    //
    // // get pools for given pair
    fn get_pools(e: Env, token_a: Address, token_b: Address) -> Map<BytesN<32>, Address>;

    // // Add initialized custom pool to the list for given pair
    // fn add_pool(e: Env, token_a: Address, token_b: Address, pool_address: Address, pool_description: Symbol);
    //
    // // remove pool from the list
    // fn remove_pool(e: Env, token_a: Address, token_b: Address, pool_hash: BytesN<32>);
}