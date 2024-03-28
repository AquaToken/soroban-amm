#![no_std]

use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq, PartialOrd, Ord)]
#[repr(u32)]
pub enum LiquidityPoolValidationError {
    WrongInputVecSize = 2001,
    TokensNotSorted = 2002,
    FeeOutOfBounds = 2003,
    AllCoinsRequired = 2004,
    InMinNotSatisfied = 2005,
    OutMinNotSatisfied = 2006,
    CannotSwapSameToken = 2007,
    InTokenOutOfBounds = 2008,
    OutTokenOutOfBounds = 2009,
    EmptyPool = 2010,
    InvalidDepositAmount = 2011,
    AdminFeeOutOfBounds = 2012,
    UnknownPoolType = 2013,
    ZeroSharesBurned = 2014,
    TooManySharesBurned = 2015,
    PastTimeNotAllowed = 2016,
    CannotComparePools = 2017,
}
