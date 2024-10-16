use soroban_sdk::{Address, Env};

pub trait TransferableContract {
    // Commit ownership transfer
    fn commit_transfer_ownership(e: Env, admin: Address, new_admin: Address);

    // Apply committed transfer ownership
    fn apply_transfer_ownership(e: Env, admin: Address);

    // Revert committed ownership transfer
    fn revert_transfer_ownership(e: Env, admin: Address);
}
