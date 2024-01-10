#![cfg(test)]
extern crate std;

use crate::LiquidityPoolClient;

use crate::pool_constants::{ADMIN_ACTIONS_DELAY, MIN_RAMP_TIME};
use rewards::utils::test_utils::assert_approx_eq_abs;
use soroban_sdk::testutils::{Ledger, LedgerInfo};
use soroban_sdk::{testutils::Address as _, Address, BytesN, Env, Vec};
use token_share::Client;

fn create_token_contract<'a>(e: &Env, admin: &Address) -> Client<'a> {
    Client::new(e, &e.register_stellar_asset_contract(admin.clone()))
}

fn create_liqpool_contract<'a>(
    e: &Env,
    admin: &Address,
    token_wasm_hash: &BytesN<32>,
    coins: &Vec<Address>,
    a: u128,
    fee: u32,
    admin_fee: u32,
    token_reward: &Address,
    // fee_fraction: u32,
) -> LiquidityPoolClient<'a> {
    let liqpool = LiquidityPoolClient::new(e, &e.register_contract(None, crate::LiquidityPool {}));
    liqpool.initialize_all(
        &admin,
        token_wasm_hash,
        coins,
        &a,
        &fee,
        &admin_fee,
        token_reward,
        &liqpool.address,
    );
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
        min_temp_entry_ttl: 999999,
        min_persistent_entry_ttl: 999999,
        max_entry_ttl: u32::MAX,
    });
}

#[cfg(feature = "tokens_2")]
#[test]
fn test_swap_empty_pool() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let admin_fee = 0_u128;
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        fee as u32,
        admin_fee as u32,
        &token_reward.address,
    );
    assert_eq!(liqpool.estimate_swap(&0, &1, &10_0000000), 0);
}

#[cfg(feature = "tokens_2")]
#[test]
fn test_happy_flow() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let admin_fee = 0_u128;
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        fee as u32,
        admin_fee as u32,
        &token_reward.address,
    );

    let token_share = Client::new(&e, &liqpool.share_id());

    token1.mint(&user1, &1000_0000000);
    token2.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    token1.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token2.approve(&user1, &liqpool.address, &1000_0000000, &99999);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000]),
        // &100_0000000,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000]),
        // &100_0000000,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
    let calculated_amount =
        liqpool.calc_token_amount(&Vec::from_array(&e, [10_0000000, 10_0000000]), &true);

    let total_share_token_amount = 400_0000000_u128; // share amount after two deposits

    assert_eq!(calculated_amount as u128, total_share_token_amount / 2 / 10);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount
    );
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token1.balance(&user1) as u128, 800_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token2.balance(&user1) as u128, 800_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 200_0000000);

    liqpool.swap(&user1, &0, &1, &10_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 790_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 210_0000000);
    assert_eq!(token2.balance(&user1) as u128, 807_9637267);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362733);

    token_share.approve(
        &user1,
        &liqpool.address,
        &(total_share_token_amount as i128),
        &99999,
    );

    liqpool.withdraw(
        &user1,
        &((total_share_token_amount as u128) / 2),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 895_0000000);
    assert_eq!(token2.balance(&user1) as u128, 903_9818633);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount / 2
    );
    assert_eq!(token1.balance(&liqpool.address) as u128, 105_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 96_0181367);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);

    liqpool.withdraw(
        &user1,
        &((total_share_token_amount as u128) / 2),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 0);
    assert_eq!(token1.balance(&liqpool.address) as u128, 0);
    assert_eq!(token2.balance(&liqpool.address) as u128, 0);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[cfg(feature = "tokens_2")]
#[test]
#[should_panic(expected = "is killed")]
fn test_kill() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
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
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [1000_0000000, 1000_0000000]),
        // &1000_0000000,
    );
}

