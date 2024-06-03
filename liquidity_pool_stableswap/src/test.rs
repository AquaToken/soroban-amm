#![cfg(test)]
extern crate std;

use crate::LiquidityPoolClient;

use crate::plane::{pool_plane, PoolPlaneClient};
use crate::pool_constants::{ADMIN_ACTIONS_DELAY, MIN_RAMP_TIME};
use rewards::utils::test_utils::assert_approx_eq_abs;
use soroban_sdk::testutils::{Events, Ledger, LedgerInfo};
use soroban_sdk::token::{
    StellarAssetClient as SorobanTokenAdminClient, TokenClient as SorobanTokenClient,
};
use soroban_sdk::{testutils::Address as _, vec, Address, BytesN, Env, IntoVal, Symbol, Vec};

fn create_token_contract<'a>(e: &Env, admin: &Address) -> SorobanTokenClient<'a> {
    SorobanTokenClient::new(e, &e.register_stellar_asset_contract(admin.clone()))
}

fn get_token_admin_client<'a>(e: &'a Env, address: &'a Address) -> SorobanTokenAdminClient<'a> {
    SorobanTokenAdminClient::new(e, address)
}

fn create_liqpool_contract<'a>(
    e: &Env,
    admin: &Address,
    router: &Address,
    token_wasm_hash: &BytesN<32>,
    coins: &Vec<Address>,
    a: u128,
    fee: u32,
    admin_fee: u32,
    token_reward: &Address,
    plane: &Address,
) -> LiquidityPoolClient<'a> {
    let liqpool = LiquidityPoolClient::new(e, &e.register_contract(None, crate::LiquidityPool {}));
    liqpool.initialize_all(
        admin,
        router,
        token_wasm_hash,
        coins,
        &a,
        &fee,
        &admin_fee,
        token_reward,
        plane,
    );
    liqpool
}

fn install_token_wasm(e: &Env) -> BytesN<32> {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_token_contract.wasm"
    );
    e.deployer().upload_contract_wasm(WASM)
}

fn create_plane_contract<'a>(e: &Env) -> PoolPlaneClient<'a> {
    PoolPlaneClient::new(e, &e.register_contract_wasm(None, pool_plane::WASM))
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

#[test]
#[should_panic(expected = "Error(Contract, #2010)")]
fn test_swap_empty_pool() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token_reward = create_token_contract(&e, &admin1);
    let plane = create_plane_contract(&e);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let admin_fee = 0_u128;
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        fee as u32,
        admin_fee as u32,
        &token_reward.address,
        &plane.address,
    );
    assert_eq!(liqpool.estimate_swap(&0, &1, &10_0000000), 0);
    token1_admin_client.mint(&user1, &10_0000000);
    assert_eq!(liqpool.swap(&user1, &0, &1, &10_0000000, &0), 0);
}

#[test]
fn test_happy_flow() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let admin_fee = 0_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        fee as u32,
        admin_fee as u32,
        &token_reward.address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
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
    assert_eq!(token2.balance(&user1) as u128, 807_9637266);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362734);

    liqpool.withdraw(
        &user1,
        &(total_share_token_amount / 2),
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
        &(total_share_token_amount / 2),
        &Vec::from_array(&e, [0, 0]),
    );

    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 0);
    assert_eq!(token1.balance(&liqpool.address) as u128, 0);
    assert_eq!(token2.balance(&liqpool.address) as u128, 0);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

#[test]
fn test_events_2_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address
    ];
    tokens.sort();
    let token1 = SorobanTokenClient::new(&e, &tokens[0]);
    let token2 = SorobanTokenClient::new(&e, &tokens[1]);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin);
    let user1 = Address::generate(&e);
    let fee = 30_u128;
    let admin_fee = 0_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [tokens[0].clone(), tokens[1].clone()]),
        10,
        fee as u32,
        admin_fee as u32,
        &token_reward.address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    let (amounts, share_amt) =
        liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
    assert_eq!(amounts.get(0).unwrap(), 1000000000);
    assert_eq!(amounts.get(1).unwrap(), 1000000000);
    assert_eq!(share_amt, 2000000000);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "deposit_liquidity"),
                    token1.address.clone(),
                    token2.address.clone(),
                )
                    .into_val(&e),
                (200_0000000_i128, 100_0000000_i128, 100_0000000_i128,).into_val(&e),
            ),
        ]
    );

    assert_eq!(liqpool.swap(&user1, &0, &1, &100, &95), 98);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "trade"),
                    token1.address.clone(),
                    token2.address.clone(),
                    user1.clone()
                )
                    .into_val(&e),
                (100_i128, 98_i128, 1_i128).into_val(&e),
            )
        ]
    );

    let amounts_out = liqpool.withdraw(&user1, &200_0000000, &Vec::from_array(&e, [0, 0]));
    assert_eq!(amounts_out.get(0).unwrap(), 1000000100);
    assert_eq!(amounts_out.get(1).unwrap(), 999999902);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "withdraw_liquidity"),
                    token1.address.clone(),
                    token2.address.clone()
                )
                    .into_val(&e),
                (
                    200_0000000_i128,
                    amounts_out.get(0).unwrap() as i128,
                    amounts_out.get(1).unwrap() as i128
                )
                    .into_val(&e),
            )
        ]
    );
}

