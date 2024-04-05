#![no_std]

use soroban_sdk::{Address, Env, IntoVal, Symbol, Val, Vec};

#[derive(Clone)]
pub struct Events(Env);

impl Events {
    #[inline(always)]
    pub fn env(&self) -> &Env {
        &self.0
    }

    #[inline(always)]
    pub fn new(env: &Env) -> Events {
        Events(env.clone())
    }
}

pub trait LiquidityPoolEvents {
    fn deposit_liquidity(&self, tokens: Vec<Address>, amounts: Vec<u128>, share_amount: u128);

    fn withdraw_liquidity(&self, tokens: Vec<Address>, amounts: Vec<u128>, share_amount: u128);

    fn trade(
        &self,
        user: Address,
        token_in: Address,
        token_out: Address,
        in_amount: u128,
        out_amount: u128,
        fee_amount: u128,
    );
}

impl LiquidityPoolEvents for Events {
    fn deposit_liquidity(&self, tokens: Vec<Address>, amounts: Vec<u128>, share_amount: u128) {
        // topics
        // [
        //   "deposit_liquidity": Symbol, // event identifier
        //   assetA: Address,   // contract addresses identifying asset deposited to the pool
        //   assetB: Address,   // contract addresses identifying asset deposited to the pool (optional)
        //   assetC: Address    // contract addresses identifying asset deposited to the pool (optional)
        // ]
        //
        // body
        // [
        //   stake_amount: i128, // amount of pool tokens received from the pool
        //   amountA: i128,      // amount of tokens deposited to the pool for assetA
        //   amountB: i128       // amount of tokens deposited to the pool for assetB (optional)
        //   amountC: i128       // amount of tokens deposited to the pool for assetC (optional)
        // ]
        let e = self.env();
        let fn_name = Symbol::new(e, "deposit_liquidity");
        let mut body: Vec<Val> = Vec::from_array(e, [(share_amount as i128).into_val(e)]);
        for i in 0..tokens.len().min(3) {
            body.push_back((amounts.get(i).unwrap() as i128).into_val(e));
        }
        match tokens.len() {
            0 => e.events().publish((fn_name,), body),
            1 => e.events().publish((fn_name, tokens.get(0).unwrap()), body),
            2 => e.events().publish(
                (fn_name, tokens.get(0).unwrap(), tokens.get(1).unwrap()),
                body,
            ),
            _ => e.events().publish(
                (
                    fn_name,
                    tokens.get(0).unwrap(),
                    tokens.get(1).unwrap(),
                    tokens.get(2).unwrap(),
                ),
                body,
            ),
        };
    }

    fn withdraw_liquidity(&self, tokens: Vec<Address>, amounts: Vec<u128>, share_amount: u128) {
        // topics
        // [
        //   "withdraw_liquidity": Symbol, // event identifier
        //   assetA: Address,   // contract addresses identifying asset withdrawn from the pool
        //   assetB: Address,   // contract addresses identifying asset withdrawn from the pool (optional)
        //   assetC: Address    // contract addresses identifying asset withdrawn from the pool (optional)
        // ]
        //
        // body
        // [
        //   stake_amount: i128, // amount of pool tokens sent to the pool
        //   amountA: i128,      // amount of tokens withdrawn from the pool for assetA
        //   amountB: i128       // amount of tokens withdrawn from the pool for assetB (optional)
        //   amountC: i128       // amount of tokens withdrawn from the pool for assetC (optional)
        // ]
        let e = self.env();
        let fn_name = Symbol::new(e, "withdraw_liquidity");
        let mut body: Vec<Val> = Vec::from_array(e, [(share_amount as i128).into_val(e)]);
        for i in 0..tokens.len().min(3) {
            body.push_back((amounts.get(i).unwrap() as i128).into_val(e));
        }
        match tokens.len() {
            0 => e.events().publish((fn_name,), body),
            1 => e.events().publish((fn_name, tokens.get(0).unwrap()), body),
            2 => e.events().publish(
                (fn_name, tokens.get(0).unwrap(), tokens.get(1).unwrap()),
                body,
            ),
            _ => e.events().publish(
                (
                    fn_name,
                    tokens.get(0).unwrap(),
                    tokens.get(1).unwrap(),
                    tokens.get(2).unwrap(),
                ),
                body,
            ),
        };
    }

    fn trade(
        &self,
        user: Address,
        token_in: Address,
        token_out: Address,
        in_amount: u128,
        out_amount: u128,
        fee_amount: u128,
    ) {
        // topics
        // [
        //   "trade": Symbol,       // event identifier
        //   sold_asset: Address,   // asset sent to the pool
        //   bought_asset: Address, // asset received from the pool
        //   trader: Address        // address of account/contract that initiated the trade
        // ]
        // body
        // [
        //   sold_amount: i128,   // amount of tokens sent to the pool
        //   bought_amount: i128, // amount of tokens received from the pool
        //   fee: i128            // fee charged by the protocol (asset sent to the pool) - optional
        // ]

        let e = self.env();
        e.events().publish(
            (Symbol::new(e, "trade"), token_in, token_out, user),
            (in_amount as i128, out_amount as i128, fee_amount as i128),
        );
    }
}
