#![no_std]

mod constants;
mod contract;
mod errors;
mod gauge;
mod storage;
mod test;
mod testutils;
mod token_share;
mod interface;
// mod interface;
// mod test_permissions;
// mod testutils;

pub use crate::contract::{RewardsGauge, RewardsGaugeClient};
