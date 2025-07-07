#![no_std]

mod constants;
mod contract;
mod errors;
mod gauge;
mod interface;
mod storage;
mod test;
mod test_permissions;
mod testutils;

pub use crate::contract::{RewardsGauge, RewardsGaugeClient};
