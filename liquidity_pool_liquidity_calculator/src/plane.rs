mod pool_plane_client {
    soroban_sdk::contractimport!(file = "../contracts/soroban_liquidity_pool_plane_contract.wasm");
}

pub use crate::plane::pool_plane_client::Client as PoolPlaneClient;

use soroban_sdk::Vec;

pub(crate) fn parse_standard_data(init_args: Vec<u128>, reserves: Vec<u128>) -> (u128, Vec<u128>) {
    (init_args.get(0).unwrap(), reserves)
}

pub struct ConcentratedPoolData {
    pub(crate) fee: u128,
    pub(crate) tick_spacing: i32,
    pub(crate) steps: u32,
    pub(crate) full_range_reserve0: u128,
    pub(crate) full_range_reserve1: u128,
    pub(crate) reserves: Vec<u128>,
}

impl ConcentratedPoolData {
    const RESERVES_PREFIX_SIZE: u32 = 4;

    pub(crate) fn reserve(&self, idx: u32) -> u128 {
        self.reserves.get(idx).unwrap_or(0)
    }

    pub(crate) fn full_range_in(&self, in_idx: u32) -> u128 {
        if in_idx == 0 {
            self.full_range_reserve0
        } else {
            self.full_range_reserve1
        }
    }

    pub(crate) fn full_range_out(&self, out_idx: u32) -> u128 {
        if out_idx == 0 {
            self.full_range_reserve0
        } else {
            self.full_range_reserve1
        }
    }

    pub(crate) fn step_0_to_1(&self, step: u32) -> (u128, u128) {
        self.step_pair(step, Self::RESERVES_PREFIX_SIZE)
    }

    pub(crate) fn step_1_to_0(&self, step: u32) -> (u128, u128) {
        self.step_pair(
            step,
            Self::RESERVES_PREFIX_SIZE + self.steps.saturating_mul(2),
        )
    }

    fn step_pair(&self, step: u32, base_offset: u32) -> (u128, u128) {
        if step >= self.steps {
            return (0, 0);
        }

        let in_idx = base_offset.saturating_add(step.saturating_mul(2));
        let out_idx = in_idx.saturating_add(1);
        (
            self.reserves.get(in_idx).unwrap_or(0),
            self.reserves.get(out_idx).unwrap_or(0),
        )
    }
}

pub(crate) fn parse_concentrated_data(
    init_args: Vec<u128>,
    reserves: Vec<u128>,
) -> ConcentratedPoolData {
    // The only supported format:
    // init_args: [version=1, fee, tick_spacing, steps]
    // reserves: [reserve0, reserve1, full_range_reserve0, full_range_reserve1, ...steps]
    let version = init_args.get(0).unwrap();
    if version != 1 {
        panic!("invalid concentrated plane data version");
    }

    let fee = init_args.get(1).unwrap();
    let mut tick_spacing = init_args.get(2).unwrap();
    if tick_spacing > i32::MAX as u128 {
        tick_spacing = i32::MAX as u128;
    }

    let steps_u128 = init_args.get(3).unwrap();
    let steps = if steps_u128 > u32::MAX as u128 {
        u32::MAX
    } else {
        steps_u128 as u32
    };

    ConcentratedPoolData {
        fee,
        tick_spacing: tick_spacing as i32,
        steps,
        full_range_reserve0: reserves.get(2).unwrap_or(0),
        full_range_reserve1: reserves.get(3).unwrap_or(0),
        reserves,
    }
}

pub struct StableSwapPoolData {
    pub(crate) fee: u128,
    pub(crate) initial_a: u128,
    pub(crate) initial_a_time: u128,
    pub(crate) future_a: u128,
    pub(crate) future_a_time: u128,
    pub(crate) xp: Vec<u128>,
}

// * `init_args`: [fee, initial_a, initial_a_time, future_a, future_a_time]
// * `xp`: pool balances list in normalized form
pub(crate) fn parse_stableswap_data(init_args: Vec<u128>, xp: Vec<u128>) -> StableSwapPoolData {
    StableSwapPoolData {
        fee: init_args.get(0).unwrap(),
        initial_a: init_args.get(1).unwrap(),
        initial_a_time: init_args.get(2).unwrap(),
        future_a: init_args.get(3).unwrap(),
        future_a_time: init_args.get(4).unwrap(),
        xp,
    }
}