#[test]
fn test_events_3_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
    ];
    tokens.sort();
    let token1 = SorobanTokenClient::new(&e, &tokens[0]);
    let token2 = SorobanTokenClient::new(&e, &tokens[1]);
    let token3 = SorobanTokenClient::new(&e, &tokens[2]);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token3_admin_client = get_token_admin_client(&e, &token3.address);
    let token_reward = create_token_contract(&e, &admin);
    let user1 = Address::generate(&e);
    let fee = 30_u128;
    let admin_fee = 0_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [tokens[0].clone(), tokens[1].clone(), tokens[2].clone()],
        ),
        10,
        fee as u32,
        admin_fee as u32,
        &token_reward.address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);
    token3_admin_client.mint(&user1, &1000_0000000);

    let (amounts, share_amt) = liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000]),
        &0,
    );
    assert_eq!(amounts.get(0).unwrap(), 1000000000);
    assert_eq!(amounts.get(1).unwrap(), 1000000000);
    assert_eq!(amounts.get(2).unwrap(), 1000000000);
    assert_eq!(share_amt, 3000000000);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "deposit_liquidity"),
                    token1.address.clone(),
                    token2.address.clone(),
                    token3.address.clone(),
                )
                    .into_val(&e),
                (
                    300_0000000_i128,
                    100_0000000_i128,
                    100_0000000_i128,
                    100_0000000_i128,
                )
                    .into_val(&e),
            ),
        ]
    );

    assert_eq!(liqpool.swap(&user1, &0, &1, &100, &95), 98);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "trade"),
                    token1.address.clone(),
                    token2.address.clone(),
                    user1.clone()
                )
                    .into_val(&e),
                (100_i128, 98_i128, 1_i128).into_val(&e),
            )
        ]
    );

    let amounts_out = liqpool.withdraw(&user1, &300_0000000, &Vec::from_array(&e, [0, 0, 0]));
    assert_eq!(amounts_out.get(0).unwrap(), 1000000100);
    assert_eq!(amounts_out.get(1).unwrap(), 999999902);
    assert_eq!(amounts_out.get(2).unwrap(), 1000000000);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "withdraw_liquidity"),
                    token1.address.clone(),
                    token2.address.clone(),
                    token3.address.clone(),
                )
                    .into_val(&e),
                (
                    300_0000000_i128,
                    amounts_out.get(0).unwrap() as i128,
                    amounts_out.get(1).unwrap() as i128,
                    amounts_out.get(2).unwrap() as i128,
                )
                    .into_val(&e),
            )
        ]
    );
}

