#![cfg(test)]
extern crate std;

use crate::{token, LiquidityPoolClient};

use soroban_sdk::testutils::{Ledger, LedgerInfo};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Vec};

fn create_token_contract<'a>(e: &Env, admin: &Address) -> token::Client<'a> {
    token::Client::new(e, &e.register_stellar_asset_contract(admin.clone()))
}

fn create_liqpool_contract<'a>(
    e: &Env,
    admin: &Address,
    token_wasm_hash: &BytesN<32>,
    coins: &Vec<Address>,
    a: u128,
    fee: u128,
    admin_fee: u128,
    token_reward: &Address,
    // fee_fraction: u32,
) -> LiquidityPoolClient<'a> {
    let liqpool = LiquidityPoolClient::new(e, &e.register_contract(None, crate::LiquidityPool {}));
    liqpool.initialize(
        admin,
        token_wasm_hash,
        coins,
        &a,
        &fee,
        &admin_fee,
        token_reward,
        &liqpool.address,
    );
    // liqpool.initialize_fee_fraction(&fee_fraction);
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
fn test_happy_flow() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::random(&e);
    let admin2 = Address::random(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::random(&e);
    let fee = 2000_i128;
    // let admin_fee = 20_i128;
    let admin_fee = 0_i128;
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        fee as u128,
        admin_fee as u128,
        &token_reward.address,
    );

    let token_share = token::Client::new(&e, &liqpool.share_id());

    token1.mint(&user1, &1000_0000000);
    token2.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1), 1000_0000000);
    assert_eq!(token2.balance(&user1), 1000_0000000);
    token1.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token2.approve(&user1, &liqpool.address, &1000_0000000, &99999);

    liqpool.add_liquidity(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000]),
        &100_0000000,
    );
    // assert_eq!(liqpool.get_virtual_price(), 13);
    liqpool.add_liquidity(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000]),
        &100_0000000,
    );
    // assert_eq!(liqpool.get_virtual_price(), 13);
    let calculated_amount =
        liqpool.calc_token_amount(&Vec::from_array(&e, [10_0000000, 10_0000000]), &true);

    let share_token_amount = 200_0000000;
    assert_eq!(calculated_amount as i128, share_token_amount / 10);
    assert_eq!(token_share.balance(&user1), share_token_amount * 2);
    assert_eq!(token_share.balance(&liqpool.address), 0);
    assert_eq!(token1.balance(&user1), 800_0000000);
    assert_eq!(token1.balance(&liqpool.address), 200_0000000);
    assert_eq!(token2.balance(&user1), 800_0000000);
    assert_eq!(token2.balance(&liqpool.address), 200_0000000);

    liqpool.exchange(&user1, &0, &1, &10_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1), 790_0000000);
    assert_eq!(token1.balance(&liqpool.address), 210_0000000);
    assert_eq!(token2.balance(&user1), 807_9637267);
    assert_eq!(token2.balance(&liqpool.address), 192_0362733);

    token_share.approve(&user1, &liqpool.address, &(share_token_amount * 2), &99999);

    liqpool.remove_liquidity(
        &user1,
        &(share_token_amount as u128),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token1.balance(&user1), 895_0000000);
    assert_eq!(token2.balance(&user1), 903_9818633);
    assert_eq!(token_share.balance(&user1), share_token_amount);
    assert_eq!(token1.balance(&liqpool.address), 105_0000000);
    assert_eq!(token2.balance(&liqpool.address), 96_0181367);
    assert_eq!(token_share.balance(&liqpool.address), 0);

    liqpool.remove_liquidity(
        &user1,
        &(share_token_amount as u128),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token1.balance(&user1), 1000_0000000);
    assert_eq!(token2.balance(&user1), 1000_0000000);
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token1.balance(&liqpool.address), 0);
    assert_eq!(token2.balance(&liqpool.address), 0);
    assert_eq!(token_share.balance(&liqpool.address), 0);
}

#[test]
#[should_panic(expected = "is killed")]
fn test_kill() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::random(&e);
    let admin2 = Address::random(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::random(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );
    token1.mint(&user1, &1000_0000000);
    token2.mint(&user1, &1000_0000000);
    token1.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token2.approve(&user1, &liqpool.address, &1000_0000000, &99999);

    liqpool.kill_me(&user1);
    liqpool.add_liquidity(
        &user1,
        &Vec::from_array(&e, [1000_0000000, 1000_0000000]),
        &1000_0000000,
    );
}

