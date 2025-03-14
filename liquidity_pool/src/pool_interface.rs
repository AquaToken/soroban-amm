use soroban_sdk::{Address, BytesN, Env, Map, Symbol, Val, Vec};

pub trait LiquidityPoolCrunch {
    // Initialize pool completely to reduce calculations cost
    fn initialize_all(
        e: Env,
        admin: Address,
        privileged_addrs: (Address, Address, Address, Address, Vec<Address>),
        router: Address,
        lp_token_wasm_hash: BytesN<32>,
        tokens: Vec<Address>,
        fee_fraction: u32,
        reward_config: (Address, Address, Address),
        plane: Address,
    );
}

pub trait LiquidityPoolTrait {
    // Get symbolic explanation of pool type.
    fn pool_type(e: Env) -> Symbol;

    // Sets the token contract addresses for this pool
    fn initialize(
        e: Env,
        admin: Address,
        privileged_addrs: (Address, Address, Address, Address, Vec<Address>),
        router: Address,
        lp_token_wasm_hash: BytesN<32>,
        tokens: Vec<Address>,
        fee_fraction: u32,
    );

    // Returns the token contract address for the pool share token
    fn share_id(e: Env) -> Address;

    // Returns the total amount of shares
    fn get_total_shares(e: Env) -> u128;

    fn get_tokens(e: Env) -> Vec<Address>;

    // Deposits token_a and token_b. Also mints pool shares for the "to" Identifier. The amount minted
    // is determined based on the difference between the reserves stored by this contract, and
    // the actual balance of token_a and token_b for this contract.
    fn deposit(
        e: Env,
        user: Address,
        desired_amounts: Vec<u128>,
        min_shares: u128,
    ) -> (Vec<u128>, u128);

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

    // Estimate amount of coins to retrieve using swap function
    fn estimate_swap(e: Env, in_idx: u32, out_idx: u32, in_amount: u128) -> u128;

    // Perform an exchange between two coins with strict amount to receive.
    // in_idx: Index value for the coin to send
    // out_idx: Index value of the coin to receive
    // out_amount: Amount of out_idx being exchanged
    // in_max: Maximum amount of in_idx to send
    fn swap_strict_receive(
        e: Env,
        user: Address,
        in_idx: u32,
        out_idx: u32,
        out_amount: u128,
        in_max: u128,
    ) -> u128;

    // Estimate amount of coins to retrieve using swap_strict_receive function
    fn estimate_swap_strict_receive(e: Env, in_idx: u32, out_idx: u32, out_amount: u128) -> u128;

    // Transfers share_amount of pool share tokens to this contract,
    // burns all pools share tokens in this contracts, and sends
    // the corresponding amount of tokens to user.
    // Returns amount of tokens withdrawn
    fn withdraw(e: Env, user: Address, share_amount: u128, min_amounts: Vec<u128>) -> Vec<u128>;

    // Get pool reserves
    fn get_reserves(e: Env) -> Vec<u128>;

    // Fee fraction getter. 1 = 0.01%
    fn get_fee_fraction(e: Env) -> u32;

    // Get dictionary of basic pool information: type, fee, special parameters if any.
    fn get_info(e: Env) -> Map<Symbol, Val>;
}

pub trait AdminInterfaceTrait {
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

    // Stop pool instantly
    fn kill_deposit(e: Env, admin: Address);
    fn kill_swap(e: Env, admin: Address);
    fn kill_claim(e: Env, admin: Address);

    // Resume pool
    fn unkill_deposit(e: Env, admin: Address);
    fn unkill_swap(e: Env, admin: Address);
    fn unkill_claim(e: Env, admin: Address);

    // Get killswitch status
    fn get_is_killed_deposit(e: Env) -> bool;
    fn get_is_killed_swap(e: Env) -> bool;
    fn get_is_killed_claim(e: Env) -> bool;
}

pub trait UpgradeableContract {
    // Get contract version
    fn version() -> u32;

    // Upgrade contract with new wasm code
    fn commit_upgrade(
        e: Env,
        admin: Address,
        new_wasm_hash: BytesN<32>,
        new_token_wasm_hash: BytesN<32>,
    );
    fn apply_upgrade(e: Env, admin: Address) -> (BytesN<32>, BytesN<32>);
    fn revert_upgrade(e: Env, admin: Address);

    // Emergency mode - bypass upgrade deadline
    fn set_emergency_mode(e: Env, admin: Address, value: bool);
    fn get_emergency_mode(e: Env) -> bool;
}

pub trait UpgradeableLPTokenTrait {
    // legacy methods to upgrade token contract up to version 120. future versions will use commit_upgrade
    fn upgrade_token_legacy(e: Env, admin: Address, new_token_wasm: BytesN<32>);
}

pub trait RewardsTrait {
    // Initialize rewards token address
    fn initialize_rewards_config(e: Env, reward_token: Address);

    fn initialize_boost_config(e: Env, reward_boost_token: Address, reward_boost_feed: Address);

    fn set_reward_boost_config(
        e: Env,
        admin: Address,
        reward_boost_token: Address,
        reward_boost_feed: Address,
    );

    // Configure rewards for pool. Every second tps of coins
    // being distributed across all liquidity providers
    // after expired_at timestamp distribution ends
    fn set_rewards_config(e: Env, admin: Address, expired_at: u64, tps: u128);

    // Calculate reward token surplus
    fn get_unused_reward(e: Env) -> u128;

    // Return reward token above the configured amount back to the router
    fn return_unused_reward(e: Env, admin: Address) -> u128;

    // Get rewards status for the pool,
    // including amount available for the user
    fn get_rewards_info(e: Env, user: Address) -> Map<Symbol, i128>;

    // Get amount of reward tokens available for the user to claim.
    fn get_user_reward(e: Env, user: Address) -> u128;

    // Checkpoints the reward for the user.
    // Useful when user moves funds by itself to avoid re-entrancy issue.
    // Can be called only by the token contract to notify pool external changes happened.
    fn checkpoint_reward(e: Env, token_contract: Address, user: Address, user_shares: u128);

    // Checkpoints total working balance and the working balance for the user.
    // Useful when user moves funds by itself to avoid re-entrancy issue.
    // Can be called only by the token contract to notify pool external changes happened.
    fn checkpoint_working_balance(
        e: Env,
        token_contract: Address,
        user: Address,
        user_shares: u128,
    );

    // Get total amount of accumulated reward for the pool
    fn get_total_accumulated_reward(e: Env) -> u128;

    // Get total amount of generated plus configured reward for the pool
    fn get_total_configured_reward(e: Env) -> u128;

    // Get total amount of claimed reward for the pool
    fn get_total_claimed_reward(e: Env) -> u128;

    // Claim reward as a user.
    // returns amount of tokens rewarded to the user
    fn claim(e: Env, user: Address) -> u128;
}