#[test]
fn test_events_4_tokens() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let mut tokens = std::vec![
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
        create_token_contract(&e, &admin).address,
    ];
    tokens.sort();
    let token1 = SorobanTokenClient::new(&e, &tokens[0]);
    let token2 = SorobanTokenClient::new(&e, &tokens[1]);
    let token3 = SorobanTokenClient::new(&e, &tokens[2]);
    let token4 = SorobanTokenClient::new(&e, &tokens[3]);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token3_admin_client = get_token_admin_client(&e, &token3.address);
    let token4_admin_client = get_token_admin_client(&e, &token4.address);
    let token_reward = create_token_contract(&e, &admin);
    let user1 = Address::generate(&e);
    let fee = 30_u128;
    let admin_fee = 0_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                tokens[0].clone(),
                tokens[1].clone(),
                tokens[2].clone(),
                tokens[3].clone(),
            ],
        ),
        10,
        fee as u32,
        admin_fee as u32,
        &token_reward.address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);
    token3_admin_client.mint(&user1, &1000_0000000);
    token4_admin_client.mint(&user1, &1000_0000000);

    let (amounts, share_amt) = liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000, 100_0000000]),
        &0,
    );
    assert_eq!(amounts.get(0).unwrap(), 1000000000);
    assert_eq!(amounts.get(1).unwrap(), 1000000000);
    assert_eq!(amounts.get(2).unwrap(), 1000000000);
    assert_eq!(amounts.get(3).unwrap(), 1000000000);
    assert_eq!(share_amt, 4000000000);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "deposit_liquidity"),
                    token1.address.clone(),
                    token2.address.clone(),
                    token3.address.clone(),
                )
                    .into_val(&e),
                (
                    400_0000000_i128,
                    100_0000000_i128,
                    100_0000000_i128,
                    100_0000000_i128,
                )
                    .into_val(&e),
            ),
        ]
    );

    assert_eq!(liqpool.swap(&user1, &0, &1, &100, &95), 98);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "trade"),
                    token1.address.clone(),
                    token2.address.clone(),
                    user1.clone()
                )
                    .into_val(&e),
                (100_i128, 98_i128, 1_i128).into_val(&e),
            )
        ]
    );

    let amounts_out = liqpool.withdraw(&user1, &400_0000000, &Vec::from_array(&e, [0, 0, 0, 0]));
    assert_eq!(amounts_out.get(0).unwrap(), 1000000100);
    assert_eq!(amounts_out.get(1).unwrap(), 999999902);
    assert_eq!(amounts_out.get(2).unwrap(), 1000000000);
    assert_eq!(amounts_out.get(3).unwrap(), 1000000000);
    assert_eq!(
        vec![&e, e.events().all().last().unwrap()],
        vec![
            &e,
            (
                liqpool.address.clone(),
                (
                    Symbol::new(&e, "withdraw_liquidity"),
                    token1.address.clone(),
                    token2.address.clone(),
                    token3.address.clone(),
                )
                    .into_val(&e),
                (
                    400_0000000_i128,
                    amounts_out.get(0).unwrap() as i128,
                    amounts_out.get(1).unwrap() as i128,
                    amounts_out.get(2).unwrap() as i128,
                )
                    .into_val(&e),
            )
        ]
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2901)")]
fn test_kill() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );
    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    liqpool.kill_me(&user1);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [1000_0000000, 1000_0000000]),
        &0,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2003)")]
fn test_bad_fee() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        10000,
        0,
        &token_reward.address,
        &plane.address,
    );
}

#[test]
#[should_panic(expected = "Error(Contract, #2004)")]
fn test_zero_initial_deposit() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );
    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [1000_0000000, 0]), &0);
}

