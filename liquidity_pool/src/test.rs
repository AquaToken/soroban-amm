#![cfg(test)]
extern crate std;

use crate::{token, LiquidityPoolClient};

use crate::assertions::assert_approx_eq_abs;
use soroban_sdk::testutils::{AuthorizedFunction, AuthorizedInvocation, Ledger, LedgerInfo};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, IntoVal, Map, Symbol, Vec};

fn create_token_contract<'a>(e: &Env, admin: &Address) -> token::Client<'a> {
    token::Client::new(e, &e.register_stellar_asset_contract(admin.clone()))
}

fn create_liqpool_contract<'a>(
    e: &Env,
    admin: &Address,
    token_wasm_hash: &BytesN<32>,
    tokens: &Vec<Address>,
    token_reward: &Address,
    fee_fraction: u32,
) -> LiquidityPoolClient<'a> {
    let liqpool = LiquidityPoolClient::new(e, &e.register_contract(None, crate::LiquidityPool {}));
    liqpool.initialize(&admin, token_wasm_hash, tokens, &fee_fraction);
    liqpool.initialize_rewards_config(token_reward, &liqpool.address);
    liqpool
}

fn install_token_wasm(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../token/target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

fn jump(e: &Env, time: u64) {
    e.ledger().set(LedgerInfo {
        timestamp: e.ledger().timestamp().saturating_add(time),
        protocol_version: 20,
        sequence_number: e.ledger().sequence(),
        network_id: Default::default(),
        base_reserve: 10,
        min_temp_entry_expiration: 999999,
        min_persistent_entry_expiration: 999999,
        max_entry_expiration: u32::MAX,
    });
}

#[test]
fn test() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let user1 = Address::random(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        &token_reward.address,
        30,
    );

    token_reward.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let reward_2_tps = 20_0000000_u128;
    let reward_3_tps = 6_0000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );
    token_reward.approve(
        &liqpool.address,
        &liqpool.address,
        &1_000_000_0000000,
        &99999,
    );

    let token_share = token::Client::new(&e, &liqpool.share_id());

    token1.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1), 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1), 1000);
    token1.approve(&user1, &liqpool.address, &1000, &99999);
    token2.approve(&user1, &liqpool.address, &1000, &99999);

    let desired_amounts = Vec::from_array(&e, [100, 100]);
    liqpool.deposit(&user1, &desired_amounts);
    assert_eq!(
        e.auths()[0],
        (
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    liqpool.address.clone(),
                    Symbol::new(&e, "deposit"),
                    Vec::from_array(&e, [user1.to_val(), desired_amounts.to_val()]),
                )),
                sub_invocations: std::vec![],
            }
        )
    );

    assert_eq!(token_reward.balance(&user1), 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1 / 2);
    // 60 seconds more passed. full reward was available though half already claimed
    jump(&e, 60);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1);

    // more rewards added with different configs
    let total_reward_2 = reward_2_tps * 100;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(100),
        &reward_2_tps,
    );
    jump(&e, 105);
    let total_reward_3 = reward_3_tps * 50;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(50),
        &reward_3_tps,
    );
    jump(&e, 500);
    // two rewards available for the user
    assert_eq!(liqpool.claim(&user1), total_reward_2 + total_reward_3);
    assert_eq!(
        token_reward.balance(&user1) as u128,
        total_reward_1 + total_reward_2 + total_reward_3
    );

    assert_eq!(token_share.balance(&user1), 100);
    assert_eq!(token_share.balance(&liqpool.address), 0);
    assert_eq!(token1.balance(&user1), 900);
    assert_eq!(token1.balance(&liqpool.address), 100);
    assert_eq!(token2.balance(&user1), 900);
    assert_eq!(token2.balance(&liqpool.address), 100);

    assert_eq!(liqpool.estimate_swap(&0, &1, &97), 49);
    assert_eq!(liqpool.swap(&user1, &0, &1, &97_u128, &49_u128), 49);
    assert_eq!(
        e.auths()[0],
        (
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    liqpool.address.clone(),
                    Symbol::new(&e, "swap"),
                    (&user1, 0_u32, 1_u32, 97_u128, 49_u128).into_val(&e)
                )),
                sub_invocations: std::vec![],
            }
        )
    );

    assert_eq!(token1.balance(&user1), 803);
    assert_eq!(token1.balance(&liqpool.address), 197);
    assert_eq!(token2.balance(&user1), 949);
    assert_eq!(token2.balance(&liqpool.address), 51);

    token_share.approve(&user1, &liqpool.address, &100, &99999);

    liqpool.withdraw(&user1, &100_u128, &Vec::from_array(&e, [197_u128, 51_u128]));
    assert_eq!(
        e.auths()[0],
        (
            user1.clone(),
            AuthorizedInvocation {
                function: AuthorizedFunction::Contract((
                    liqpool.address.clone(),
                    Symbol::new(&e, "withdraw"),
                    Vec::from_array(
                        &e,
                        [
                            user1.clone().into_val(&e),
                            100_u128.into_val(&e),
                            Vec::from_array(&e, [197_u128, 51_u128]).into_val(&e)
                        ]
                    )
                )),
                sub_invocations: std::vec![],
            }
        )
    );

    jump(&e, 600);
    assert_eq!(liqpool.claim(&user1), 0);
    assert_eq!(
        token_reward.balance(&user1) as u128,
        total_reward_1 + total_reward_2 + total_reward_3
    );

    assert_eq!(token1.balance(&user1), 1000);
    assert_eq!(token2.balance(&user1), 1000);
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token1.balance(&liqpool.address), 0);
    assert_eq!(token2.balance(&liqpool.address), 0);
    assert_eq!(token_share.balance(&liqpool.address), 0);
}