#[cfg(feature = "tokens_3")]
#[test]
fn test_happy_flow_3_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);
    let admin3 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token3 = create_token_contract(&e, &admin3);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let admin_fee = 0_u128;
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
        fee as u32,
        admin_fee as u32,
        &token_reward.address,
    );

    let token_share = Client::new(&e, &liqpool.share_id());

    token1.mint(&user1, &1000_0000000);
    token2.mint(&user1, &1000_0000000);
    token3.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token3.balance(&user1) as u128, 1000_0000000);
    token1.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token2.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token3.approve(&user1, &liqpool.address, &1000_0000000, &99999);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000]),
        // &100_0000000,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000]),
        // &100_0000000,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000); // ???
    let calculated_amount = liqpool.calc_token_amount(
        &Vec::from_array(&e, [10_0000000, 10_0000000, 10_0000000]),
        &true,
    );

    let total_share_token_amount = 600_0000000_u128; // share amount after two deposits

    assert_eq!(calculated_amount, total_share_token_amount / 2 / 10);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount
    );
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token1.balance(&user1) as u128, 800_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token2.balance(&user1) as u128, 800_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);

    liqpool.swap(&user1, &0, &1, &10_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 790_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 210_0000000);
    assert_eq!(token2.balance(&user1) as u128, 807_9637267);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362733);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);

    liqpool.swap(&user1, &2, &0, &20_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 805_9304412);
    assert_eq!(token1.balance(&liqpool.address) as u128, 194_0695588);
    assert_eq!(token2.balance(&user1) as u128, 807_9637267);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362733);
    assert_eq!(token3.balance(&user1) as u128, 780_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 220_0000000);

    token_share.approve(
        &user1,
        &liqpool.address,
        &(total_share_token_amount as i128),
        &99999,
    );

    liqpool.withdraw(
        &user1,
        &((total_share_token_amount as u128) / 2),
        &Vec::from_array(&e, [0, 0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 902_9652206);
    assert_eq!(token2.balance(&user1) as u128, 903_9818633);
    assert_eq!(token3.balance(&user1) as u128, 890_0000000);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount / 2
    );
    assert_eq!(token1.balance(&liqpool.address) as u128, 97_0347794);
    assert_eq!(token2.balance(&liqpool.address) as u128, 96_0181367);
    assert_eq!(token3.balance(&liqpool.address) as u128, 110_0000000);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);

    liqpool.withdraw(
        &user1,
        &((total_share_token_amount as u128) / 2),
        &Vec::from_array(&e, [0, 0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token3.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 0);
    assert_eq!(token1.balance(&liqpool.address) as u128, 0);
    assert_eq!(token2.balance(&liqpool.address) as u128, 0);
    assert_eq!(token3.balance(&liqpool.address) as u128, 0);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[cfg(feature = "tokens_4")]
#[test]
fn test_happy_flow_4_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);
    let admin3 = Address::generate(&e);
    let admin4 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token3 = create_token_contract(&e, &admin3);
    let token4 = create_token_contract(&e, &admin4);

    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let admin_fee = 0_u128;
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
                token4.address.clone(),
            ],
        ),
        10,
        fee as u32,
        admin_fee as u32,
        &token_reward.address,
    );

    let token_share = Client::new(&e, &liqpool.share_id());

    token1.mint(&user1, &1000_0000000);
    token2.mint(&user1, &1000_0000000);
    token3.mint(&user1, &1000_0000000);
    token4.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token3.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token4.balance(&user1) as u128, 1000_0000000);
    token1.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token2.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token3.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token4.approve(&user1, &liqpool.address, &1000_0000000, &99999);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000, 100_0000000]),
        // &100_0000000,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000, 100_0000000]),
        // &100_0000000,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000); // ???
    let calculated_amount = liqpool.calc_token_amount(
        &Vec::from_array(&e, [10_0000000, 10_0000000, 10_0000000, 10_0000000]),
        &true,
    );

    let total_share_token_amount = 800_0000000_u128; // share amount after two deposits

    assert_eq!(calculated_amount, total_share_token_amount / 2 / 10);
    assert_eq!(
        token_share.balance(&user1) as u128,
        total_share_token_amount
    );
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token1.balance(&user1) as u128, 800_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token2.balance(&user1) as u128, 800_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token4.balance(&user1) as u128, 800_0000000);
    assert_eq!(token4.balance(&liqpool.address) as u128, 200_0000000);

    liqpool.swap(&user1, &0, &1, &10_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 790_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 210_0000000);
    assert_eq!(token2.balance(&user1) as u128, 807_9637267);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362733);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token4.balance(&user1) as u128, 800_0000000);
    assert_eq!(token4.balance(&liqpool.address) as u128, 200_0000000);

    liqpool.swap(&user1, &3, &0, &20_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 805_9304932);
    assert_eq!(token1.balance(&liqpool.address) as u128, 194_0695068);
    assert_eq!(token2.balance(&user1) as u128, 807_9637267);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362733);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token4.balance(&user1) as u128, 780_0000000);
    assert_eq!(token4.balance(&liqpool.address) as u128, 220_0000000);

    token_share.approve(
        &user1,
        &liqpool.address,
        &(total_share_token_amount as i128),
        &99999,
    );

    liqpool.withdraw(
        &user1,
        &(total_share_token_amount),
        &Vec::from_array(&e, [0, 0, 0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token3.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token4.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 0);
    assert_eq!(token1.balance(&liqpool.address) as u128, 0);
    assert_eq!(token2.balance(&liqpool.address) as u128, 0);
    assert_eq!(token3.balance(&liqpool.address) as u128, 0);
    assert_eq!(token4.balance(&liqpool.address) as u128, 0);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[cfg(feature = "tokens_2")]
#[test]
fn test_withdraw_partial() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let user1 = Address::generate(&e);
    // let fee = 20000_u128;
    let fee = 0_u128;
    // let admin_fee = 300000_u128;
    let admin_fee = 0_u128;
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        fee as u32,
        admin_fee as u32,
        &token_reward.address,
    );

    let token_share = Client::new(&e, &liqpool.share_id());

    token1.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);

    token2.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    token1.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token2.approve(&user1, &liqpool.address, &1000_0000000, &99999);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000]),
        // &100_0000000,
    );

    let share_token_amount = 200_0000000;
    assert_eq!(token_share.balance(&user1) as u128, share_token_amount);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token1.balance(&user1) as u128, 900_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 100_0000000);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 100_0000000);

    liqpool.swap(&user1, &0, &1, &10_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 890_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 110_0000000);
    assert_eq!(token2.balance(&user1) as u128, 909_9091734 - fee);
    assert_eq!(token2.balance(&liqpool.address) as u128, 90_0908266 + fee);

    token_share.approve(
        &user1,
        &liqpool.address,
        &(share_token_amount as i128),
        &99999,
    );

    liqpool.withdraw(
        &user1,
        &((share_token_amount as u128) * 30 / 100),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 923_0000000);
    assert_eq!(token2.balance(&user1) as u128, 936_9364213);
    assert_eq!(
        token_share.balance(&user1) as u128,
        share_token_amount - (share_token_amount * 30 / 100)
    );
    assert_eq!(token1.balance(&liqpool.address) as u128, 77_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 63_0635787);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[cfg(feature = "tokens_2")]
