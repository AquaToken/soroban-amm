use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum TokenError {
    AlreadyInitialized = 601,
    InsufficientBalance = 602,
    InsufficientAllowance = 603,
    NegativeNotAllowed = 604,
    DecimalTooLarge = 605,
    PastTimeNotAllowed = 606,
}