#[test]
fn test_custom_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let user1 = Address::random(&e);

    token1.mint(&user1, &1000000_0000000);
    token2.mint(&user1, &1000000_0000000);

    // we're checking fraction against value required to swap 1 token
    for fee_config in [
        (0, 1_0101011_u128),        // 0%
        (10, 1_0111122_u128),       // 0.1%
        (30, 1_0131405_u128),       // 0.3%
        (100, 1_0203041_u128),      // 1%
        (1000, 1_1223345_u128),     // 10%
        (3000, 1_4430015_u128),     // 30%
        (9900, 101_0101011_u128),   // 99%
        (9999, 10101_0101011_u128), // 99.99% - maximum fee
    ] {
        let liqpool = create_liqpool_contract(
            &e,
            &user1,
            &install_token_wasm(&e),
            &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
            &token_reward.address,
            fee_config.0, // ten percent
        );
        token1.approve(&user1, &liqpool.address, &100000_0000000, &99999);
        token2.approve(&user1, &liqpool.address, &100000_0000000, &99999);
        liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]));
        assert_eq!(liqpool.estimate_swap(&1, &0, &fee_config.1), 1_0000000);
        assert_eq!(liqpool.swap(&user1, &1, &0, &fee_config.1, &0), 1_0000000);
    }
}

#[test]
fn test_simple_ongoing_reward() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let user1 = Address::random(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        &token_reward.address,
        30,
    );

    token_reward.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );
    token_reward.approve(
        &liqpool.address,
        &liqpool.address,
        &1_000_000_0000000,
        &99999,
    );

    token1.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1), 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1), 1000);
    token1.approve(&user1, &liqpool.address, &1000, &99999);
    token2.approve(&user1, &liqpool.address, &1000, &99999);

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]));

    assert_eq!(token_reward.balance(&user1), 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1 / 2);
}

#[test]
fn test_estimate_ongoing_reward() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let user1 = Address::random(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        &token_reward.address,
        30,
    );

    token_reward.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );
    token_reward.approve(
        &liqpool.address,
        &liqpool.address,
        &1_000_000_0000000,
        &99999,
    );

    token1.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1), 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1), 1000);
    token1.approve(&user1, &liqpool.address, &1000, &99999);
    token2.approve(&user1, &liqpool.address, &1000, &99999);

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]));

    assert_eq!(token_reward.balance(&user1), 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);
    assert_eq!(liqpool.get_user_reward(&user1), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&user1) as u128, 0);
}

#[test]
fn test_simple_reward() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let user1 = Address::random(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        &token_reward.address,
        30,
    );

    token1.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1), 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1), 1000);
    token1.approve(&user1, &liqpool.address, &1000, &99999);
    token2.approve(&user1, &liqpool.address, &1000, &99999);

    // 10 seconds. user depositing
    jump(&e, 10);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]));

    // 20 seconds. rewards set up for 60 seconds
    jump(&e, 10);
    token_reward.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );
    token_reward.approve(
        &liqpool.address,
        &liqpool.address,
        &1_000_000_0000000,
        &99999,
    );

    // 90 seconds. rewards ended.
    jump(&e, 70);
    // calling set rewards config to checkpoint. should be removed
    liqpool.set_rewards_config(&user1, &e.ledger().timestamp().saturating_add(60), &0_u128);

    // 100 seconds. user claim reward
    jump(&e, 10);
    assert_eq!(token_reward.balance(&user1), 0);
    // full reward should be available to the user
    assert_eq!(liqpool.claim(&user1), total_reward_1);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1);
}

