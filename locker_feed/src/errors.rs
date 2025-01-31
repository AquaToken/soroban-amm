use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum FeedError {
    AlreadyInitialized = 201,
}
