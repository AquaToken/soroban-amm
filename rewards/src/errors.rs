use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum RewardsError {
    PastTimeNotAllowed = 701,
    SameRewardsConfig = 702,
}
