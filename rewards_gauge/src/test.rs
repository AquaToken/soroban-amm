#![cfg(test)]
extern crate std;

use crate::testutils::{create_contract, Setup};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::Address;
use utils::test_utils::{assert_approx_eq_abs, jump, time_warp};

#[test]
fn test_simple_reward() {
    let setup = Setup::with_mocked_pool();

    let user1 = Address::generate(&setup.env);
    let user2 = Address::generate(&setup.env);

    let reward_token_sac = StellarAssetClient::new(&setup.env, &setup.reward_token.address);
    reward_token_sac.mint(&setup.operator, &1_000_000_0000000);

    let mut total_shares = 0;

    jump(&setup.env, 1);
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &setup.operator, &0, &total_shares);
    total_shares = 1000_0000000;
    // inv(1) = 0
    jump(&setup.env, 1);

    assert_eq!(setup.contract.get_reward_config().tps, 0,);

    // rewards are scheduled for 2-102 seconds
    setup.contract.schedule_rewards_config(
        &setup.pool_address,
        &setup.operator,
        &None,
        &100,
        &1_0000000,
        &total_shares,
    );
    // inv(2) = 0. no rewards yet generated

    jump(&setup.env, 10);
    assert_eq!(setup.contract.get_reward_config().tps, 1_0000000,);

    // first user deposits after 10 seconds
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user1, &0, &total_shares);
    total_shares += 1000_0000000;
    // inv(12) = 1 * 10 / 100000

    jump(&setup.env, 10);

    // second user deposits
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user2, &0, &total_shares);
    total_shares += 1000_0000000;
    // inv(22) = inv(12) + 1 * 10 / 101000

    jump(&setup.env, 10);

    // first user withdraws 20 seconds after deposit
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user1, &1000_0000000, &total_shares);
    total_shares -= 1000_0000000;

    jump(&setup.env, 100);

    // both claim rewards after reward period ends
    let user1_claim = setup
        .contract
        .claim(&setup.pool_address, &user1, &0, &total_shares);
    let user2_claim =
        setup
            .contract
            .claim(&setup.pool_address, &user2, &1000_0000000, &total_shares);
    // tps = 1
    // user1
    //  share1 = 1 / 2
    //  duration1 = 10
    //  reward1 = 1 * 10 / 2 = 5
    //  share2 = 1 / 3
    //  duration2 = 10
    //  reward2 = 1 * 10 / 3 = 3.(3)
    //  reward = 5 + 3.(3) = 8.(3)
    // user2
    //  share1 = 1 / 3
    //  duration1 = 10
    //  reward1 = 1 * 10 / 3 = 3.(3)
    //  share2 = 1 / 2
    //  duration2 = 70
    //  reward2 = 1 * 70 / 2 = 35
    //  reward = 3.(3) + 35 = 38.(3)
    assert_eq!(user1_claim, 83333333);
    assert_eq!(user2_claim, 383333333);
}

#[test]
fn test_retroactive_reward() {
    let setup = Setup::default();
    let env = setup.env;
    let reward_token_sac = StellarAssetClient::new(&env, &setup.reward_token.address);
    reward_token_sac.mint(&setup.operator, &1_000_000_0000000);

    let user1 = Address::generate(&env);
    let user2 = Address::generate(&env);

    // 10 seconds. first user depositing before gauge set up
    jump(&env, 10);
    let mut total_shares = 100;

    // 20 seconds. gauge set up
    jump(&env, 10);
    let gauge = create_contract(
        &env,
        &setup.pool_address,
        &setup.operator,
        &setup.reward_token.address,
    );

    // 30 seconds. rewards set up for 60 seconds
    jump(&env, 10);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    gauge.schedule_rewards_config(
        &setup.pool_address,
        &setup.operator,
        &None,
        &60,
        &reward_1_tps,
        &total_shares,
    );

    // 40 seconds. second user depositing after gauge set up
    jump(&env, 30);
    gauge.checkpoint_user(&setup.pool_address, &user2, &0, &total_shares);
    total_shares += 100;

    // 100 seconds. rewards ended.
    jump(&env, 40);
    // 110 seconds. user claims reward
    jump(&env, 10);
    // full reward should be available to users. first receives 3/4 of the reward, second receives 1/4 since it joined later
    assert_eq!(
        gauge.claim(&setup.pool_address, &user1, &100, &total_shares),
        total_reward_1 / 4 * 3
    );
    assert_eq!(
        gauge.claim(&setup.pool_address, &user2, &100, &total_shares),
        total_reward_1 / 4
    );
    assert_eq!(
        setup.reward_token.balance(&user1) as u128,
        total_reward_1 / 4 * 3
    );
    assert_eq!(
        setup.reward_token.balance(&user2) as u128,
        total_reward_1 / 4
    );
}

