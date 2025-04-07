#![cfg(test)]

use crate::FeesCollectorClient;
use soroban_sdk::testutils::{Address as _, Ledger, LedgerInfo};
use soroban_sdk::{Address, BytesN, Env, Symbol};

pub fn install_dummy_wasm<'a>(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(file = "../contracts/dummy_contract.wasm");
    e.deployer().upload_contract_wasm(WASM)
}

pub fn create_contract<'a>(e: &Env) -> FeesCollectorClient<'a> {
    let client = FeesCollectorClient::new(e, &e.register(crate::contract::FeesCollector {}, ()));
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
    pub(crate) emergency_admin: Address,
    pub(crate) collector: FeesCollectorClient<'a>,
}

impl Default for Setup<'_> {
    // Create setup from default config and mint tokens for all users & set rewards config
    fn default() -> Self {
        let env = Env::default();
        env.mock_all_auths();
        env.cost_estimate().budget().reset_unlimited();

        let admin = Address::generate(&env);
        let collector = create_contract(&env);
        collector.init_admin(&admin);

        let emergency_admin = Address::generate(&env);
        collector.commit_transfer_ownership(
            &admin,
            &Symbol::new(&env, "EmergencyAdmin"),
            &emergency_admin,
        );
        collector.apply_transfer_ownership(&admin, &Symbol::new(&env, "EmergencyAdmin"));

        Setup {
            env,
            admin,
            emergency_admin,
            collector,
        }
    }
}