#[test]
fn test_zero_deposit_ok() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );
    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [500_0000000, 500_0000000]), &0);
    liqpool.deposit(&user1, &Vec::from_array(&e, [500_0000000, 0]), &0);
}

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
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token3_admin_client = get_token_admin_client(&e, &token3.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let admin_fee = 0_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
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
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);
    token3_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token3.balance(&user1) as u128, 1000_0000000);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000]),
        &0,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000]),
        &0,
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
    assert_eq!(token2.balance(&user1) as u128, 807_9637266);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362734);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);

    liqpool.swap(&user1, &2, &0, &20_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 805_9304412);
    assert_eq!(token1.balance(&liqpool.address) as u128, 194_0695588);
    assert_eq!(token2.balance(&user1) as u128, 807_9637266);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362734);
    assert_eq!(token3.balance(&user1) as u128, 780_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 220_0000000);

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
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token3_admin_client = get_token_admin_client(&e, &token3.address);
    let token4_admin_client = get_token_admin_client(&e, &token4.address);

    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let fee = 2000_u128;
    let admin_fee = 0_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
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
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);
    token3_admin_client.mint(&user1, &1000_0000000);
    token4_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token3.balance(&user1) as u128, 1000_0000000);
    assert_eq!(token4.balance(&user1) as u128, 1000_0000000);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000, 100_0000000]),
        &0,
    );
    assert_eq!(liqpool.get_virtual_price(), 1_0000000);
    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [100_0000000, 100_0000000, 100_0000000, 100_0000000]),
        &0,
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
    assert_eq!(token2.balance(&user1) as u128, 807_9637266);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362734);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token4.balance(&user1) as u128, 800_0000000);
    assert_eq!(token4.balance(&liqpool.address) as u128, 200_0000000);

    liqpool.swap(&user1, &3, &0, &20_0000000, &1_0000000);

    assert_eq!(token1.balance(&user1) as u128, 805_9304931);
    assert_eq!(token1.balance(&liqpool.address) as u128, 194_0695069);
    assert_eq!(token2.balance(&user1) as u128, 807_9637266);
    assert_eq!(token2.balance(&liqpool.address) as u128, 192_0362734);
    assert_eq!(token3.balance(&user1) as u128, 800_0000000);
    assert_eq!(token3.balance(&liqpool.address) as u128, 200_0000000);
    assert_eq!(token4.balance(&user1) as u128, 780_0000000);
    assert_eq!(token4.balance(&liqpool.address) as u128, 220_0000000);

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
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let user1 = Address::generate(&e);
    let fee = 0_u128;
    let admin_fee = 0_u128;
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        fee as u32,
        admin_fee as u32,
        &token_reward.address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);

    token2_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);

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

    liqpool.withdraw(
        &user1,
        &(share_token_amount * 30 / 100),
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
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token1.balance(&user1) as u128, 1000_0000000);

    token2_admin_client.mint(&user1, &1000_0000000);
    assert_eq!(token2.balance(&user1) as u128, 1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);

    let share_token_amount = 200_0000000_u128;
    assert_eq!(token_share.balance(&user1) as u128, share_token_amount);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
    assert_eq!(token1.balance(&user1) as u128, 900_0000000);
    assert_eq!(token1.balance(&liqpool.address) as u128, 100_0000000);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 100_0000000);

    assert_eq!(
        liqpool.withdraw_one_coin(&user1, &100_0000000, &0, &10_0000000),
        Vec::from_array(&e, [91_0435607_u128, 0_u128]),
    );

    assert_eq!(token1.balance(&user1) as u128, 991_0435607);
    assert_eq!(token1.balance(&liqpool.address) as u128, 8_9564393);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    assert_eq!(token2.balance(&liqpool.address) as u128, 100_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 100_0000000);
    assert_eq!(token_share.balance(&liqpool.address) as u128, 0);
}

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
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let user1 = Address::generate(&e);

    token1_admin_client.mint(&user1, &1000000_0000000);
    token2_admin_client.mint(&user1, &1000000_0000000);

    // we're checking fraction against value required to swap 1 token
    for fee_config in [
        (0, 0, 9990916, 0, 0),           // fee = 0%, admin fee = 0%
        (10, 0, 9980925, 0, 0),          // fee = 0.1%, admin fee = 0%
        (30, 0, 9960943, 0, 0),          // fee = 0.3%, admin fee = 0%
        (100, 0, 9891006, 0, 0),         // fee = 1%, admin fee = 0%
        (1000, 0, 8991824, 0, 0),        // fee = 10%, admin fee = 0%
        (3000, 0, 6993641, 0, 0),        // fee = 30%, admin fee = 0%
        (100, 10, 9891006, 0, 100),      // fee = 0.1%, admin fee = 0.1%
        (100, 100, 9891006, 0, 1000),    // fee = 0.1%, admin fee = 1%
        (100, 1000, 9891006, 0, 9991),   // fee = 0.1%, admin fee = 10%
        (100, 2000, 9891006, 0, 19982),  // fee = 0.1%, admin fee = 20%
        (100, 5000, 9891006, 0, 49955),  // fee = 0.1%, admin fee = 50%
        (100, 10000, 9891006, 0, 99910), // fee = 0.1%, admin fee = 100%
    ] {
        let plane = create_plane_contract(&e);
        let liqpool = create_liqpool_contract(
            &e,
            &user1,
            &Address::generate(&e),
            &install_token_wasm(&e),
            &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
            10,
            fee_config.0,
            fee_config.1,
            &token_reward.address,
            &plane.address,
        );
        liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 100_0000000]), &0);
        assert_eq!(liqpool.estimate_swap(&0, &1, &1_0000000), fee_config.2);
        assert_eq!(liqpool.swap(&user1, &0, &1, &1_0000000, &0), fee_config.2);
        assert_eq!(liqpool.admin_balances(&0), fee_config.3);
        assert_eq!(liqpool.admin_balances(&1), fee_config.4)
    }
}

