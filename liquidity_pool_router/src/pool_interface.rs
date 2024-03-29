use soroban_sdk::{Address, BytesN, Env, Map, Symbol, Val, Vec};

pub trait LiquidityPoolInterfaceTrait {
    // Get symbolic explanation of pool type.
    fn pool_type(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Symbol;

    // Get dictionary of basic pool information: type, fee, special parameters if any.
    fn get_info(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Map<Symbol, Val>;

    // Get address for specified pool index.
    fn get_pool(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address;

    // Returns the token contract address for the pool share token.
    fn share_id(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Address;

    // Returns the total amount of shares
    fn get_total_shares(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128;

    // Getter for the pool balances array.
    fn get_reserves(e: Env, tokens: Vec<Address>, pool_index: BytesN<32>) -> Vec<u128>;

    // Deposit coins into the pool.
    // desired_amounts: List of amounts of coins to deposit
    // Returns amounts deposited and the amount of LP tokens received in exchange for the deposited tokens.
    fn deposit(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
        desired_amounts: Vec<u128>,
        min_shares: u128,
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

    // Estimate amount of coins to retrieve using swap function
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
}

pub trait RewardsInterfaceTrait {
    // Configure rewards for pool. Every second tps of coins
    // being distributed across all liquidity providers
    // after expired_at timestamp distribution ends
    fn set_rewards_config(
        e: Env,
        admin: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
        expired_at: u64,
        tps: u128,
    );

    // Get rewards status for the pool,
    // including amount available for the user
    fn get_rewards_info(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        pool_index: BytesN<32>,
    ) -> Map<Symbol, i128>;

    // Get amount of reward tokens available for the user to claim.
    fn get_user_reward(e: Env, user: Address, tokens: Vec<Address>, pool_index: BytesN<32>)
        -> u128;

    // Claim reward as a user.
    // returns amount of tokens rewarded to the user
    fn claim(e: Env, user: Address, tokens: Vec<Address>, pool_index: BytesN<32>) -> u128;
}

pub trait PoolsManagementTrait {
    // Initialize standard pool with default arguments
    fn init_pool(e: Env, tokens: Vec<Address>) -> (BytesN<32>, Address);

    // Initialize standard pool with custom arguments.
    // fee_fraction should match pre-defined set of values: 0.1%, 0.3%, 1%
    // 10 = 0.1%, 30 = 0.3%, 100 = 1%
    fn init_standard_pool(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        fee_fraction: u32,
    ) -> (BytesN<32>, Address);

    // Initialize stableswap pool with custom arguments.
    // a - amplification coefficient
    // fee_fraction has denominator 10000; 1 = 0.01%, 10 = 0.1%, 100 = 1%
    // admin_fee - percentage of fee that goes to pool admin
    fn init_stableswap_pool(
        e: Env,
        user: Address,
        tokens: Vec<Address>,
        a: u128,
        fee_fraction: u32,
        admin_fee: u32,
    ) -> (BytesN<32>, Address);

    // Get pools for given pair
    fn get_pools(e: Env, tokens: Vec<Address>) -> Map<BytesN<32>, Address>;

    // Remove pool from the list
    fn remove_pool(e: Env, user: Address, tokens: Vec<Address>, pool_hash: BytesN<32>);

    // Calculates the number of unique token sets.
    fn get_tokens_sets_count(e: Env) -> u128;

    // Retrieves tokens at a specified index
    fn get_tokens(e: Env, index: u128) -> Vec<Address>;

    // Retrieves a lists of pools in batch based on half-open `[..)` range of tokens indexes.
    //
    // # Returns
    //
    // A list containing tuples containing a vector of addresses of the corresponding tokens
    // and a mapping of pool hashes to pool addresses.
    fn get_pools_for_tokens_range(
        e: Env,
        start: u128,
        end: u128,
    ) -> Vec<(Vec<Address>, Map<BytesN<32>, Address>)>;
}

pub trait PoolPlaneInterface {
    // configure pools plane address to be used as lightweight proxy to optimize instructions & batch operations
    fn set_pools_plane(e: Env, admin: Address, plane: Address);

    // get pools plane address
    fn get_plane(e: Env) -> Address;
}

pub trait SwapRouterInterface {
    // Estimate swap comparing all the available pools for given tokens set.
    //  returns best pool hash, address and estimated out value
    fn estimate_swap_routed(
        e: Env,
        tokens: Vec<Address>,
        token_in: Address,
        token_out: Address,
        in_amount: u128,
    ) -> (BytesN<32>, Address, u128);

    // Set swap router address. it's separate contract optimized to estimate swap for multiple pools
    fn set_swap_router(e: Env, admin: Address, router: Address);

    // Get swap router address
    fn get_swap_router(e: Env) -> Address;
}

pub trait CombinedSwapInterface {
    /// Executes a chain of token swaps to exchange an input token for an output token.
    ///
    /// # Arguments
    ///
    /// * `user` - The address of the user executing the swaps.
    /// * `swaps_chain` - The series of swaps to be executed. Each swap is represented by a tuple containing:
    ///   - A vector of token addresses liquidity pool belongs to
    ///   - Pool index hash
    ///   - The token to obtain
    /// * `token_in` - The address of the input token to be swapped.
    /// * `in_amount` - The amount of the input token to be swapped.
    /// * `out_min` - The minimum amount of the output token to be received.
    ///
    /// # Returns
    ///
    /// The amount of the output token received after all swaps have been executed.
    fn swap_chained(
        e: Env,
        user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address,
        in_amount: u128,
        out_min: u128,
    ) -> u128;
}
