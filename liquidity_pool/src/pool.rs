use soroban_sdk::{Env, U256};

pub fn get_deposit_amounts(
    e: &Env,
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

    let amount_b = U256::from_u128(e, desired_a)
        .mul(&U256::from_u128(e, reserve_b).div(&U256::from_u128(e, reserve_a)))
        .to_u128()
        .expect("math overflow");
    if amount_b <= desired_b {
        if amount_b < min_b {
            panic!("amount_b less than min")
        }
        (desired_a, amount_b)
    } else {
        let amount_a = U256::from_u128(e, desired_b)
            .mul(&U256::from_u128(e, reserve_a).div(&U256::from_u128(e, reserve_b)))
            .to_u128()
            .expect("math overflow");
        if amount_a > desired_a || desired_a < min_a {
            panic!("amount_a invalid")
        }
        (amount_a, desired_b)
    }
}