#[test]
fn test_deposit_inequal() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [10_0000000, 100_0000000]), &0);
    assert_eq!(token_share.balance(&user1) as u128, 101_8767615);
    assert_eq!(token1.balance(&user1) as u128, 990_0000000);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100_0000000, 10_0000000]), &0);
    assert_eq!(token1.balance(&user1) as u128, 890_0000000);
    assert_eq!(token2.balance(&user1) as u128, 890_0000000);
}

#[test]
fn test_remove_liquidity_imbalance() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &1000_0000000);
    token2_admin_client.mint(&user1, &1000_0000000);

    liqpool.deposit(&user1, &Vec::from_array(&e, [10_0000000, 100_0000000]), &0);
    assert_eq!(token1.balance(&user1) as u128, 990_0000000);
    assert_eq!(token2.balance(&user1) as u128, 900_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 101_8767615);
    liqpool.remove_liquidity_imbalance(
        &user1,
        &Vec::from_array(&e, [9_0000000, 9_0000000]),
        &90_0000000,
    );
    assert_eq!(token1.balance(&user1) as u128, 999_0000000);
    assert_eq!(token2.balance(&user1) as u128, 909_0000000);
    assert_eq!(token_share.balance(&user1) as u128, 62_1428988);
}

#[test]
fn test_simple_ongoing_reward() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let token_reward_admin_client = get_token_admin_client(&e, &token_reward.address);

    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    token_reward_admin_client.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    token1_admin_client.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1) as u128, 1000);

    token2_admin_client.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1) as u128, 1000);

    // 10 seconds passed since config, user depositing
    jump(&e, 10);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);

    assert_eq!(token_reward.balance(&user1) as u128, 0);
    // 30 seconds passed, half of the reward is available for the user
    jump(&e, 30);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 2);
    assert_eq!(token_reward.balance(&user1) as u128, total_reward_1 / 2);
}

#[test]
fn test_simple_reward() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let token_reward_admin_client = get_token_admin_client(&e, &token_reward.address);

    let user1 = Address::generate(&e);
    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    token1_admin_client.mint(&user1, &1000);
    assert_eq!(token1.balance(&user1) as u128, 1000);

    token2_admin_client.mint(&user1, &1000);
    assert_eq!(token2.balance(&user1) as u128, 1000);

    // 10 seconds. user depositing
    jump(&e, 10);
    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);

    // 20 seconds. rewards set up for 60 seconds
    jump(&e, 10);
    token_reward_admin_client.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
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

#[test]
fn test_two_users_rewards() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let token_reward_admin_client = get_token_admin_client(&e, &token_reward.address);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    token_reward_admin_client.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    for user in [&user1, &user2] {
        token1_admin_client.mint(user, &1000);
        assert_eq!(token1.balance(user) as u128, 1000);

        token2_admin_client.mint(user, &1000);
        assert_eq!(token2.balance(user) as u128, 1000);
    }

    // two users make deposit for equal value. second after 30 seconds after rewards start,
    //  so it gets only 1/4 of total reward
    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);
    jump(&e, 30);
    assert_eq!(liqpool.claim(&user1), total_reward_1 / 2);
    liqpool.deposit(&user2, &Vec::from_array(&e, [100, 100]), &0);
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

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);
    let token_reward_admin_client = get_token_admin_client(&e, &token_reward.address);

    let user1 = Address::generate(&e);
    let user2 = Address::generate(&e);

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &user1,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    token_reward_admin_client.mint(&liqpool.address, &1_000_000_0000000);
    let reward_1_tps = 10_5000000_u128;
    let total_reward_1 = reward_1_tps * 60;
    liqpool.set_rewards_config(
        &user1,
        &e.ledger().timestamp().saturating_add(60),
        &reward_1_tps,
    );

    for user in [&user1, &user2] {
        token1_admin_client.mint(user, &1000);
        assert_eq!(token1.balance(user) as u128, 1000);

        token2_admin_client.mint(user, &1000);
        assert_eq!(token2.balance(user) as u128, 1000);
    }

    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);
    jump(&e, 59);
    liqpool.deposit(&user2, &Vec::from_array(&e, [1000, 1000]), &0);
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
#[should_panic(expected = "Error(Contract, #102)")]
fn test_config_rewards_not_admin() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &admin,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                create_token_contract(&e, &admin).address,
                create_token_contract(&e, &admin).address,
            ],
        ),
        10,
        0,
        0,
        &(create_token_contract(&e, &admin).address),
        &(create_plane_contract(&e).address),
    );

    liqpool.set_rewards_config(
        &Address::generate(&e),
        &e.ledger().timestamp().saturating_add(60),
        &1,
    );
}

