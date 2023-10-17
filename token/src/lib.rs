#![no_std]
#![allow(dead_code)]

mod admin;
mod allowance;
mod balance;
mod contract;
mod event;
mod metadata;
mod storage_types;
mod test;

pub use crate::contract::TokenClient;
