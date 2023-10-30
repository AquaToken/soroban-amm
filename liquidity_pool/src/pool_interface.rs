use soroban_sdk::{Address, BytesN, Env, Map, Symbol, Vec};

pub trait LiquidityPoolTrait {
    // Sets the token contract addresses for this pool
    // todo: move rewards configuration to gauge
    fn initialize(
        e: Env,
        admin: Address,
        lp_token_wasm_hash: BytesN<32>,
        tokens: Vec<Address>,
        fee_fraction: u32,
    ) -> bool;

    fn get_fee_fraction(e: Env) -> u32;

    // Returns the token contract address for the pool share token
    fn share_id(e: Env) -> Address;
    fn get_reserves(e: Env) -> Vec<i128>;
    fn get_tokens(e: Env) -> Vec<Address>;

    // Deposits token_a and token_b. Also mints pool shares for the "to" Identifier. The amount minted
    // is determined based on the difference between the reserves stored by this contract, and
    // the actual balance of token_a and token_b for this contract.
    fn deposit(e: Env, user: Address, desired_amounts: Vec<i128>) -> (Vec<i128>, i128);

    // the swap will sell in_idx token and buy out_idx token
    fn swap(
        e: Env,
        user: Address,
        in_idx: u32,
        out_idx: u32,
        in_amount: i128,
        out_min: i128,
    ) -> i128;
    fn estimate_swap(e: Env, in_idx: u32, out_idx: u32, in_amount: i128) -> i128;

    // transfers share_amount of pool share tokens to this contract, burns all pools share tokens in this contracts, and sends the
    // corresponding amount of token_a and token_b to "to".
    // Returns amount of both tokens withdrawn
    fn withdraw(e: Env, to: Address, share_amount: i128, min_a: i128, min_b: i128) -> (i128, i128);

    fn version() -> u32;
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) -> bool;
}

pub trait RewardsTrait {
    fn initialize_rewards_config(
        e: Env,
        admin: Address,
        reward_token: Address,
        reward_storage: Address,
    );
    fn set_rewards_config(e: Env, admin: Address, expired_at: u64, amount: i128) -> bool;
    fn get_rewards_info(e: Env, user: Address) -> Map<Symbol, i128>;
    fn get_user_reward(e: Env, user: Address) -> i128;
    fn claim(e: Env, user: Address) -> i128;
}
