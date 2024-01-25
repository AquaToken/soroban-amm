use soroban_sdk::{Env, Vec};

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
            a0 + (a1 - a0) * (now - t0) / (t1 - t0)
        } else {
            a0 - (a0 - a1) * (now - t0) / (t1 - t0)
        }
    } else {
        // when t1 == 0 or block.timestamp >= t1
        a1
    }
}

// xp size = N_COINS
fn get_d(n_coins: u32, xp: Vec<u128>, amp: u128) -> u128 {
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
            d_p = d_p * d / (x1 * n_coins as u128) // If division by 0, this will be borked: only withdrawal will work. And that is good
        }
        d_prev = d;
        d = (ann * s + d_p * n_coins as u128) * d / ((ann - 1) * d + (n_coins as u128 + 1) * d_p);
        // // Equality with the precision of 1
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

fn get_y(n_coins: u32, in_idx: u32, out_idx: u32, x: u128, xp: Vec<u128>, a: u128) -> u128 {
    // x in the input is converted to the same price/precision

    if in_idx == out_idx {
        panic!("same coin")
    } // dev: same coin
      // if !(j >= 0) {
      //     panic!("j below zero")
      // } // dev: j below zero
    if out_idx >= n_coins {
        panic!("j above N_COINS")
    } // dev: j above N_COINS

    // should be unreachable, but good for safety
    // if !(i >= 0) {
    //     panic!("bad arguments")
    // }
    if in_idx >= n_coins {
        panic!("bad arguments")
    }

    let amp = a;
    let d = get_d(xp.len(), xp.clone(), amp);
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
        c = c * d / (x1 * n_coins as u128);
    }
    c = c * d / (ann * n_coins as u128);
    let b = s + d / ann; // - D
    let mut y_prev;
    let mut y = d;
    for _i in 0..255 {
        y_prev = y;
        y = (y * y + c) / (2 * y + b - d);
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

fn get_dy(reserves: Vec<u128>, fee_fraction: u128, a: u128, i: u32, j: u32, dx: u128) -> u128 {
    // dx and dy in c-units
    let xp = reserves.clone();

    let x = xp.get(i).unwrap() + (dx * RATE / PRECISION);
    let y = get_y(reserves.len(), i, j, x, xp.clone(), a);

    if y == 0 {
        // pool is empty
        return 0;
    }

    let dy = (xp.get(j).unwrap() - y - 1) * PRECISION / RATE;
    let fee = fee_fraction * dy / FEE_DENOMINATOR as u128;
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
    get_dy(reserves, fee_fraction, a, in_idx, out_idx, in_amount)
}
