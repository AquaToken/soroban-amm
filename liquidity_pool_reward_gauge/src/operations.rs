use crate::constants::MAX_GAUGES;
use crate::errors::GaugeError;
use crate::events::GaugeEvents;
use crate::storage::{
    get_is_killed_gauges_claim, get_reward_gauges, set_is_killed_gauges_claim, set_reward_gauges,
    RewardConfig,
};
use soroban_sdk::{panic_with_error, Address, BytesN, Env, IntoVal, Map, Symbol, Val, Vec};

pub fn add(e: &Env, gauge_address: Address) {
    let mut configured_gauges = get_reward_gauges(e);
    if configured_gauges.contains(&gauge_address) {
        panic_with_error!(e, GaugeError::GaugeAlreadyExists);
    }
    configured_gauges.push_back(gauge_address.clone());
    if configured_gauges.len() > MAX_GAUGES {
        panic_with_error!(e, GaugeError::GaugesOverMax);
    }
    set_reward_gauges(e, &configured_gauges);
    GaugeEvents::new(e).add(gauge_address);
}

pub fn remove(e: &Env, gauge_address: Address) {
    let mut configured_gauges = get_reward_gauges(e);
    if let Some(index) = configured_gauges.first_index_of(&gauge_address) {
        configured_gauges.remove(index);
        set_reward_gauges(e, &configured_gauges);
    } else {
        panic_with_error!(e, GaugeError::GaugeNotFound);
    }
    GaugeEvents::new(e).remove(gauge_address);
}

pub fn upgrade(e: &Env, new_wasm: &BytesN<32>) {
    for gauge in get_reward_gauges(e).iter() {
        e.invoke_contract::<Val>(
            &gauge,
            &Symbol::new(e, "upgrade"),
            Vec::from_array(
                e,
                [e.current_contract_address().to_val(), new_wasm.into_val(e)],
            ),
        );
    }
}

pub fn kill_claim(e: &Env) {
    set_is_killed_gauges_claim(e, &true);
    GaugeEvents::new(e).kill_claim();
}

pub fn unkill_claim(e: &Env) {
    set_is_killed_gauges_claim(e, &false);
    GaugeEvents::new(e).unkill_claim();
}

pub fn list(e: &Env) -> Vec<Address> {
    get_reward_gauges(e)
}

pub fn checkpoint_user(e: &Env, user: &Address, working_balance: u128, working_supply: u128) {
    for gauge in get_reward_gauges(e).iter() {
        e.invoke_contract::<Val>(
            &gauge,
            &Symbol::new(e, "checkpoint_user"),
            Vec::from_array(
                e,
                [
                    e.current_contract_address().to_val(),
                    user.to_val(),
                    working_balance.into_val(e),
                    working_supply.into_val(e),
                ],
            ),
        );
    }
}

pub fn claim(
    e: &Env,
    user: &Address,
    working_balance: u128,
    working_supply: u128,
) -> Map<Address, u128> {
    if get_is_killed_gauges_claim(e) {
        panic_with_error!(e, GaugeError::ClaimKilled);
    }

    let mut result = Map::new(e);
    for gauge in get_reward_gauges(e).iter() {
        let reward_token = crate::token::get_reward_token(e, &gauge);
        let claimed_amount = e.invoke_contract(
            &gauge,
            &Symbol::new(e, "claim"),
            Vec::from_array(
                e,
                [
                    e.current_contract_address().to_val(),
                    user.to_val(),
                    working_balance.into_val(e),
                    working_supply.into_val(e),
                ],
            ),
        );
        GaugeEvents::new(e).claim(user.clone(), reward_token.clone(), claimed_amount);
        result.set(reward_token, claimed_amount);
    }
    result
}

pub fn get_rewards_info(
    e: &Env,
    user: &Address,
    working_balance: u128,
    working_supply: u128,
) -> Map<Address, Map<Symbol, i128>> {
    let mut result = Map::new(e);
    for gauge in get_reward_gauges(e).iter() {
        let reward_config: RewardConfig =
            e.invoke_contract(&gauge, &Symbol::new(e, "get_reward_config"), Vec::new(e));
        let user_reward: u128 = e.invoke_contract(
            &gauge,
            &Symbol::new(e, "get_user_reward"),
            Vec::from_array(
                e,
                [
                    e.current_contract_address().to_val(),
                    user.to_val(),
                    working_balance.into_val(e),
                    working_supply.into_val(e),
                ],
            ),
        );

        result.set(
            crate::token::get_reward_token(e, &gauge),
            Map::from_array(
                e,
                [
                    ("user_reward".into_val(e), (user_reward as i128).into_val(e)),
                    ("tps".into_val(e), (reward_config.tps as i128).into_val(e)),
                    (
                        "expired_at".into_val(e),
                        (reward_config.expired_at as i128).into_val(e),
                    ),
                ],
            ),
        );
    }
    result
}

pub fn schedule_rewards_config(
    e: &Env,
    gauge: Address,
    operator: Address,
    start_at: Option<u64>,
    duration: u64,
    tps: u128,
    working_supply: u128,
) {
    e.invoke_contract::<()>(
        &gauge,
        &Symbol::new(e, "schedule_rewards_config"),
        Vec::from_array(
            e,
            [
                e.current_contract_address().to_val(),
                operator.to_val(),
                start_at.into_val(e),
                duration.into_val(e),
                tps.into_val(e),
                working_supply.into_val(e),
            ],
        ),
    );
    let start_at = start_at.unwrap_or(e.ledger().timestamp());
    GaugeEvents::new(e).schedule_reward(
        crate::token::get_reward_token(e, &gauge),
        start_at,
        start_at + duration,
        tps,
    )
}
