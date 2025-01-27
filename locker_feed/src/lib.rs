#![no_std]

mod contract;
mod errors;
mod interface;
mod test_permissions;
mod testutils;

pub use crate::contract::{LockerFeed, LockerFeedClient};
