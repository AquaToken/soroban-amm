use soroban_sdk::{Address, Env, Map, Symbol};

pub trait RewardsGaugeInterface {
    // Admin functions
    // Adds a new rewards gauge to the contract.
    fn gauge_add(e: Env, admin: Address, gauge_address: Address);

    // Removes a rewards gauge from the contract by reward token address.
    fn gauge_remove(e: Env, admin: Address, reward_token: Address);

    // Schedules a reward for a specific gauge.
    fn gauge_schedule_reward(
        e: Env,
        router: Address,
        distributor: Address,
        gauge: Address,
        start_at: Option<u64>,
        duration: u64,
        tps: u128,
    );

    // Kills the gauges claim functionality, preventing users from claiming rewards.
    fn kill_gauges_claim(e: Env, admin: Address);

    // Restores the gauges claim functionality, allowing users to claim rewards again.
    fn unkill_gauges_claim(e: Env, admin: Address);

    // Public functions
    // Lists all reward gauges.
    fn get_gauges(e: Env) -> Map<Address, Address>;

    // Claims rewards for a user across all gauges.
    fn gauges_claim(e: Env, user: Address) -> Map<Address, u128>;

    // Rewards info getter
    fn gauges_get_reward_info(e: Env, user: Address) -> Map<Address, Map<Symbol, i128>>;
}
