use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 102,
    AlreadyInitialized = 201,
    InvalidConfig = 3000,
    ConfigNotExpiredYet = 3001,
    StartNotInFuture = 3002,
    StartTooEarly = 3003,
}
