use soroban_sdk::contracterror;

#[contracterror]
#[derive(Copy, Clone)]
#[repr(u32)]
pub enum LiquidityPoolCalculatorError {
    // solution did not converge
    MaxIterationsReached = 209,
}