#[test]
fn test_config_rewards_router() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin = Address::generate(&e);
    let router = Address::generate(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &admin,
        &router,
        &install_token_wasm(&e),
        &Vec::from_array(
            &e,
            [
                create_token_contract(&e, &admin).address,
                create_token_contract(&e, &admin).address,
            ],
        ),
        10,
        0,
        0,
        &(create_token_contract(&e, &admin).address),
        &(create_plane_contract(&e).address),
    );

    liqpool.set_rewards_config(&router, &e.ledger().timestamp().saturating_add(60), &1);
}

#[test]
#[should_panic(expected = "Error(Contract, #2908)")]
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

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    liqpool.commit_new_fee(&pool_admin_original, &30, &1);
    assert_eq!(liqpool.get_fee_fraction(), 0);
    assert_eq!(liqpool.get_admin_fee(), 0);
    liqpool.apply_new_fee(&pool_admin_original);
    jump(&e, 2 * 30 * 86400 - 1);
}

#[test]
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

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
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
#[should_panic(expected = "Error(Contract, #2908)")]
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

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    liqpool.commit_transfer_ownership(&pool_admin_original, &pool_admin_new);
    // check admin by calling protected method
    liqpool.donate_admin_fees(&pool_admin_original);
    jump(&e, ADMIN_ACTIONS_DELAY - 1);
    liqpool.apply_transfer_ownership(&pool_admin_original);
}

#[test]
#[should_panic(expected = "Error(Contract, #2906)")]
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

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    liqpool.commit_transfer_ownership(&pool_admin_original, &pool_admin_new);
    liqpool.commit_transfer_ownership(&pool_admin_original, &pool_admin_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
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

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    jump(&e, ADMIN_ACTIONS_DELAY + 1);
    liqpool.apply_transfer_ownership(&pool_admin_original);
}

#[test]
#[should_panic(expected = "Error(Contract, #2907)")]
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

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    liqpool.commit_transfer_ownership(&pool_admin_original, &pool_admin_new);
    // check admin by calling protected method
    liqpool.donate_admin_fees(&pool_admin_original);
    jump(&e, ADMIN_ACTIONS_DELAY + 1);
    liqpool.revert_transfer_ownership(&pool_admin_original);
    liqpool.apply_transfer_ownership(&pool_admin_original);
}

#[test]
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

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    liqpool.commit_transfer_ownership(&pool_admin_original, &pool_admin_new);
    // check admin by calling protected method
    liqpool.donate_admin_fees(&pool_admin_original);
    jump(&e, ADMIN_ACTIONS_DELAY + 1);
    liqpool.apply_transfer_ownership(&pool_admin_original);
    liqpool.donate_admin_fees(&pool_admin_new);
}

#[test]
#[should_panic(expected = "Error(Contract, #2902)")]
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

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
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
#[should_panic(expected = "Error(Contract, #2903)")]
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

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
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
#[should_panic(expected = "Error(Contract, #2905)")]
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

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
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

    let plane = create_plane_contract(&e);
    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin_original,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
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
#[should_panic(expected = "Error(Contract, #2006)")]
fn test_deposit_min_mint() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin = Address::generate(&e);
    let plane = create_plane_contract(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    let user1 = Address::generate(&e);
    token1_admin_client.mint(&user1, &i128::MAX);
    token2_admin_client.mint(&user1, &i128::MAX);

    liqpool.deposit(
        &user1,
        &Vec::from_array(&e, [1_000_000_000_0000000, 1_000_000_000_0000000]),
        &0,
    );
    liqpool.deposit(&user1, &Vec::from_array(&e, [1, 1]), &10);
}

