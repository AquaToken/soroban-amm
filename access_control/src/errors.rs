use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum AccessControlError {
    AdminNotFound = 101,
    UserNotAdmin = 102,
    AdminAlreadySet = 103,
}
