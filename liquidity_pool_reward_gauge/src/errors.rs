use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum GaugeError {
    ClaimKilled = 207,
    GaugesOverMax = 305,
    GaugeNotFound = 404,
}