#[test]
fn test_simple_scheduled_reward() {
    let setup = Setup::with_mocked_pool();

    let user1 = Address::generate(&setup.env);
    let user2 = Address::generate(&setup.env);

    let reward_token_sac = StellarAssetClient::new(&setup.env, &setup.reward_token.address);
    reward_token_sac.mint(&setup.operator, &1_000_000_0000000);

    let mut total_shares = 0;

    jump(&setup.env, 1);
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &setup.operator, &0, &total_shares);
    total_shares = 1000_0000000;
    // inv(1) = 0
    jump(&setup.env, 1);

    // schedule rewards to start on 11th second. stop them on 102nd second to keep the same logic as in the previous test
    setup.contract.schedule_rewards_config(
        &setup.pool_address,
        &setup.operator,
        &Some(11),
        &91,
        &1_0000000,
        &total_shares,
    );
    // inv(2) = 0. config not started yet. No rewards yet generated
    assert_eq!(setup.contract.get_reward_config().tps, 0,);

    jump(&setup.env, 10);
    assert_eq!(setup.contract.get_reward_config().tps, 1_0000000,);
    // first user deposits after 10 seconds. config started 1 second before
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user1, &0, &total_shares);
    total_shares += 1000_0000000;
    // inv(12) = 1 * 10 / 100000

    jump(&setup.env, 10);

    // second user deposits
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user2, &0, &total_shares);
    total_shares += 1000_0000000;
    // inv(22) = inv(12) + 1 * 10 / 101000

    jump(&setup.env, 10);

    // first user withdraws 20 seconds after deposit
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user1, &1000_0000000, &total_shares);
    total_shares -= 1000_0000000;

    jump(&setup.env, 100);
    assert_eq!(setup.contract.get_reward_config().tps, 0);

    // both claim rewards after reward period ends
    let user1_claim = setup
        .contract
        .claim(&setup.pool_address, &user1, &0, &total_shares);
    let user2_claim =
        setup
            .contract
            .claim(&setup.pool_address, &user2, &1000_0000000, &total_shares);
    // tps = 1
    // user1
    //  share1 = 1 / 2
    //  duration1 = 10
    //  reward1 = 1 * 10 / 2 = 5
    //  share2 = 1 / 3
    //  duration2 = 10
    //  reward2 = 1 * 10 / 3 = 3.(3)
    //  reward = 5 + 3.(3) = 8.(3)
    // user2
    //  share1 = 1 / 3
    //  duration1 = 10
    //  reward1 = 1 * 10 / 3 = 3.(3)
    //  share2 = 1 / 2
    //  duration2 = 70
    //  reward2 = 1 * 70 / 2 = 35
    //  reward = 3.(3) + 35 = 38.(3)
    assert_eq!(user1_claim, 83333333);
    assert_eq!(user2_claim, 383333333);
}

#[test]
fn test_get_config_expired_scheduled_reward() {
    let setup = Setup::with_mocked_pool();

    let reward_token_sac = StellarAssetClient::new(&setup.env, &setup.reward_token.address);
    reward_token_sac.mint(&setup.operator, &1_000_000_0000000);

    let mut total_shares = 0;

    jump(&setup.env, 1);
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &setup.operator, &0, &total_shares);
    total_shares = 1000_0000000;
    // inv(1) = 0
    jump(&setup.env, 1);

    setup.contract.schedule_rewards_config(
        &setup.pool_address,
        &setup.operator,
        &Some(11),
        &91,
        &1_0000000,
        &total_shares,
    );
    // inv(2) = 0. config not started yet. No rewards yet generated
    assert_eq!(setup.contract.get_reward_config().tps, 0,);

    jump(&setup.env, 10);
    assert_eq!(setup.contract.get_reward_config().tps, 1_0000000,);

    // config expires. however there was no checkpoint yet, so it was not promoted to current config
    jump(&setup.env, 100);
    assert_eq!(setup.contract.get_reward_config().tps, 0);
}

