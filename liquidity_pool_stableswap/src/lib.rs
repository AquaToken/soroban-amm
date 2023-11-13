#![no_std]
#![allow(dead_code)]
mod admin;
mod assertions;
mod constants;
mod contract;
mod pool_constants;
mod pool_interface;
mod rewards;
mod storage;
mod test;
mod token;

#[cfg(all(feature = "tokens_2", feature = "tokens_3"))]
compile_error!("only one feature with tokens number should be specified");

#[cfg(all(feature = "tokens_3", feature = "tokens_4"))]
compile_error!("only one feature with tokens number should be specified");

#[cfg(all(feature = "tokens_2", feature = "tokens_4"))]
compile_error!("only one feature with tokens number should be specified");

#[cfg(all(
    not(feature = "tokens_2"),
    not(feature = "tokens_3"),
    not(feature = "tokens_4")
))]
compile_error!("please specify tokens number feature");

#[cfg(feature = "tokens_2")]
mod pool_2_constants;
#[cfg(feature = "tokens_3")]
mod pool_3_constants;
#[cfg(feature = "tokens_4")]
mod pool_4_constants;

pub use contract::*;
