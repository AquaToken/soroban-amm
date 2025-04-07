#![cfg(test)]

use crate::contract::LockerFeedArgs;
use crate::{LockerFeed, LockerFeedClient};
use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{Address, BytesN, Env};

pub fn install_dummy_wasm<'a>(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(file = "../contracts/dummy_contract.wasm");
    e.deployer().upload_contract_wasm(WASM)
}

pub fn create_contract<'a>(
    e: &Env,
    admin: &Address,
    operations_admin: &Address,
    emergency_admin: &Address,
) -> LockerFeedClient<'a> {
    let client = LockerFeedClient::new(
        e,
        &e.register(
            LockerFeed {},
            LockerFeedArgs::__constructor(admin, operations_admin, emergency_admin),
        ),
    );
    client
}

pub(crate) fn jump(e: &Env, time: u64) {
    e.ledger().set(LedgerInfo {
        timestamp: e.ledger().timestamp().saturating_add(time),
        protocol_version: e.ledger().protocol_version(),
        sequence_number: e.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_ttl: 999999,
        min_persistent_entry_ttl: 999999,
        max_entry_ttl: u32::MAX,
    });
}

pub(crate) struct Setup<'a> {
    pub(crate) env: Env,

    pub(crate) admin: Address,
    pub(crate) operations_admin: Address,
    pub(crate) emergency_admin: Address,
    pub(crate) contract: LockerFeedClient<'a>,
}

impl Default for Setup<'_> {
    // Create setup from default config and mint tokens for all users & set rewards config
    fn default() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&env);
        let operations_admin = Address::generate(&env);
        let emergency_admin = Address::generate(&env);
        let contract = create_contract(&env, &admin, &operations_admin, &emergency_admin);

        Setup {
            env,
            admin,
            emergency_admin,
            operations_admin,
            contract,
        }
    }
}
