use soroban_sdk::{contractevent, Address, Env};

#[contractevent(data_format = "vec")]
pub struct ClaimFees {
    #[topic]
    pub owner: Address,
    #[topic]
    pub token0: Address,
    #[topic]
    pub token1: Address,
    pub amount0: i128,
    pub amount1: i128,
}