#[test]
fn test_withdraw_one_token() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let user1 = Address::generate(&e);
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

    let token_share = Client::new(&e, &liqpool.share_id());

    token1.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);

    token2.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    token1.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token2.approve(&user1, &liqpool.address, &1000_0000000, &99999);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000]),
        // &100_0000000,
    );

    let share_token_amount = 200_0000000_u128;
    assert_eq!(token_share.balance(&user1) as u128, share_token_amount);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token1.balance(&user1) as u128, 900_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 100_0000000);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 100_0000000);

    token_share.approve(
        &user1,
        &liqpool.address,
        &(share_token_amount as i128),
        &99999,
    );

    liqpool.withdraw_one_coin(&user1, &100_0000000, &0, &10_0000000);

    assert_eq!(token1.balance(&user1) as u128, 991_0435607);
    assert_eq!(token1.balance(&liqpool.address) as u128, 8_9564393);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 100_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 100_0000000);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[cfg(feature = "tokens_2")]
#[test]
fn test_custom_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let user1 = Address::generate(&e);

    token1.mint(&user1, &1000000_0000000);
    token2.mint(&user1, &1000000_0000000);

    // we're checking fraction against value required to swap 1 token
    for fee_config in [
        (0, 0, 9990916, 0, 0),           // fee = 0%, admin fee = 0%
        (10, 0, 9980926, 0, 0),          // fee = 0.1%, admin fee = 0%
        (30, 0, 9960944, 0, 0),          // fee = 0.3%, admin fee = 0%
        (100, 0, 9891007, 0, 0),         // fee = 1%, admin fee = 0%
        (1000, 0, 8991825, 0, 0),        // fee = 10%, admin fee = 0%
        (3000, 0, 6993642, 0, 0),        // fee = 30%, admin fee = 0%
        (9900, 0, 99910, 0, 0),          // fee = 99%, admin fee = 0%
        (9999, 0, 1000, 0, 0),           // fee = 99.99% - maximum fee, admin fee = 0%
        (100, 10, 9891007, 0, 99),       // fee = 0.1%, admin fee = 0.1%
        (100, 100, 9891007, 0, 999),     // fee = 0.1%, admin fee = 1%
        (100, 1000, 9891007, 0, 9990),   // fee = 0.1%, admin fee = 10%
        (100, 2000, 9891007, 0, 19981),  // fee = 0.1%, admin fee = 20%
        (100, 5000, 9891007, 0, 49954),  // fee = 0.1%, admin fee = 50%
        (100, 10000, 9891007, 0, 99909), // fee = 0.1%, admin fee = 100%
    ] {
        let liqpool = create_liqpool_contract(
            &e,
            &user1,
            &install_token_wasm(&e),
            &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
            10,
            fee_config.0,
            fee_config.1,
            &token_reward.address,
        );
        token1.approve(&user1, &liqpool.address, &100000_0000000, &99999);
        token2.approve(&user1, &liqpool.address, &100000_0000000, &99999);
        liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]));
        assert_eq!(liqpool.estimate_swap(&0, &1, &1_0000000), fee_config.2);
        assert_eq!(liqpool.swap(&user1, &0, &1, &1_0000000, &0), fee_config.2);
        assert_eq!(liqpool.admin_balances(&0), fee_config.3);
        assert_eq!(liqpool.admin_balances(&1), fee_config.4)
    }
}

