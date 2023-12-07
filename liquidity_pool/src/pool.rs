use crate::constants::{FEE_MULTIPLIER, PRICE_PRECISION};

pub fn get_deposit_amounts(
    desired_a: u128,
    min_a: u128,
    desired_b: u128,
    min_b: u128,
    reserve_a: u128,
    reserve_b: u128,
) -> (u128, u128) {
    if reserve_a == 0 && reserve_b == 0 {
        return (desired_a, desired_b);
    }

    let amount_b = desired_a * reserve_b / reserve_a;
    if amount_b <= desired_b {
        if amount_b < min_b {
            panic!("amount_b less than min")
        }
        (desired_a, amount_b)
    } else {
        let amount_a = desired_b * reserve_a / reserve_b;
        if amount_a > desired_a || desired_a < min_a {
            panic!("amount_a invalid")
        }
        (amount_a, desired_b)
    }
}

fn get_min_price(reserve_a: u128, reserve_b: u128, fee_fraction: u128) -> u128 {
    // minimal available price for swap
    PRICE_PRECISION * FEE_MULTIPLIER * reserve_a / (reserve_b * (FEE_MULTIPLIER - fee_fraction))
}

fn price_weight(price: u128, min_price: u128) -> u128 {
    // returns price weighted with exponent (p_min/p)^8
    let mut result = PRICE_PRECISION * min_price / price;
    for _i in 1..8 {
        result = result * min_price / price;
    }
    result
}

fn get_depth(reserve_a: u128, reserve_b: u128, fee_fraction: u128, price: u128) -> u128 {
    let reserve_a_precise = PRICE_PRECISION * reserve_a;
    let reserve_b_precise = PRICE_PRECISION * reserve_b;
    let price_without_fee = price * (FEE_MULTIPLIER - fee_fraction);
    let right_part = reserve_a_precise * FEE_MULTIPLIER / price_without_fee * PRICE_PRECISION;
    // special case on first iteration when right part is slightly more than reserve_b because of rounding
    if right_part <= reserve_b_precise {
        reserve_b_precise - right_part
    } else {
        0
    }
}

pub(crate) fn get_liquidity(reserve_a: u128, reserve_b: u128, fee_fraction: u128) -> u128 {
    // approximation for liquidity integral. iterations number and dp are heuristics
    // liquidity = integral[p_min...inf] ((Y - X/(P(1-F))) * (P_min/P)^8) dP)
    if reserve_a == 0 || reserve_b == 0 {
        return 0;
    }

    let mut result = 0;
    let min_price = get_min_price(reserve_a, reserve_b, fee_fraction);
    let iterations_number = 20;
    // we're calculating sum up to amm_price * 2 as further values doesn't give us much because of weight exponent
    // 20 iterations is optimal for this price curve
    let dp = min_price / iterations_number;
    for i in 0..iterations_number {
        let price = min_price + dp * i;
        let depth = get_depth(reserve_a, reserve_b, fee_fraction, price);
        let weight = price_weight(price, min_price);
        let integration_result = depth * weight / PRICE_PRECISION * dp / PRICE_PRECISION;
        result += integration_result;
    }
    result / PRICE_PRECISION
}
