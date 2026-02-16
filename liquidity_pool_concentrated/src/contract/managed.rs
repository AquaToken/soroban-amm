use super::*;

#[contractimpl]
impl ManagedLiquidityPool for ConcentratedLiquidityPool {
    // Full initialization in one call: pool params + rewards config + plane.
    // Called by router during pool creation to avoid multiple cross-contract calls.
    fn initialize_all(
        e: Env,
        admin: Address,
        privileged_addrs: (Address, Address, Address, Address, Vec<Address>, Address),
        router: Address,
        tokens: Vec<Address>,
        fee: u32,
        tick_spacing: i32,
        reward_config: (Address, Address, Address),
        plane: Address,
    ) {
        let (reward_token, reward_boost_token, reward_boost_feed) = reward_config;
        Self::init_pools_plane(e.clone(), plane);
        Self::initialize(
            e.clone(),
            admin,
            privileged_addrs,
            router,
            tokens,
            fee,
            tick_spacing,
        );
        Self::initialize_boost_config(e.clone(), reward_boost_token, reward_boost_feed);
        Self::initialize_rewards_config(e, reward_token);
    }
}