#[cfg(feature = "tokens_2")]
#[test]
fn test_deposit_inequal() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
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

    let token_share = Client::new(&e, &liqpool.share_id());

    token1.mint(&user1, &1000_0000000);
    token2.mint(&user1, &1000_0000000);
    token1.approve(&user1, &liqpool.address, &1000_0000000, &99999);
    token2.approve(&user1, &liqpool.address, &1000_0000000, &99999);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [10_0000000, 100_0000000]),
        // &10_0000000,
    );

    assert_eq!(token_share.balance(&user1) as u128, 101_8767615);
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
}

#[cfg(feature = "tokens_2")]
#[test]
fn test_simple_ongoing_reward() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let user1 = Address::generate(&e);
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
    assert_eq!(token1.balance(&user1) as u128, 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1) as u128, 1000);
    token1.approve(&user1, &liqpool.address, &1000, &99999);
    token2.approve(&user1, &liqpool.address, &1000, &99999);

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100, 100]),
        // &100,
    );

    assert_eq!(token_reward.balance(&user1) as u128, 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1 / 2);
}

#[cfg(feature = "tokens_2")]
#[test]
fn test_simple_reward() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let user1 = Address::generate(&e);
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

    token1.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1) as u128, 1000);

    token2.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1) as u128, 1000);
    token1.approve(&user1, &liqpool.address, &1000, &99999);
    token2.approve(&user1, &liqpool.address, &1000, &99999);

    // 10 seconds. user depositing
    jump(&e, 10);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100, 100]),
        // &100,
    );

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
    assert_eq!(token_reward.balance(&user1) as u128, 0);
    // full reward should be available to the user
    assert_eq!(liqpool.claim(&user1), total_reward_1);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1);
}

