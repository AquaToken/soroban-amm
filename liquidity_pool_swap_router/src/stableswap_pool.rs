use liquidity_pool_validation_errors::LiquidityPoolValidationError;
use soroban_sdk::{panic_with_error, Env, Vec};
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::{Env, Vec, U256};

const RATE: u128 = 1_0000000;
const PRECISION: u128 = 1_0000000;
const FEE_DENOMINATOR: u32 = 10000; // 0.01% = 0.0001 = 1 / 10000

fn a(e: &Env, initial_a: u128, initial_a_time: u128, future_a: u128, future_a_time: u128) -> u128 {
    // Handle ramping A up or down
    let t1 = future_a_time;
    let a1 = future_a;
    let now = e.ledger().timestamp() as u128;

    if now < t1 {
        let a0 = initial_a;
        let t0 = initial_a_time;
        // Expressions in u128 cannot have negative numbers, thus "if"
        if a1 > a0 {
            a0 + (a1 - a0).fixed_mul_floor(e, now - t0, t1 - t0)
        } else {
            a0 - (a0 - a1).fixed_mul_floor(e, now - t0, t1 - t0)
        }
    } else {
        // when t1 == 0 or block.timestamp >= t1
        a1
    }
}

// xp size = N_COINS
fn get_d(e: &Env, n_coins: u32, xp: Vec<u128>, amp: u128) -> u128 {
    let mut s = 0;
    for x in xp.clone() {
        s += x;
    }
    if s == 0 {
        return 0;
    }

    let mut d_prev;
    let mut d = s;
    let ann = amp * n_coins as u128;
    for _i in 0..255 {
        let mut d_p = d;
        for x1 in xp.clone() {
            d_p = d_p.fixed_mul_floor(e, d, x1 * n_coins as u128); // If division by 0, this will be borked: only withdrawal will work. And that is good
        }
        d_prev = d;
        d = (ann * s + d_p * n_coins as u128).fixed_mul_floor(
            e,
            d,
            (ann - 1) * d + (n_coins as u128 + 1) * d_p,
        );
        // Equality with the precision of 1
        if d > d_prev {
            if d - d_prev <= 1 {
                break;
            }
        } else if d_prev - d <= 1 {
            break;
        }
    }
    d
}

fn get_y(
    e: &Env,
    n_coins: u32,
    in_idx: u32,
    out_idx: u32,
    x: u128,
    xp: Vec<u128>,
    a: u128,
) -> u128 {
    // x in the input is converted to the same price/precision

    if in_idx == out_idx {
        panic_with_error!(e, LiquidityPoolValidationError::AllCoinsRequired);
    }
    if out_idx >= n_coins {
        panic_with_error!(e, LiquidityPoolValidationError::OutTokenOutOfBounds);
    }

    if in_idx >= n_coins {
        panic_with_error!(e, LiquidityPoolValidationError::InTokenOutOfBounds);
    }

    let amp = a;
    let d = get_d(e, xp.len(), xp.clone(), amp);
    let mut c = d;
    let mut s = 0;
    let ann = amp * n_coins as u128;

    let mut x1;
    for i in 0..n_coins {
        if i == in_idx {
            x1 = x;
        } else if i != out_idx {
            x1 = xp.get(i).unwrap();
        } else {
            continue;
        }
        s += x1;
        c = c.fixed_mul_floor(e, d, x1 * n_coins as u128);
    }
    let c_256 = U256::from_u128(&e, c)
        .mul(&U256::from_u128(&e, d))
        .div(&U256::from_u128(&e, ann * n_coins as u128));
    let b = s + d / ann; // - D
    let mut y_prev;
    let mut y = d;
    for _i in 0..255 {
        y_prev = y;
        let y_256 = U256::from_u128(&e, y);
        y = y_256
            .mul(&y_256)
            .add(&c_256)
            .div(&U256::from_u128(&e, 2 * y + b - d))
            .to_u128()
            .expect("math overflow");

        // Equality with the precision of 1
        if y > y_prev {
            if y - y_prev <= 1 {
                break;
            }
        } else if y_prev - y <= 1 {
            break;
        }
    }
    y
}

fn get_dy(
    e: &Env,
    reserves: Vec<u128>,
    fee_fraction: u128,
    a: u128,
    i: u32,
    j: u32,
    dx: u128,
) -> u128 {
    // dx and dy in c-units
    let xp = reserves.clone();

    let x = xp.get(i).unwrap() + (dx.fixed_mul_floor(e, RATE, PRECISION));
    let y = get_y(e, reserves.len(), i, j, x, xp.clone(), a);

    if y == 0 {
        // pool is empty
        return 0;
    }

    let dy = (xp.get(j).unwrap() - y - 1).fixed_mul_floor(e, PRECISION, RATE);
    let fee = fee_fraction.fixed_mul_floor(e, dy, FEE_DENOMINATOR as u128);
    dy - fee
}

pub(crate) fn estimate_swap(
    e: &Env,
    fee_fraction: u128,
    initial_a: u128,
    initial_a_time: u128,
    future_a: u128,
    future_a_time: u128,
    reserves: Vec<u128>,
    in_idx: u32,
    out_idx: u32,
    in_amount: u128,
) -> u128 {
    let a = a(e, initial_a, initial_a_time, future_a, future_a_time);
    get_dy(e, reserves, fee_fraction, a, in_idx, out_idx, in_amount)
}
