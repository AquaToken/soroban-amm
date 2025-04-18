use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 102,
    PathIsEmpty = 307,
    OutMinNotSatisfied = 2006,
    InMaxNotSatisfied = 2020,
    FeeFractionTooHigh = 2904,
}
