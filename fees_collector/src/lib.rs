#![no_std]

mod contract;
mod interface;
mod storage;
mod test;

pub use crate::contract::{FeesCollector, FeesCollectorClient};
