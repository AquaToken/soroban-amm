use crate::storage::DataKey;
use liquidity_pool_config_storage as config_storage;
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{Address, Bytes, Env, IntoVal};

pub(crate) fn deploy_rewards_gauge(
    e: &Env,
    pool: Address,
    operator: Address,
    reward_token: Address,
) -> Address {
    let mut salt = Bytes::new(e);
    salt.append(&pool.clone().to_xdr(e));
    salt.append(&operator.clone().to_xdr(e));
    salt.append(&reward_token.clone().to_xdr(e));

    let gauge_wasm =
        config_storage::operations::get_value_unchecked(e, DataKey::GaugeWASM.into_val(e));
    let contract_id = e
        .deployer()
        .with_current_contract(e.crypto().sha256(&salt).to_bytes())
        .deploy_v2(gauge_wasm, (pool, operator, reward_token));

    contract_id
}