#[test]
fn test_scheduled_reward() {
    let setup = Setup::with_mocked_pool();

    let day = 3600 * 24; // 1 day in seconds
    let week = day * 7; // 1 week in seconds
    let week1_start = 1751846400; // 7 july 2025
    let week2_start = 1752451200; // 14 july 2025
    let week3_start = 1753056000; // 21 july 2025

    // each user has 100 shares
    // sees future reward, joins on week 0. exits after week 2 when rewards end
    let user1 = Address::generate(&setup.env);
    // joins on week 1 day 1, exits after week 2 after rewards end
    let user2 = Address::generate(&setup.env);
    // joins on week 1 day 4, exits on week 2 before rewards end
    let user3 = Address::generate(&setup.env);
    let user_share = 100_0000000; // 100 shares

    let reward_token_sac = StellarAssetClient::new(&setup.env, &setup.reward_token.address);
    reward_token_sac.mint(&setup.operator, &1_000_000_000_0000000);
    let operator_share = 1000_0000000; // 1000 shares

    let mut total_shares = 0;
    // 1 july 2025
    time_warp(&setup.env, 1751360058);

    // operator deposits
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &setup.operator, &0, &total_shares);
    total_shares = operator_share;

    // schedule weekly rewards for the next week. 1 token per second
    setup.contract.schedule_rewards_config(
        &setup.pool_address,
        &setup.operator,
        &Some(week1_start),
        &week,
        &1_0000000,
        &total_shares,
    );

    jump(&setup.env, day);
    // 2 july 2025, first user deposits after 1 day
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user1, &0, &total_shares);
    total_shares += user_share;

    time_warp(&setup.env, week1_start + 2 * day);

    // second user deposits on wednesday of week 1
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user2, &0, &total_shares);
    total_shares += user_share;

    time_warp(&setup.env, week1_start + 3 * day);
    // third user deposits on thursday of week 1
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user3, &0, &total_shares);
    total_shares += user_share;

    jump(&setup.env, 5);
    // one block after operator schedules rewards for the next week. 2 tokens per second
    setup.contract.schedule_rewards_config(
        &setup.pool_address,
        &setup.operator,
        &Some(week2_start),
        &week,
        &2_0000000,
        &total_shares,
    );

    // first user checkpoints, second checkpoint after 5 seconds
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user1, &user_share, &total_shares);
    jump(&setup.env, 5);
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user2, &user_share, &total_shares);

    // third user claims rewards on friday of week 1
    time_warp(&setup.env, week1_start + 4 * day);
    let user3_claim1 =
        setup
            .contract
            .claim(&setup.pool_address, &user3, &user_share, &total_shares);

    time_warp(&setup.env, week2_start + 4 * day);
    // third user withdraws and claim on friday of week 2
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user3, &user_share, &total_shares);
    total_shares -= user_share;
    let user3_claim2 = setup
        .contract
        .claim(&setup.pool_address, &user3, &0, &total_shares);
    let user3_claim = user3_claim1 + user3_claim2;

    time_warp(&setup.env, week3_start + 2 * day);

    // first and second users withdraw and claim rewards after week 2 ends
    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user1, &user_share, &total_shares);
    total_shares -= user_share;
    let user1_claim = setup
        .contract
        .claim(&setup.pool_address, &user1, &0, &total_shares);

    setup
        .contract
        .checkpoint_user(&setup.pool_address, &user2, &user_share, &total_shares);
    total_shares -= user_share;
    let user2_claim = setup
        .contract
        .claim(&setup.pool_address, &user2, &0, &total_shares);

    jump(&setup.env, day);
    let operator_claim = setup.contract.claim(
        &setup.pool_address,
        &setup.operator,
        &operator_share,
        &total_shares,
    );
    assert_eq!(total_shares, operator_share);

    // week 1, 1 token per second, 86400 tokens generated per day
    //  monday-tuesday:
    //    operator 1000 shares, 78545,4545454 tokens
    //    user1 100 shares, 7854,5454545 tokens
    //  wednesday:
    //    operator 1000 shares, 72000 tokens
    //    user1 100 shares, 7200 tokens
    //    user2 100 shares, 7200 tokens
    //  thursday-sunday:
    //    operator 1000 shares, 66461,5384615 tokens
    //    user1 100 shares, 6646,1538461 tokens
    //    user2 100 shares, 6646,1538461 tokens
    //    user3 100 shares, 6646,1538461 tokens
    // week 2, 2 tokens per second, 172800 tokens generated per day
    //  monday-thursday:
    //    operator 1000 shares, 132923,0769230 tokens
    //    user1 100 shares, 13292,3076923 tokens
    //    user2 100 shares, 13292,3076923 tokens
    //    user3 100 shares, 13292,3076923 tokens
    //  friday-sunday:
    //    operator 1000 shares, 144000 tokens
    //    user1 100 shares, 14400 tokens
    //    user2 100 shares, 14400 tokens
    // week3, no rewards scheduled, no tokens generated
    //
    // user1:      7854,5454545 * 2  +  7200  +  6646,1538461 * 4  +  13292,3076923 * 4  +  14400 * 3  =  145862,9370626
    // user2:                           7200  +  6646,1538461 * 4  +  13292,3076923 * 4  +  14400 * 3  =  130153,8461536
    // user3:                                    6646,1538461 * 4  +  13292,3076923 * 4                =  79753,8461536
    // operator:  78545,4545454 * 2  + 72000  + 66461,5384615 * 4  + 132923,0769230 * 4  + 144000 * 3  =  1458629,3706288

    // check individual claims
    assert_approx_eq_abs(user1_claim, 145862_9370626, 3);
    assert_approx_eq_abs(user2_claim, 130153_8461536, 2);
    assert_approx_eq_abs(user3_claim1, 6646_1538461, 1);
    assert_approx_eq_abs(user3_claim2, 73107_6923076, 1);
    assert_approx_eq_abs(user3_claim, 79753_8461536, 2);
    assert_approx_eq_abs(operator_claim, 1458629_3706288, 6);

    // check total rewards
    let total_reward = user1_claim + user2_claim + user3_claim + operator_claim;
    let total_reward_planned = 86400_0000000 * 7 + 172800_0000000 * 7;
    assert!(total_reward_planned >= total_reward);
    assert_approx_eq_abs(total_reward, total_reward_planned, 6);
}
