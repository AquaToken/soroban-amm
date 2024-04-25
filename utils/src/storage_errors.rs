use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum StorageError {
    ValueNotInitialized = 501,
    ValueMissing = 502,
}
