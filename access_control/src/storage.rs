use soroban_sdk::{contracttype, Env};
use utils::bump::bump_instance;

#[derive(Clone)]
#[contracttype]
pub enum DataKey {
    Admin,           // owner - upgrade, set privileged roles
    FutureAdmin,     // pending owner
    Operator,        // rewards admin - configure rewards. legacy name cannot be changed
    OperationsAdmin, // operations admin - add/remove pools, ramp A, set fees, etc
    PauseAdmin,      // pause admin - pause/unpause pools
    EmPauseAdmins,   // emergency pause admin - pause pools in emergency

    TransferOwnershipDeadline,
}

// transfer_ownership_deadline
pub fn get_transfer_ownership_deadline(e: &Env) -> u64 {
    bump_instance(e);
    e.storage()
        .instance()
        .get(&DataKey::TransferOwnershipDeadline)
        .unwrap_or(0)
}

pub fn put_transfer_ownership_deadline(e: &Env, value: &u64) {
    bump_instance(e);
    e.storage()
        .instance()
        .set(&DataKey::TransferOwnershipDeadline, value);
}