#[cfg(feature = "tokens_2")]
#[test]
fn test_two_users_rewards() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

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
        assert_eq!(token1.balance(user) as u128, 1000);

        token2.mint(user, &1000);
        assert_eq!(token2.balance(user) as u128, 1000);

        token1.approve(user, &liqpool.address, &1000, &99999);
        token2.approve(user, &liqpool.address, &1000, &99999);
    }

    // two users make deposit for equal value. second after 30 seconds after rewards start,
    //  so it gets only 1/4 of total reward
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100, 100]),
        // &100,
    );
    jump(&e, 30);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 2);
    liqpool.deposit(
        &user2,
        &Vec::from_array(&e, [100, 100]),
        // &100,
    );
    jump(&e, 100);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 4);
    assert_eq!(liqpool.claim(&user2), total_reward_1 / 4);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1 / 4 * 3);
    assert_eq!(token_reward.balance(&user2) as u128, total_reward_1 / 4);
}

#[test]
#[cfg(feature = "tokens_2")]
fn test_lazy_user_rewards() {
    // first user comes as initial liquidity provider and expects to get maximum reward
    //  second user comes at the end makes huge deposit
    //  first should receive almost full reward

    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

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
        assert_eq!(token1.balance(user) as u128, 1000);

        token2.mint(user, &1000);
        assert_eq!(token2.balance(user) as u128, 1000);

        token1.approve(user, &liqpool.address, &1000, &99999);
        token2.approve(user, &liqpool.address, &1000, &99999);
    }

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100, 100]),
        // &100,
    );
    jump(&e, 59);
    liqpool.deposit(
        &user2,
        &Vec::from_array(&e, [1000, 1000]),
        // &100,
    );
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
#[cfg(feature = "tokens_2")]
#[should_panic(expected = "insufficient time")]
fn test_update_fee_too_early() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );

    liqpool.commit_new_fee(&pool_admin_original, &30, &1);
    assert_eq!(liqpool.get_fee_fraction(), 0);
    assert_eq!(liqpool.get_admin_fee(), 0);
    liqpool.apply_new_fee(&pool_admin_original);
    jump(&e, 2 * 30 * 86400 - 1);
}

#[test]
#[cfg(feature = "tokens_2")]
fn test_update_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );

    liqpool.commit_new_fee(&pool_admin_original, &30, &1);
    assert_eq!(liqpool.get_fee_fraction(), 0);
    assert_eq!(liqpool.get_admin_fee(), 0);

    jump(&e, 2 * 30 * 86400 + 1);
    liqpool.apply_new_fee(&pool_admin_original);
    assert_eq!(liqpool.get_fee_fraction(), 30);
    assert_eq!(liqpool.get_admin_fee(), 1);
}

#[test]
#[cfg(feature = "tokens_2")]
#[should_panic(expected = "insufficient time")]
fn test_transfer_ownership_too_early() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);
    let pool_admin_new = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );

    liqpool.commit_transfer_ownership(&pool_admin_original, &pool_admin_new);
    // check admin by calling protected method
    liqpool.donate_admin_fees(&pool_admin_original);
    jump(&e, ADMIN_ACTIONS_DELAY - 1);
    liqpool.apply_transfer_ownership(&pool_admin_original);
}

#[test]
#[cfg(feature = "tokens_2")]
#[should_panic(expected = "active transfer")]
fn test_transfer_ownership_twice() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);
    let pool_admin_new = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );

    liqpool.commit_transfer_ownership(&pool_admin_original, &pool_admin_new);
    liqpool.commit_transfer_ownership(&pool_admin_original, &pool_admin_new);
}

#[test]
#[cfg(feature = "tokens_2")]
#[should_panic(expected = "no active transfer")]
fn test_transfer_ownership_not_committed() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );

    jump(&e, ADMIN_ACTIONS_DELAY + 1);
    liqpool.apply_transfer_ownership(&pool_admin_original);
}

#[test]
#[cfg(feature = "tokens_2")]
#[should_panic(expected = "no active transfer")]
fn test_transfer_ownership_reverted() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);
    let pool_admin_new = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );

    liqpool.commit_transfer_ownership(&pool_admin_original, &pool_admin_new);
    // check admin by calling protected method
    liqpool.donate_admin_fees(&pool_admin_original);
    jump(&e, ADMIN_ACTIONS_DELAY + 1);
    liqpool.revert_transfer_ownership(&pool_admin_original);
    liqpool.apply_transfer_ownership(&pool_admin_original);
}

