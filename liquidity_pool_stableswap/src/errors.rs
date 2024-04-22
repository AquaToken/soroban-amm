use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum LiquidityPoolError {
    AlreadyInitialized = 201,
    PlaneAlreadyInitialized = 202,
    RewardsAlreadyInitialized = 203,
    InvariantDoesNotHold = 204,

    // pool specific validation errors
    PoolKilled = 2901,
    RampTooEarly = 2902,
    RampTimeLessThanMinimum = 2903,
    RampOverMax = 2904,
    RampTooFast = 2905,
    AnotherActionActive = 2906,
    NoActionActive = 2907,
    ActionNotReadyYet = 2908,
}
