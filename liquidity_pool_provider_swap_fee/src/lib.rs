#![no_std]

mod constants;
mod contract;
mod errors;
mod events;
mod interface;
mod storage;
mod test;
mod test_permissions;
mod testutils;

pub use crate::contract::{ProviderSwapFeeCollector, ProviderSwapFeeCollectorClient};