#[test]
fn test_two_users_rewards() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let user1 = Address::random(&e);
    let user2 = Address::random(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        &token_reward.address,
        30,
    );

    token_reward.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );
    token_reward.approve(
        &liqpool.address,
        &liqpool.address,
        &1_000_000_0000000,
        &99999,
    );

    for user in [&user1, &user2] {
        token1.mint(user, &1000);
        assert_eq!(token1.balance(user), 1000);

        token2.mint(user, &1000);
        assert_eq!(token2.balance(user), 1000);

        token1.approve(user, &liqpool.address, &1000, &99999);
        token2.approve(user, &liqpool.address, &1000, &99999);
    }

    // two users make deposit for equal value. second after 30 seconds after rewards start,
    //  so it gets only 1/4 of total reward
    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]));
    jump(&e, 30);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 2);
    liqpool.deposit(&user2, &Vec::from_array(&e, [100, 100]));
    jump(&e, 100);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 4);
    assert_eq!(liqpool.claim(&user2), total_reward_1 / 4);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1 / 4 * 3);
    assert_eq!(token_reward.balance(&user2) as u128, total_reward_1 / 4);
}

#[test]
fn test_lazy_user_rewards() {
    // first user comes as initial liquidity provider and expects to get maximum reward
    //  second user comes at the end makes huge deposit
    //  first should receive almost full reward

    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let user1 = Address::random(&e);
    let user2 = Address::random(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        &token_reward.address,
        30,
    );

    token_reward.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );
    token_reward.approve(
        &liqpool.address,
        &liqpool.address,
        &1_000_000_0000000,
        &99999,
    );

    for user in [&user1, &user2] {
        token1.mint(user, &1000);
        assert_eq!(token1.balance(user), 1000);

        token2.mint(user, &1000);
        assert_eq!(token2.balance(user), 1000);

        token1.approve(user, &liqpool.address, &1000, &99999);
        token2.approve(user, &liqpool.address, &1000, &99999);
    }

    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]));
    jump(&e, 59);
    liqpool.deposit(&user2, &Vec::from_array(&e, [1000, 1000]));
    jump(&e, 100);
    let user1_claim = liqpool.claim(&user1);
    let user2_claim = liqpool.claim(&user2);
    assert_approx_eq_abs(
        user1_claim,
        total_reward_1 * 59 / 60 + total_reward_1 / 1100 * 100 / 60,
        1000,
    );
    assert_approx_eq_abs(user2_claim, total_reward_1 / 1100 * 1000 / 60, 1000);
    assert_approx_eq_abs(token_reward.balance(&user1) as u128, user1_claim, 1000);
    assert_approx_eq_abs(token_reward.balance(&user2) as u128, user2_claim, 1000);
    assert_approx_eq_abs(user1_claim + user2_claim, total_reward_1, 1000);
}

#[test]
fn test_deposit_ddos() {
    // first user comes as initial liquidity provider
    //  many users come
    //  user does withdraw

    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::random(&e);
    let mut admin2 = Address::random(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let admin = Address::random(&e);
    let users_to_simulate = 300;

    let liqpool = create_liqpool_contract(
        &e,
        &admin,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        &token_reward.address,
        30,
    );

    token_reward.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    liqpool.set_rewards_config(
        &admin,
        &e.ledger().timestamp().saturating_add(users_to_simulate * 2),
        &reward_1_tps,
    );
    token_reward.approve(
        &liqpool.address,
        &liqpool.address,
        &1_000_000_0000000,
        &99999,
    );

    let mut users = Map::new(&e);
    for i in 0..users_to_simulate {
        let user = Address::random(&e);
        users.set(i, user.clone());

        token1.mint(&user, &1000);
        assert_eq!(token1.balance(&user), 1000);

        token2.mint(&user, &1000);
        assert_eq!(token2.balance(&user), 1000);

        token1.approve(&user, &liqpool.address, &1000, &99999);
        token2.approve(&user, &liqpool.address, &1000, &99999);

        jump(&e, 1);
        liqpool.deposit(&user, &Vec::from_array(&e, [1000, 1000]));
    }

    jump(&e, 100);
    e.budget().reset_default();
    e.budget().reset_tracker();
    // println!("claim start");
    let user1_claim = liqpool.claim(&users.get(0).unwrap());
    // println!("claim end");
    e.budget().print();
    assert!(
        user1_claim > 0,
        "assertion failed: `(left < right)` \
         (left: `{:?}`, right: `{:?}``)",
        user1_claim,
        0,
    );
}
