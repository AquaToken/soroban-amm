use soroban_sdk::{Address, BytesN, Env, Map, Symbol, Val, Vec};

pub trait LiquidityPoolInterfaceTrait {
    fn pool_type(e: Env) -> Symbol;

    // Sets the token contract addresses for this pool
    fn initialize(
        e: Env,
        admin: Address,
        lp_token_wasm_hash: BytesN<32>,
        tokens: Vec<Address>,
        a: u128,
        fee_fraction: u128,
        admin_fee: u128,
    );

    // The pool swap fee, as an integer with 1e4 precision. 0.01% = 1; 0.3% = 30; 1% = 100;
    fn get_fee_fraction(e: Env) -> u32;

    // The percentage of the swap fee that is taken as an admin fee, as an integer with with 1e4 precision.
    fn get_admin_fee(e: Env) -> u32;

    // Returns the token contract address for the pool share token
    fn share_id(e: Env) -> Address;

    // Getter for the pool balances array.
    fn get_reserves(e: Env) -> Vec<u128>;

    // Getter for the array of swappable coins within the pool.
    fn get_tokens(e: Env) -> Vec<Address>;

    // Deposit coins into the pool.
    // desired_amounts: List of amounts of coins to deposit
    // Returns amounts deposited and the amount of LP tokens received in exchange for the deposited tokens.
    fn deposit(e: Env, user: Address, desired_amounts: Vec<u128>) -> (Vec<u128>, u128);

    // Perform an exchange between two coins.
    // in_idx: Index value for the coin to send
    // out_idx: Index value of the coin to receive
    // in_amount: Amount of in_idx being exchanged
    // out_min: Minimum amount of out_idx to receive
    // Returns the actual amount of coin out_idx received. Index values can be found via the get_tokens public getter method.
    fn swap(
        e: Env,
        user: Address,
        in_idx: u32,
        out_idx: u32,
        in_amount: u128,
        out_min: u128,
    ) -> u128;

    fn estimate_swap(e: Env, in_idx: u32, out_idx: u32, in_amount: u128) -> u128;

    // Withdraw coins from the pool.
    // share_amount: Quantity of LP tokens to burn in the withdrawal
    // min_amounts: Minimum amounts of underlying coins to receive
    // Returns a list of the amounts for each coin that was withdrawn.
    fn withdraw(e: Env, user: Address, share_amount: u128, min_amounts: Vec<u128>) -> Vec<u128>;

    fn get_liquidity(e: Env) -> u128;

    fn get_info(e: Env) -> Map<Symbol, Val>;
}

pub trait UpgradeableContractTrait {
    fn version() -> u32;
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
}

pub trait RewardsTrait {
    // todo: move rewards configuration to gauge
    fn initialize_rewards_config(e: Env, reward_token: Address, reward_storage: Address);
    fn set_rewards_config(e: Env, admin: Address, expired_at: u64, tps: u128);
    fn get_rewards_info(e: Env, user: Address) -> Map<Symbol, i128>;
    fn get_user_reward(e: Env, user: Address) -> u128;
    fn claim(e: Env, user: Address) -> u128;
}

pub trait AdminInterfaceTrait {
    fn ramp_a(e: Env, admin: Address, future_a: u128, future_time: u64);
    fn stop_ramp_a(e: Env, admin: Address);
    fn commit_new_fee(e: Env, admin: Address, new_fee: u128, new_admin_fee: u128);
    fn apply_new_fee(e: Env, admin: Address);
    fn revert_new_parameters(e: Env, admin: Address);
    fn commit_transfer_ownership(e: Env, admin: Address, new_admin: Address);
    fn apply_transfer_ownership(e: Env, admin: Address);
    fn revert_transfer_ownership(e: Env, admin: Address);
    fn admin_balances(e: Env, i: u32) -> u128;
    fn withdraw_admin_fees(e: Env, admin: Address);
    fn donate_admin_fees(e: Env, admin: Address);
    fn kill_me(e: Env, admin: Address);
    fn unkill_me(e: Env, admin: Address);
}

pub trait InternalInterfaceTrait {
    fn xp(e: Env) -> Vec<u128>;
    fn xp_mem(e: Env, balances: Vec<u128>) -> Vec<u128>;
    fn get_d(e: Env, xp: Vec<u128>, amp: u128) -> u128;
    fn get_d_mem(e: Env, balances: Vec<u128>, amp: u128) -> u128;
    fn get_y(e: Env, i: u32, j: u32, x: u128, xp_: Vec<u128>) -> u128;
    fn get_y_d(e: Env, a: u128, i: u32, xp: Vec<u128>, d: u128) -> u128;
    fn internal_calc_withdraw_one_coin(e: Env, _token_amount: u128, i: u32) -> (u128, u128);
}

pub trait LiquidityPoolTrait:
    LiquidityPoolInterfaceTrait
    + UpgradeableContractTrait
    + RewardsTrait
    + AdminInterfaceTrait
    + InternalInterfaceTrait
{
    // The amplification coefficient for the pool.
    fn a(e: Env) -> u128;

    // Returns portfolio virtual price (for calculating profit) scaled up by 1e7
    fn get_virtual_price(e: Env) -> u128;

    // Simplified method to calculate addition or reduction in token supply at
    // deposit or withdrawal without taking fees into account (but looking at
    // slippage).
    // Needed to prevent front-running, not for precise calculations!
    fn calc_token_amount(e: Env, amounts: Vec<u128>, deposit: bool) -> u128;

    // Get the amount of coin j one would receive for swapping dx of coin i.
    fn get_dy(e: Env, i: u32, j: u32, dx: u128) -> u128;

    fn get_dy_underlying(e: Env, i: u32, j: u32, dx: u128) -> u128;

    // Withdraw coins from the pool in an imbalanced amount.
    // amounts: List of amounts of underlying coins to withdraw
    // max_burn_amount: Maximum amount of LP token to burn in the withdrawal
    // Returns actual amount of the LP tokens burned in the withdrawal.
    fn remove_liquidity_imbalance(
        e: Env,
        user: Address,
        amounts: Vec<u128>,
        max_burn_amount: u128,
    ) -> u128;

    // Calculate the amount received when withdrawing a single coin.
    // token_amount: Amount of LP tokens to burn in the withdrawal
    // i: Index value of the coin to withdraw
    fn calc_withdraw_one_coin(e: Env, _token_amount: u128, i: u32) -> u128;

    // Withdraw a single coin from the pool.
    // token_amount: Amount of LP tokens to burn in the withdrawal
    // i: Index value of the coin to withdraw
    // min_amount: Minimum amount of coin to receive
    // Returns the amount of coin i received.
    fn withdraw_one_coin(e: Env, user: Address, token_amount: u128, i: u32, min_amount: u128);
}
