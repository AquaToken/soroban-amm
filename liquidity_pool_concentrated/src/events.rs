use soroban_sdk::{contractevent, Address, Env};

// topics
// [
//   "claim_fees": Symbol,      // event identifier
//   owner: Address             // address of account/contract that initiated the claim
//   token0: Address,           // Address of token0
//   token1: Address,           // Address of token1
// ]
// body
// [
//   amount0: i128,              // amount of token0 claimed
//   amount1: i128,              // amount of token1 claimed
// ]
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
