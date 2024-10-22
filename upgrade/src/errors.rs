use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum Error {
    AnotherActionActive = 2906,
    NoActionActive = 2907,
    ActionNotReadyYet = 2908,
}