#[test]
#[cfg(feature = "tokens_2")]
fn test_transfer_ownership() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);
    let pool_admin_new = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );

    liqpool.commit_transfer_ownership(&pool_admin_original, &pool_admin_new);
    // check admin by calling protected method
    liqpool.donate_admin_fees(&pool_admin_original);
    jump(&e, ADMIN_ACTIONS_DELAY + 1);
    liqpool.apply_transfer_ownership(&pool_admin_original);
    liqpool.donate_admin_fees(&pool_admin_new);
}

#[test]
#[cfg(feature = "tokens_2")]
#[should_panic(expected = "ramp time is less than minimal")]
fn test_ramp_a_too_early() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );

    jump(&e, MIN_RAMP_TIME - 1);
    assert_eq!(liqpool.a(), 10);
    liqpool.ramp_a(
        &pool_admin_original,
        &30,
        &e.ledger().timestamp().saturating_add(MIN_RAMP_TIME + 1),
    );
}

#[test]
#[cfg(feature = "tokens_2")]
#[should_panic(expected = "insufficient time")]
fn test_ramp_a_too_short() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );

    jump(&e, MIN_RAMP_TIME + 1);
    assert_eq!(liqpool.a(), 10);
    liqpool.ramp_a(
        &pool_admin_original,
        &30,
        &e.ledger().timestamp().saturating_add(MIN_RAMP_TIME - 1),
    );
}

#[test]
#[cfg(feature = "tokens_2")]
#[should_panic(expected = "too rapid change")]
fn test_ramp_a_too_fast() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );

    jump(&e, MIN_RAMP_TIME + 1);
    assert_eq!(liqpool.a(), 10);
    liqpool.ramp_a(
        &pool_admin_original,
        &101,
        &e.ledger().timestamp().saturating_add(MIN_RAMP_TIME + 1),
    );
}

#[test]
#[cfg(feature = "tokens_2")]
fn test_ramp_a() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin_original = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
    );

    jump(&e, MIN_RAMP_TIME + 1);
    assert_eq!(liqpool.a(), 10);
    liqpool.ramp_a(
        &pool_admin_original,
        &99,
        &e.ledger().timestamp().saturating_add(MIN_RAMP_TIME + 1),
    );
    jump(&e, MIN_RAMP_TIME / 2 + 1);
    assert_eq!(liqpool.a(), 54);
    jump(&e, MIN_RAMP_TIME);
    assert_eq!(liqpool.a(), 99);
}

#[test]
#[cfg(feature = "tokens_2")]
fn test_liquidity() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let mut admin1 = Address::generate(&e);
    let mut admin2 = Address::generate(&e);

    let mut token1 = create_token_contract(&e, &admin1);
    let mut token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    if &token2.address < &token1.address {
        std::mem::swap(&mut token1, &mut token2);
        std::mem::swap(&mut admin1, &mut admin2);
    }
    let user1 = Address::generate(&e);

    token1.mint(&user1, &1_000_000_000_000_000_000_0000000);
    token2.mint(&user1, &1_000_000_000_000_000_000_0000000);

    for config in [
        (10, 10, 0),
        (30, 30, 0),
        (100, 100, 2),
        (3000, 3000, 102),
        (1000, 3000, 68),
        (3000, 1000, 68),
        (10_0000000, 30_0000000, 6809640),
        (30_0000000, 10_0000000, 6809640),
        (
            30_000_000_000_0000000,
            10_000_000_000_0000000,
            6809641229721896,
        ),
    ] {
        let liqpool = create_liqpool_contract(
            &e,
            &user1,
            &install_token_wasm(&e),
            &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
            85,
            30, // 0.3%
            0,
            &token_reward.address,
        );
        token1.approve(
            &user1,
            &liqpool.address,
            &1_000_000_000_000_000_000_0000000,
            &99999,
        );
        token2.approve(
            &user1,
            &liqpool.address,
            &1_000_000_000_000_000_000_0000000,
            &99999,
        );
        liqpool.deposit(&user1, &Vec::from_array(&e, [config.0, config.1]));
        assert_eq!(liqpool.get_liquidity(), config.2);
    }
}
