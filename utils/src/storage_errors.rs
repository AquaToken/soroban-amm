use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum StorageError {
    AlreadyInitialized = 201,
    ValueNotInitialized = 501,
    ValueMissing = 502,
}
