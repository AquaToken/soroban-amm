use soroban_sdk::{Address, BytesN, Env, Map, Symbol, Val, Vec};

pub trait ManagedLiquidityPool {
    // Initialize pool completely to reduce calculations cost
    fn initialize_all(
        e: Env,
        admin: Address,
        privileged_addrs: (Address, Address, Address, Address, Vec<Address>),
        router: Address,
        token_wasm_hash: BytesN<32>,
        coins: Vec<Address>,
        a: u128,
        fee: u32,
        reward_token: Address,
        plane: Address,
    );
}

pub trait LiquidityPoolInterfaceTrait {
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
        a: u128,
        fee_fraction: u32,
    );

    // The pool swap fee, as an integer with 1e4 precision. 0.01% = 1; 0.3% = 30; 1% = 100;
    fn get_fee_fraction(e: Env) -> u32;

    // Returns the token contract address for the pool share token
    fn share_id(e: Env) -> Address;

    // Returns the total amount of shares
    fn get_total_shares(e: Env) -> u128;

    // Getter for the pool balances array.
    fn get_reserves(e: Env) -> Vec<u128>;

    // Getter for the array of swappable coins within the pool.
    fn get_tokens(e: Env) -> Vec<Address>;

    // Getter for array of tokens decimals in the pool.
    fn get_decimals(e: Env) -> Vec<u32>;

    // Deposit coins into the pool.
    // desired_amounts: List of amounts of coins to deposit
    // Returns amounts deposited and the amount of LP tokens received in exchange for the deposited tokens.
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

    // Withdraw coins from the pool.
    // share_amount: Quantity of LP tokens to burn in the withdrawal
    // min_amounts: Minimum amounts of underlying coins to receive
    // Returns a list of the amounts for each coin that was withdrawn.
    fn withdraw(e: Env, user: Address, share_amount: u128, min_amounts: Vec<u128>) -> Vec<u128>;

    // Get dictionary of basic pool information: type, fee, special parameters if any.
    fn get_info(e: Env) -> Map<Symbol, Val>;
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

    fn set_locked_token(e: Env, admin: Address, locked_token: Address);

    fn set_locker_feed(e: Env, admin: Address, locker_feed: Address);

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

    // Start ramping A to target value in future timestamp
    fn ramp_a(e: Env, admin: Address, future_a: u128, future_time: u64);

    // Stop ramping A
    fn stop_ramp_a(e: Env, admin: Address);

    // Set new fee to be applied in future
    fn commit_new_fee(e: Env, admin: Address, new_fee: u32);

    // Apply committed fee
    fn apply_new_fee(e: Env, admin: Address);

    // Revert committed parameters to current values
    fn revert_new_parameters(e: Env, admin: Address);

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

pub trait LiquidityPoolTrait:
    LiquidityPoolInterfaceTrait
    + UpgradeableContract
    + UpgradeableLPTokenTrait
    + RewardsTrait
    + AdminInterfaceTrait
{
    // The amplification coefficient Amp for the pool. Amp = A*N**(N-1)
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
    // share_amount: Amount of LP tokens to burn in the withdrawal
    // i: Index value of the coin to withdraw
    fn calc_withdraw_one_coin(e: Env, share_amount: u128, i: u32) -> u128;

    // Withdraw a single coin from the pool.
    // share_amount: Amount of LP tokens to burn in the withdrawal
    // i: Index value of the coin to withdraw
    // min_amount: Minimum amount of coin to receive
    // Returns the amount of coin i received.
    fn withdraw_one_coin(
        e: Env,
        user: Address,
        share_amount: u128,
        i: u32,
        min_amount: u128,
    ) -> Vec<u128>;
}