#[test]
fn test_deposit_inequal_ok() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin = Address::generate(&e);
    let plane = create_plane_contract(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        10,
        0,
        0,
        &token_reward.address,
        &plane.address,
    );

    let user1 = Address::generate(&e);
    token1_admin_client.mint(&user1, &i128::MAX);
    token2_admin_client.mint(&user1, &i128::MAX);

    liqpool.deposit(&user1, &Vec::from_array(&e, [100, 100]), &0);

    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());
    assert_eq!(token1.balance(&liqpool.address), 100);
    assert_eq!(token2.balance(&liqpool.address), 100);
    assert_eq!(token_share.balance(&user1), 200);
    liqpool.deposit(&user1, &Vec::from_array(&e, [200, 100]), &0);
    assert_eq!(token1.balance(&liqpool.address), 300);
    assert_eq!(token2.balance(&liqpool.address), 200);
    assert_eq!(token_share.balance(&user1), 499);
}

#[test]
fn test_large_numbers() {
    let e = Env::default();
    e.mock_all_auths();
    e.budget().reset_unlimited();

    let admin1 = Address::generate(&e);
    let admin2 = Address::generate(&e);

    let token1 = create_token_contract(&e, &admin1);
    let token2 = create_token_contract(&e, &admin2);
    let token1_admin_client = get_token_admin_client(&e, &token1.address);
    let token2_admin_client = get_token_admin_client(&e, &token2.address);
    let token_reward = create_token_contract(&e, &admin1);

    let pool_admin = Address::generate(&e);
    let plane = create_plane_contract(&e);

    let liqpool = create_liqpool_contract(
        &e,
        &pool_admin,
        &Address::generate(&e),
        &install_token_wasm(&e),
        &Vec::from_array(&e, [token1.address.clone(), token2.address.clone()]),
        85,
        6,
        0,
        &token_reward.address,
        &plane.address,
    );

    let user1 = Address::generate(&e);
    let token_share = SorobanTokenClient::new(&e, &liqpool.share_id());

    token1_admin_client.mint(&user1, &i128::MAX);
    token2_admin_client.mint(&user1, &i128::MAX);

    let amount_to_deposit = u128::MAX / 1_000_000;
    let desired_amounts = Vec::from_array(&e, [amount_to_deposit, amount_to_deposit]);

    liqpool.deposit(&user1, &desired_amounts, &0);

    // when we deposit equal amounts, we gotta have deposited amount of share tokens
    assert_eq!(token_share.balance(&liqpool.address), 0);
    assert_eq!(
        token1.balance(&user1),
        i128::MAX - amount_to_deposit as i128
    );
    assert_eq!(token1.balance(&liqpool.address), amount_to_deposit as i128);
    assert_eq!(
        token2.balance(&user1),
        i128::MAX - amount_to_deposit as i128
    );
    assert_eq!(token2.balance(&liqpool.address), amount_to_deposit as i128);

    let swap_in = amount_to_deposit / 1_000;
    // swap out shouldn't differ for more than 0.1% since fee is 0.06%
    let expected_swap_result_delta = swap_in / 1000;
    let estimate_swap_result = liqpool.estimate_swap(&0, &1, &swap_in);
    assert_approx_eq_abs(estimate_swap_result, swap_in, expected_swap_result_delta);
    assert_eq!(
        liqpool.swap(&user1, &0, &1, &swap_in, &estimate_swap_result),
        estimate_swap_result
    );

    assert_eq!(
        token1.balance(&user1),
        i128::MAX - amount_to_deposit as i128 - swap_in as i128
    );
    assert_eq!(
        token1.balance(&liqpool.address),
        amount_to_deposit as i128 + swap_in as i128
    );
    assert_eq!(
        token2.balance(&user1),
        i128::MAX - amount_to_deposit as i128 + estimate_swap_result as i128
    );
    assert_eq!(
        token2.balance(&liqpool.address),
        amount_to_deposit as i128 - estimate_swap_result as i128
    );

    let share_amount = token_share.balance(&user1);

    let withdraw_amounts = [
        amount_to_deposit + swap_in,
        amount_to_deposit - estimate_swap_result,
    ];
    liqpool.withdraw(
        &user1,
        &(share_amount as u128),
        &Vec::from_array(&e, withdraw_amounts),
    );

    assert_eq!(token1.balance(&user1), i128::MAX);
    assert_eq!(token2.balance(&user1), i128::MAX);
    assert_eq!(token_share.balance(&user1), 0);
    assert_eq!(token1.balance(&liqpool.address), 0);
    assert_eq!(token2.balance(&liqpool.address), 0);
    assert_eq!(token_share.balance(&liqpool.address), 0);
}
