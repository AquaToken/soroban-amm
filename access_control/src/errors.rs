use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum AccessControlError {
    AdminNotFound = 101,
    Unauthorized = 102,
    AdminAlreadySet = 103,
}
