use crate::constants::FEE_MULTIPLIER;
use soroban_fixed_point_math::SorobanFixedPoint;
use soroban_sdk::{Env, Vec};

// Derivation of the closed-form result for the integral:
//
//   We start with the integral
//       w = ∫[p_min,∞] (Y - X/(P*(1-F))) · (p_min/P)^8 dP,
//   where p_min is defined as
//       p_min = X/(Y*(1-F)).
//
//   The derivation proceeds in the following steps:
//
//   1. **Split the Integral**
//
//      Write w as the difference of two integrals:
//        w = Y ∫[p_min,∞] (p_min/P)^8 dP
//            - (X/(1-F)) ∫[p_min,∞] (p_min/P)^8 · (1/P) dP.
//
//   2. **Evaluate the First Integral**
//
//      Notice that
//        (p_min/P)^8 = p_min^8 · P^(−8).
//
//      Thus, the first integral becomes:
//        I₁ = Y · p_min^8 ∫[p_min,∞] P^(−8) dP.
//
//      For any n > 1, we have:
//        ∫[p_min,∞] P^(−n) dP = 1/[(n−1) · p_min^(n−1)].
//
//      With n = 8:
//        ∫[p_min,∞] P^(−8) dP = 1/(7 · p_min^7).
//
//      Therefore:
//        I₁ = Y · p_min^8 · [1/(7 · p_min^7)] = (Y · p_min)/7.
//
//   3. **Evaluate the Second Integral**
//
//      The second integral is:
//        I₂ = (X/(1-F)) · p_min^8 ∫[p_min,∞] P^(−9) dP.
//
//      For n = 9:
//        ∫[p_min,∞] P^(−9) dP = 1/(8 · p_min^8).
//
//      Thus:
//        I₂ = (X/(1-F)) · p_min^8 · [1/(8 · p_min^8)] = X/(8*(1-F)).
//
//   4. **Combine the Results**
//
//      Now, combine I₁ and I₂ to obtain:
//        w = I₁ - I₂ = (Y · p_min)/7 - X/(8*(1-F)).
//
//   5. **Substitute p_min**
//
//      Replace p_min with its definition, p_min = X/(Y*(1-F)):
//
//        (Y · p_min)/7 = Y/(7) · [X/(Y*(1-F))] = X/(7*(1-F)).
//
//      Thus, the expression for w becomes:
//        w = X/(7*(1-F)) - X/(8*(1-F)).
//
//   6. **Simplify the Expression**
//
//      Factor out X/(1-F):
//        w = (X/(1-F)) · (1/7 - 1/8).
//
//      Compute the difference:
//        1/7 - 1/8 = (8 - 7)/56 = 1/56.
//
//      Therefore, the neat closed-form result is:
//        w = X/(56*(1-F)).
//      which holds for any values of X and Y (with X, Y > 0) as long as the integrand and p_min are defined as above.
pub fn get_liquidity(
    e: &Env,
    fee_fraction: u128,
    reserves: &Vec<u128>,
    in_idx: u32,
    _out_idx: u32,
) -> u128 {
    let x = reserves.get(in_idx).unwrap();
    x.fixed_mul_floor(e, &FEE_MULTIPLIER, &(56 * (FEE_MULTIPLIER - fee_fraction)))
}
