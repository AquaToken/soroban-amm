use soroban_sdk::{Address, BytesN, Env, Map, Symbol, Vec};

pub trait LiquidityPoolTrait {
    fn pool_type(e: Env) -> Symbol;

    // Sets the token contract addresses for this pool
    fn initialize(
        e: Env,
        admin: Address,
        lp_token_wasm_hash: BytesN<32>,
        tokens: Vec<Address>,
        fee_fraction: u32,
    );

    // Returns the token contract address for the pool share token
    fn share_id(e: Env) -> Address;

    fn get_tokens(e: Env) -> Vec<Address>;

    // Deposits token_a and token_b. Also mints pool shares for the "to" Identifier. The amount minted
    // is determined based on the difference between the reserves stored by this contract, and
    // the actual balance of token_a and token_b for this contract.
    fn deposit(e: Env, user: Address, desired_amounts: Vec<u128>) -> (Vec<u128>, u128);

    // swap will sell in_idx token and buy out_idx token
    fn swap(
        e: Env,
        user: Address,
        in_idx: u32,
        out_idx: u32,
        in_amount: u128,
        out_min: u128,
    ) -> u128;

    fn estimate_swap(e: Env, in_idx: u32, out_idx: u32, in_amount: u128) -> u128;

    // transfers share_amount of pool share tokens to this contract, burns all pools share tokens in this contracts, and sends the
    // corresponding amount of tokens"user".
    // Returns amount of tokens withdrawn
    fn withdraw(e: Env, user: Address, share_amount: u128, min_amounts: Vec<u128>) -> Vec<u128>;

    fn get_reserves(e: Env) -> Vec<u128>;

    fn get_fee_fraction(e: Env) -> u32;

    fn get_liquidity(e: Env) -> u128;
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
