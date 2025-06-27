use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum Error {
    Unauthorized = 102,
    AlreadyInitialized = 201,
}
