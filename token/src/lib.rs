#![no_std]
#![allow(dead_code)]

mod allowance;
mod balance;
mod contract;
pub mod errors;
mod metadata;
mod test;

pub use crate::contract::TokenClient;
