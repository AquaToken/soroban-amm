#![cfg(test)]
extern crate std;

use crate::testutils::{MockedPoolClient, Setup};
use soroban_sdk::testutils::Address as _;
use soroban_sdk::token::StellarAssetClient;
use soroban_sdk::Address;
use utils::test_utils::jump;

#[test]
fn test() {
    let setup = Setup::with_mocked_pool();

    let pool = MockedPoolClient::new(&setup.env, &setup.pool_address);

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
    // we need pool total shares for set_rewards_config, so sync it
    pool.set_total_shares(&total_shares);
    // inv(1) = 0
    jump(&setup.env, 1);

    setup
        .contract
        .set_rewards_config(&setup.pool_address, &setup.operator, &100, &1_0000000);
    // inv(2) = 0. no rewards yet generated

    jump(&setup.env, 10);

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
