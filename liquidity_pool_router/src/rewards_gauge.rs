use crate::errors::LiquidityPoolRouterError;
use crate::pool_utils::assert_tokens_sorted;
use crate::rewards::get_rewards_manager;
use crate::storage::{get_pool, DataKey};
use liquidity_pool_config_storage as config_storage;
use rewards::storage::RewardTokenStorageTrait;
use soroban_sdk::xdr::ToXdr;
use soroban_sdk::{
    panic_with_error, Address, Bytes, BytesN, Env, IntoVal, Symbol, TryFromVal, Vec,
};
use utils::storage_errors::StorageError;

pub(crate) fn gauge_set_reward_per_day_threshold(
    e: &Env,
    admin: &Address,
    reward_equivalent_day: u128,
) {
    config_storage::operations::set_value(
        e,
        admin,
        DataKey::GaugeRewardMinDayAmt.into_val(e),
        reward_equivalent_day.into_val(e),
    );
}

pub(crate) fn gauge_set_reward_duration_threshold(e: &Env, admin: &Address, duration_seconds: u64) {
    config_storage::operations::set_value(
        e,
        admin,
        DataKey::GaugeRewardMinDuration.into_val(e),
        duration_seconds.into_val(e),
    );
}

pub(crate) fn gauge_get_reward_per_day_threshold(e: &Env) -> u128 {
    match u128::try_from_val(
        e,
        &config_storage::operations::get_value(e, DataKey::GaugeRewardMinDayAmt.into_val(e)),
    ) {
        Ok(value) => value,
        Err(_) => panic_with_error!(e, StorageError::ValueConversionError),
    }
}

pub(crate) fn gauge_get_reward_duration_threshold(e: &Env) -> u64 {
    match u64::try_from_val(
        e,
        &config_storage::operations::get_value(e, DataKey::GaugeRewardMinDuration.into_val(e)),
    ) {
        Ok(value) => value,
        Err(_) => panic_with_error!(e, StorageError::ValueConversionError),
    }
}

pub(crate) fn deploy_rewards_gauge(e: &Env, pool: Address, reward_token: Address) -> Address {
    let mut salt = Bytes::new(e);
    salt.append(&pool.clone().to_xdr(e));
    salt.append(&reward_token.clone().to_xdr(e));

    let gauge_wasm = config_storage::operations::get_value(e, DataKey::GaugeWASM.into_val(e));
    let contract_id = e
        .deployer()
        .with_current_contract(e.crypto().sha256(&salt).to_bytes())
        .deploy_v2(gauge_wasm, (pool, reward_token));

    contract_id
}

pub(crate) fn calculate_equivalent_reward(
    e: &Env,
    token_in: &Address,
    in_amount: u128,
    swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,
) -> u128 {
    let mut last_token_out: Option<Address> = None;
    let mut last_estimate = 0;

    let reward_token = get_rewards_manager(&e).storage().get_reward_token();
    if token_in == &reward_token {
        // swaps chain not required if someone tries to distribute reward token directly.
        // however, in this case, boosts won't work
        return in_amount;
    }

    if swaps_chain.len() == 0 {
        panic_with_error!(&e, LiquidityPoolRouterError::PathIsEmpty);
    }

    for i in 0..swaps_chain.len() {
        let (tokens, pool_index, token_out) = swaps_chain.get(i).unwrap();
        assert_tokens_sorted(&e, &tokens);

        let pool_id = get_pool(&e, &tokens, pool_index);

        let token_in_local;
        let in_amount_local;
        if i == 0 {
            token_in_local = token_in.clone();
            in_amount_local = in_amount;
        } else {
            token_in_local = match last_token_out {
                Some(v) => v,
                None => panic_with_error!(&e, StorageError::ValueNotInitialized),
            };
            in_amount_local = last_estimate;
        }

        // fn estimate_swap(e: Env, in_idx: u32, out_idx: u32, in_amount: u128) -> u128;
        last_estimate = e.invoke_contract(
            &pool_id,
            &Symbol::new(e, "estimate_swap"),
            Vec::from_array(
                &e,
                [
                    tokens
                        .first_index_of(token_in_local.clone())
                        .unwrap()
                        .into_val(e),
                    tokens
                        .first_index_of(token_out.clone())
                        .unwrap()
                        .into_val(e),
                    in_amount_local.into_val(e),
                ],
            ),
        );

        last_token_out = Some(token_out);
    }

    if last_token_out != Some(reward_token) {
        panic_with_error!(&e, LiquidityPoolRouterError::PathMustEndWithRewardToken);
    }

    last_estimate
}
