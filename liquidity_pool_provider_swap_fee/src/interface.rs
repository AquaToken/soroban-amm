use soroban_sdk::{Address, BytesN, Env, Vec};

pub trait ProviderSwapFeeInterface {
    // Executes a chain of token swaps to exchange an input token for an output token.
    //
    // # Arguments
    //
    // * `user` - The address of the user executing the swaps.
    // * `swaps_chain` - The series of swaps to be executed. Each swap is represented by a tuple containing:
    //   - A vector of token addresses liquidity pool belongs to
    //   - Pool index hash
    //   - The token to obtain
    // * `token_in` - The address of the input token to be swapped.
    // * `in_amount` - The amount of the input token to be swapped.
    // * `out_min` - The minimum amount of the output token to be received.
    //
    // # Returns
    //
    // The amount of the output token received after all swaps have been executed.
    fn swap_chained(
        e: Env,
        user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address,
        in_amount: u128,
        out_min: u128,
    ) -> u128;

    // Executes a chain of token swaps to exchange an input token for an output token.
    //
    // # Arguments
    //
    // * `user` - The address of the user executing the swaps.
    // * `swaps_chain` - The series of swaps to be executed. Each swap is represented by a tuple containing:
    //   - A vector of token addresses liquidity pool belongs to
    //   - Pool index hash
    //   - The token to obtain
    // * `token_in` - The address of the input token to be swapped.
    // * `out_amount` - The amount of the output token to be received.
    // * `in_max` - The max amount of the input token to spend.
    //
    // # Returns
    //
    // The amount of the input token spent after all swaps have been executed.
    fn swap_chained_strict_receive(
        e: Env,
        user: Address,
        swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
        token_in: Address,
        out_amount: u128,
        in_max: u128,
    ) -> u128;
}
