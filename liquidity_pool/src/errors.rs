use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum LiquidityPoolError {
    AlreadyInitialized = 201,
    PlaneAlreadyInitialized = 202,
    RewardsAlreadyInitialized = 203,
    InvariantDoesNotHold = 204,
    PoolDepositKilled = 205,
    PoolSwapKilled = 206,
    PoolClaimKilled = 207,
    FutureShareIdNotSet = 208,
    // pool specific validation errors
}
