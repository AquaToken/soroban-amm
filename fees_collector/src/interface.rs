use soroban_sdk::{Address, Env};

pub trait AdminInterface {
    // Initializes the admin user.
    fn init_admin(e: Env, account: Address);
}
