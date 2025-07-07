#![no_std]

mod contract;
mod test;
mod test_permissions;
mod testutils;

pub use crate::contract::{ConfigStorage, ConfigStorageClient};
