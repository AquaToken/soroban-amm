use soroban_sdk::{Address, BytesN, Env, Vec};

pub trait ProviderSwapFeeInterface {
    // swap_chained
    // Executes a multi-hop token swap with fee deduction.
    //
    // Arguments:
    //   - e: The Soroban environment.
    //   - user: The user initiating the swap (must be authorized).
    //   - swaps_chain: A vector describing the swap path.
    //   - token_in: The input token address.
    //   - in_amount: The amount of token_in provided by the user.
    //   - out_min: The minimum acceptable output token amount (after fee deduction).
    //   - fee_fraction: The provider fee fraction in basis points (bps).
    //
    // Returns:
    //   - A u128 value representing the net output tokens transferred to the user.
    fn swap_chained(
        e: Env,
        user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address,
        in_amount: u128,
        out_min: u128,
        fee_fraction: u32,
    ) -> u128;

    // swap_chained_strict_receive
    // Executes a multi-hop swap ensuring a specific output amount by adjusting the input and fee.
    //
    // Arguments:
    //   - e: The Soroban environment.
    //   - user: The user initiating the swap (must be authorized).
    //   - swaps_chain: A vector defining the swap path.
    //   - token_in: The input token address.
    //   - out_amount: The exact target output amount.
    //   - in_max: The maximum amount of token_in the user is willing to spend.
    //   - fee_fraction: The provider fee fraction in basis points (bps).
    //
    // Returns:
    //   - A u128 value representing the total input amount (including fees) required.
    fn swap_chained_strict_receive(
        e: Env,
        user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address,
        out_amount: u128,
        in_max: u128,
        fee_fraction: u32,
    ) -> u128;
}