#[test]
#[ignore]
fn test_happy_flow_3_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::random(&e);
    let admin2 = Address::random(&e);
    let admin3 = Address::random(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token3 = create_token_contract(&e, &admin3);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::random(&e);
    let fee = 0_i128;
    let admin_fee = 0_i128;
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                token1.address.clone(),
                token2.address.clone(),
                token3.address.clone(),
            ],
        ),
        10,
        fee as u128,
        admin_fee as u128,
        &token_reward.address,
    );

    let token_share = token::Client::new(&e, &liqpool.share_id());

    token1.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1), 1000_0000000);

    token2.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1), 1000_0000000);

    token3.mint(&user1, &1000_0000000);
    assert_eq!(token3.balance(&user1), 1000_0000000);

    token1.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token2.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token3.approve(&user1, &liqpool.address, &1000_0000000, &99999);

    liqpool.add_liquidity(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000]),
        &100_0000000,
    );

    let share_token_amount = 230769230_7692307;
    assert_eq!(token_share.balance(&user1), share_token_amount);
    assert_eq!(token_share.balance(&liqpool.address), 0);
    assert_eq!(token1.balance(&user1), 900_0000000);
    assert_eq!(token1.balance(&liqpool.address), 100_0000000);
    assert_eq!(token2.balance(&user1), 900_0000000);
    assert_eq!(token2.balance(&liqpool.address), 100_0000000);
    assert_eq!(token3.balance(&user1), 900_0000000);
    assert_eq!(token3.balance(&liqpool.address), 100_0000000);

    // assert_eq!(liqpool.estimate_swap_out(&false, &49), 97_i128,);
    liqpool.exchange(&user1, &1, &2, &10_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1), 900_0000000);
    assert_eq!(token1.balance(&liqpool.address), 100_0000000);
    assert_eq!(token2.balance(&user1), 890_0000000);
    assert_eq!(token2.balance(&liqpool.address), 110_0000000);
    assert_eq!(token3.balance(&user1), 909_9091734 - fee);
    assert_eq!(token3.balance(&liqpool.address), 90_0908266 + fee);

    token_share.approve(&user1, &liqpool.address, &share_token_amount, &99999);

    liqpool.remove_liquidity(
        &user1,
        &(share_token_amount as u128),
        &Vec::from_array(&e, [0, 0, 0]),
    );

    assert_eq!(token1.balance(&user1), 1000_0000000);
    assert_eq!(token2.balance(&user1), 1000_0000000);
    assert_eq!(token3.balance(&user1), 1000_0000000);
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token1.balance(&liqpool.address), 0);
    assert_eq!(token2.balance(&liqpool.address), 0);
    assert_eq!(token3.balance(&liqpool.address), 0);
    assert_eq!(token_share.balance(&liqpool.address), 0);
}

#[test]
fn test_remove_liquidity_partial() {
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
    // let fee = 20000_i128;
    let fee = 0_i128;
    // let admin_fee = 300000_i128;
    let admin_fee = 0_i128;
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        fee as u128,
        admin_fee as u128,
        &token_reward.address,
    );

    let token_share = token::Client::new(&e, &liqpool.share_id());

    token1.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1), 1000_0000000);

    token2.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1), 1000_0000000);
    token1.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token2.approve(&user1, &liqpool.address, &1000_0000000, &99999);

    liqpool.add_liquidity(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000]),
        &100_0000000,
    );

    let share_token_amount = 200_0000000;
    assert_eq!(token_share.balance(&user1), share_token_amount);
    assert_eq!(token_share.balance(&liqpool.address), 0);
    assert_eq!(token1.balance(&user1), 900_0000000);
    assert_eq!(token1.balance(&liqpool.address), 100_0000000);
    assert_eq!(token2.balance(&user1), 900_0000000);
    assert_eq!(token2.balance(&liqpool.address), 100_0000000);

    liqpool.exchange(&user1, &0, &1, &10_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1), 890_0000000);
    assert_eq!(token1.balance(&liqpool.address), 110_0000000);
    assert_eq!(token2.balance(&user1), 909_9091734 - fee);
    assert_eq!(token2.balance(&liqpool.address), 90_0908266 + fee);

    token_share.approve(&user1, &liqpool.address, &share_token_amount, &99999);

    liqpool.remove_liquidity(
        &user1,
        &((share_token_amount as u128) * 30 / 100),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token1.balance(&user1), 923_0000000);
    assert_eq!(token2.balance(&user1), 936_9364213);
    assert_eq!(
        token_share.balance(&user1),
        share_token_amount - (share_token_amount * 30 / 100)
    );
    assert_eq!(token1.balance(&liqpool.address), 77_0000000);
    assert_eq!(token2.balance(&liqpool.address), 63_0635787);
    assert_eq!(token_share.balance(&liqpool.address), 0);
}

#[test]
fn test_remove_liquidity_one_token() {
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
        10,
        0,
        0,
        &token_reward.address,
    );

    let token_share = token::Client::new(&e, &liqpool.share_id());

    token1.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1), 1000_0000000);

    token2.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1), 1000_0000000);
    token1.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token2.approve(&user1, &liqpool.address, &1000_0000000, &99999);

    liqpool.add_liquidity(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000]),
        &100_0000000,
    );

    let share_token_amount = 200_0000000;
    assert_eq!(token_share.balance(&user1), share_token_amount);
    assert_eq!(token_share.balance(&liqpool.address), 0);
    assert_eq!(token1.balance(&user1), 900_0000000);
    assert_eq!(token1.balance(&liqpool.address), 100_0000000);
    assert_eq!(token2.balance(&user1), 900_0000000);
    assert_eq!(token2.balance(&liqpool.address), 100_0000000);

    token_share.approve(&user1, &liqpool.address, &share_token_amount, &99999);

    liqpool.remove_liquidity_one_coin(&user1, &100_0000000, &0, &10_0000000);

    assert_eq!(token1.balance(&user1), 991_0435607);
    assert_eq!(token1.balance(&liqpool.address), 8_9564393);
    assert_eq!(token2.balance(&user1), 900_0000000);
    assert_eq!(token2.balance(&liqpool.address), 100_0000000);
    assert_eq!(token_share.balance(&user1), 100_0000000);
    assert_eq!(token_share.balance(&liqpool.address), 0);
}
