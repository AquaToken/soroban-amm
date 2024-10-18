use soroban_sdk::{Address, Env, Symbol, Vec};

#[derive(Clone)]
pub struct Events(Env);

impl Events {
    #[inline(always)]
    pub fn env(&self) -> &Env {
        &self.0
    }

    #[inline(always)]
    pub fn new(env: &Env) -> Events {
        Events(env.clone())
    }

    pub fn commit_transfer_ownership(&self, new_owner: Address) {
        self.env().events().publish(
            (Symbol::new(self.env(), "commit_transfer_ownership"),),
            (new_owner,),
        )
    }

    pub fn apply_transfer_ownership(&self, new_owner: Address) {
        self.env().events().publish(
            (Symbol::new(self.env(), "apply_transfer_ownership"),),
            (new_owner,),
        )
    }

    pub fn revert_transfer_ownership(&self) {
        self.env()
            .events()
            .publish((Symbol::new(self.env(), "revert_transfer_ownership"),), ())
    }

    pub fn set_privileged_addrs(
        &self,
        rewards_admin: Address,
        operations_admin: Address,
        pause_admin: Address,
        emergency_pause_admins: Vec<Address>,
    ) {
        self.env().events().publish(
            (Symbol::new(self.env(), "set_privileged_addrs"),),
            (
                rewards_admin,
                operations_admin,
                pause_admin,
                emergency_pause_admins,
            ),
        )
    }
}
