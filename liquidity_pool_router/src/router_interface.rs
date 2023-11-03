use soroban_sdk::{Address, BytesN, Env, Map, Symbol, Vec};

pub trait LiquidityPoolRouterTrait {
    fn init_pool(e: Env, token_a: Address, token_b: Address) -> Address;
    fn init_standard_pool(e: Env, token_a: Address, token_b: Address, fee_fraction: u32)
        -> Address;

    fn init_stableswap_pool(
        e: Env,
        token_a: Address,
        token_b: Address,
        a: u128,
        fee_fraction: u128,
        admin_fee: u128,
    ) -> Address;

    fn get_pools_list(e: Env) -> Vec<Address>;

    fn get_pool_hash(e: Env) -> BytesN<32>;
    // fn set_pool_hash(e: Env, new_hash: BytesN<32>);

    fn get_token_hash(e: Env) -> BytesN<32>;
    // fn set_token_hash(e: Env, new_hash: BytesN<32>);
    fn get_reward_token(e: Env) -> Address;
    // fn set_reward_token(e: Env, reward_token: Address);

    fn deposit(
        e: Env,
        account: Address,
        token_a: Address,
        token_b: Address,
        desired_a: i128,
        min_a: i128,
        desired_b: i128,
        min_b: i128,
    ) -> (i128, i128);

    // swaps out an exact amount of "buy", in exchange for "sell" that this contract has an
    // allowance for from "to". "sell" amount swapped in must not be greater than "in_max"
    fn swap_out(
        e: Env,
        account: Address,
        sell: Address,
        buy: Address,
        out: i128,
        in_max: i128,
    ) -> i128;
    fn estimate_swap_out(e: Env, sell: Address, buy: Address, out: i128) -> i128;

    fn withdraw(
        e: Env,
        account: Address,
        token_a: Address,
        token_b: Address,
        share_amount: i128,
        min_a: i128,
        min_b: i128,
    ) -> (i128, i128);

    // returns the contract address for the specified token_a/token_b combo
    fn get_pool(e: Env, token_a: Address, token_b: Address) -> (bool, Address);

    // get pool reserves amount. it may differ from pool balance
    fn get_reserves(e: Env, token_a: Address, token_b: Address) -> (i128, i128);
    fn set_rewards_config(
        e: Env,
        token_a: Address,
        token_b: Address,
        admin: Address,
        expired_at: u64,
        amount: i128,
    );
    fn get_rewards_info(
        e: Env,
        token_a: Address,
        token_b: Address,
        user: Address,
    ) -> Map<Symbol, i128>;
    fn get_user_reward(e: Env, token_a: Address, token_b: Address, user: Address) -> i128;
    fn claim(e: Env, token_a: Address, token_b: Address, user: Address) -> i128;
}

pub trait UpgradeableContract {
    fn version() -> u32;
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
}

pub trait AdminInterface {
    fn init_admin(e: Env, account: Address);
    fn set_token_hash(e: Env, new_hash: BytesN<32>);
    fn set_pool_hash(e: Env, new_hash: BytesN<32>);
    fn set_reward_token(e: Env, reward_token: Address);
}
