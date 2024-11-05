use soroban_sdk::{Address, Env, Symbol};

pub trait TransferableContract {
    // Commit ownership transfer
    fn commit_transfer_ownership(e: Env, admin: Address, role_name: Symbol, new_address: Address);

    // Apply committed transfer ownership
    fn apply_transfer_ownership(e: Env, admin: Address, role_name: Symbol);

    // Revert committed ownership transfer
    fn revert_transfer_ownership(e: Env, admin: Address, role_name: Symbol);

    // Get future address for transfer ownership process
    fn get_future_address(e: Env, role_name: Symbol) -> Address;
}
