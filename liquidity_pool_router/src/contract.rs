use crate::admin::{get_admin, has_admin, require_admin, set_admin};
use crate::pool_contract::{StableSwapLiquidityPoolClient, StandardLiquidityPoolClient};
use crate::storage::{
    get_constant_product_pool_hash, get_pool_id, get_pools_list, get_reward_token,
    get_stableswap_pool_hash, get_token_hash, has_pool, put_pool, set_constant_product_pool_hash,
    set_reward_token, set_token_hash,
};
use crate::token;
use soroban_sdk::IntoVal;
use soroban_sdk::{
    contract, contractimpl, symbol_short, vec, Address, BytesN, Env, Map, Symbol, TryIntoVal, Vec,
};

pub trait LiquidityPoolRouterTrait {
    fn init_pool(e: Env, token_a: Address, token_b: Address) -> Address;
    fn init_standard_pool(e: Env, token_a: Address, token_b: Address, fee_fraction: u32)
        -> Address;
    fn init_stableswap_pool(
        e: Env,
        token_a: Address,
        token_b: Address,
        fee_fraction: u32,
    ) -> Address;

    fn get_pools_list(e: Env) -> Vec<Address>;

    fn get_pool_hash(e: Env) -> BytesN<32>;
    fn set_pool_hash(e: Env, new_hash: BytesN<32>);

    fn get_token_hash(e: Env) -> BytesN<32>;
    fn set_token_hash(e: Env, new_hash: BytesN<32>);
    fn get_reward_token(e: Env) -> Address;
    fn set_reward_token(e: Env, reward_token: Address);

    fn deposit(
        e: Env,
        account: Address,
        token_a: Address,
        token_b: Address,
        desired_a: i128,
        min_a: i128,
        desired_b: i128,
        min_b: i128,
    ) -> (i128, i128);

    // swaps out an exact amount of "buy", in exchange for "sell" that this contract has an
    // allowance for from "to". "sell" amount swapped in must not be greater than "in_max"
    fn swap_out(
        e: Env,
        account: Address,
        sell: Address,
        buy: Address,
        out: i128,
        in_max: i128,
    ) -> i128;
    fn estimate_swap_out(e: Env, sell: Address, buy: Address, out: i128) -> i128;

    fn withdraw(
        e: Env,
        account: Address,
        token_a: Address,
        token_b: Address,
        share_amount: i128,
        min_a: i128,
        min_b: i128,
    ) -> (i128, i128);

    // returns the contract address for the specified token_a/token_b combo
    fn get_pool(e: Env, token_a: Address, token_b: Address) -> (bool, Address);

    // get pool reserves amount. it may differ from pool balance
    fn get_reserves(e: Env, token_a: Address, token_b: Address) -> (i128, i128);
    fn set_rewards_config(
        e: Env,
        token_a: Address,
        token_b: Address,
        admin: Address,
        expired_at: u64,
        amount: i128,
    );
    fn get_rewards_info(
        e: Env,
        token_a: Address,
        token_b: Address,
        user: Address,
    ) -> Map<Symbol, i128>;
    fn get_user_reward(e: Env, token_a: Address, token_b: Address, user: Address) -> i128;
    fn claim(e: Env, token_a: Address, token_b: Address, user: Address) -> i128;
}

pub trait UpgradeableContract {
    fn init_admin(e: Env, account: Address);
    fn version() -> u32;
    fn upgrade(e: Env, new_wasm_hash: BytesN<32>);
}

#[contract]
struct LiquidityPoolRouter;

enum PoolType {
    Standard,
    StableSwap,
}

fn deploy_pool(
    e: &Env,
    token_a: &Address,
    token_b: &Address,
    pool_type: PoolType,
    fee_fraction: u32,
) {
    let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
    let liquidity_pool_wasm_hash = match pool_type {
        PoolType::Standard => get_constant_product_pool_hash(&e),
        PoolType::StableSwap => get_stableswap_pool_hash(&e),
    };

    let pool_contract_id = e
        .deployer()
        .with_current_contract(salt.clone())
        .deploy(liquidity_pool_wasm_hash);

    put_pool(&e, &salt, &pool_contract_id);

    // TODO: NOT FOR PRODUCTION
    //  this is unsafe as we can store limited amount of records
    // add_pool_to_list(&e, &get_pool_id(&e, &salt));

    let pool_type_str;
    match pool_type {
        PoolType::Standard => {
            init_standard_pool(e, token_a, token_b, &pool_contract_id, fee_fraction);
            pool_type_str = symbol_short!("constant");
        }
        PoolType::StableSwap => {
            // admin fee and a are hardcoded
            init_stableswap_pool(e, token_a, token_b, 10, &pool_contract_id, fee_fraction, 0);
            pool_type_str = symbol_short!("stable");
        }
    }

    e.events().publish(
        (
            Symbol::new(&e, "init_pool"),
            token_a.clone(),
            token_b.clone(),
        ),
        (&pool_contract_id, pool_type_str, fee_fraction),
    );
}

fn init_standard_pool(
    e: &Env,
    token_a: &Address,
    token_b: &Address,
    pool_contract_id: &Address,
    fee_fraction: u32,
) {
    let token_wasm_hash = get_token_hash(&e);
    let reward_token = get_reward_token(&e);
    let admin = get_admin(&e);
    let liq_pool_client = StandardLiquidityPoolClient::new(&e, pool_contract_id);
    liq_pool_client.initialize(
        &admin,
        &token_wasm_hash,
        &token_a,
        &token_b,
        &fee_fraction,
        &reward_token,
        &e.current_contract_address(),
    );
}

fn init_stableswap_pool(
    e: &Env,
    token_a: &Address,
    token_b: &Address,
    a: u128,
    pool_contract_id: &Address,
    fee_fraction: u32,
    admin_fee_fraction: u32,
) {
    let token_wasm_hash = get_token_hash(&e);
    let reward_token = get_reward_token(&e);
    let admin = get_admin(&e);
    let liq_pool_client = StableSwapLiquidityPoolClient::new(&e, pool_contract_id);
    liq_pool_client.initialize(
        &admin,
        &token_wasm_hash,
        &Vec::from_array(&e, [token_a.clone(), token_b.clone()]),
        &a,
        &(fee_fraction as u128),
        &(admin_fee_fraction as u128),
        &reward_token,
        &e.current_contract_address(),
    );
}

#[contractimpl]
impl LiquidityPoolRouterTrait for LiquidityPoolRouter {
    fn init_pool(e: Env, token_a: Address, token_b: Address) -> Address {
        // todo: allow multiple pools within pair of tokens
        let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
        if !has_pool(&e, &salt) {
            deploy_pool(&e, &token_a, &token_b, PoolType::Standard, 30);
        }
        let (_pool_exists, pool_id) = Self::get_pool(e.clone(), token_a, token_b);
        pool_id
    }

    fn init_standard_pool(
        e: Env,
        token_a: Address,
        token_b: Address,
        fee_fraction: u32,
    ) -> Address {
        let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
        if !has_pool(&e, &salt) {
            deploy_pool(&e, &token_a, &token_b, PoolType::Standard, fee_fraction);
        }
        let (_pool_exists, pool_id) = Self::get_pool(e.clone(), token_a, token_b);
        pool_id
    }

    fn init_stableswap_pool(
        e: Env,
        token_a: Address,
        token_b: Address,
        fee_fraction: u32,
    ) -> Address {
        let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
        if !has_pool(&e, &salt) {
            deploy_pool(&e, &token_a, &token_b, PoolType::StableSwap, fee_fraction);
        }
        let (_pool_exists, pool_id) = Self::get_pool(e.clone(), token_a, token_b);
        pool_id
    }

    fn get_pools_list(e: Env) -> Vec<Address> {
        get_pools_list(&e)
    }

    fn get_pool_hash(e: Env) -> BytesN<32> {
        get_constant_product_pool_hash(&e)
    }

    fn set_pool_hash(e: Env, new_hash: BytesN<32>) {
        require_admin(&e);
        set_constant_product_pool_hash(&e, &new_hash);
    }

    fn get_token_hash(e: Env) -> BytesN<32> {
        get_token_hash(&e)
    }

    fn set_token_hash(e: Env, new_hash: BytesN<32>) {
        require_admin(&e);
        set_token_hash(&e, &new_hash);
    }

    fn get_reward_token(e: Env) -> Address {
        get_reward_token(&e)
    }

    fn set_reward_token(e: Env, reward_token: Address) {
        require_admin(&e);
        set_reward_token(&e, &reward_token);
    }

    fn deposit(
        e: Env,
        account: Address,
        token_a: Address,
        token_b: Address,
        desired_a: i128,
        min_a: i128,
        desired_b: i128,
        min_b: i128,
    ) -> (i128, i128) {
        account.require_auth();

        let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
        let pool_id = get_pool_id(&e, &salt);

        let (amount_a, amount_b): (i128, i128) = e.invoke_contract(
            &pool_id,
            &symbol_short!("deposit"),
            vec![
                &e,
                account.clone().try_into_val(&e).unwrap(),
                desired_a.try_into_val(&e).unwrap(),
                min_a.try_into_val(&e).unwrap(),
                desired_b.try_into_val(&e).unwrap(),
                min_b.try_into_val(&e).unwrap(),
            ],
        );

        // e.events().publish(
        //     (Symbol::new(&e, "deposit"), token_a, token_b, account),
        //     vec![&e, pool_id, amount_a, amount_b],
        // );

        (amount_a, amount_b)
    }

    fn swap_out(
        e: Env,
        account: Address,
        sell: Address,
        buy: Address,
        out: i128,
        in_max: i128,
    ) -> i128 {
        account.require_auth();
        let (token_a, token_b) = crate::utils::sort(&sell, &buy);
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), token_a.clone(), token_b.clone());
        if !pool_exists {
            panic!("pool not exists")
        }

        let in_amt = e.invoke_contract(
            &pool_id,
            &symbol_short!("swap"),
            vec![
                &e,
                account.clone().try_into_val(&e).unwrap(),
                (buy == token_a).try_into_val(&e).unwrap(),
                out.try_into_val(&e).unwrap(),
                in_max.try_into_val(&e).unwrap(),
            ],
        );

        e.events().publish(
            (Symbol::new(&e, "swap_out"), token_a, token_b, account),
            (pool_id, sell, buy, out, in_amt),
        );

        in_amt
    }

    fn estimate_swap_out(e: Env, sell: Address, buy: Address, out: i128) -> i128 {
        let (token_a, token_b) = crate::utils::sort(&sell, &buy);
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), token_a.clone(), token_b);
        if !pool_exists {
            panic!("pool not exists")
        }

        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "estimate_swap_out"),
            vec![
                &e,
                (buy == token_a).try_into_val(&e).unwrap(),
                out.try_into_val(&e).unwrap(),
            ],
        )
    }

    fn withdraw(
        e: Env,
        account: Address,
        token_a: Address,
        token_b: Address,
        share_amount: i128,
        min_a: i128,
        min_b: i128,
    ) -> (i128, i128) {
        account.require_auth();
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), token_a.clone(), token_b.clone());
        if !pool_exists {
            panic!("pool not exists")
        }

        let (amount_a, amount_b) = e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "withdraw"),
            vec![
                &e,
                account.clone().try_into_val(&e).unwrap(),
                share_amount.try_into_val(&e).unwrap(),
                min_a.try_into_val(&e).unwrap(),
                min_b.try_into_val(&e).unwrap(),
            ],
        );

        if amount_a < min_a || amount_b < min_b {
            panic!("min not satisfied");
        }

        e.events().publish(
            (Symbol::new(&e, "withdraw"), token_a, token_b, account),
            (pool_id, share_amount, amount_a, amount_b),
        );

        (amount_a, amount_b)
    }

    fn get_pool(e: Env, token_a: Address, token_b: Address) -> (bool, Address) {
        let salt = crate::utils::pool_salt(&e, &token_a, &token_b);
        (has_pool(&e, &salt), get_pool_id(&e, &salt))
    }

    fn get_reserves(e: Env, token_a: Address, token_b: Address) -> (i128, i128) {
        let (a, b) = crate::utils::sort(&token_a, &token_b);
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), a, b);
        if !pool_exists {
            panic!("pool not exists")
        }
        e.invoke_contract(&pool_id, &symbol_short!("get_rsrvs"), Vec::new(&e))
    }

    fn set_rewards_config(
        e: Env,
        token_a: Address,
        token_b: Address,
        admin: Address,
        expired_at: u64,
        amount: i128,
    ) {
        admin.require_auth();
        let (a, b) = crate::utils::sort(&token_a, &token_b);
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), a, b);
        if !pool_exists {
            panic!("pool not exists")
        }
        let _a: bool = e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "set_rewards_config"),
            vec![
                &e,
                admin.clone().try_into_val(&e).unwrap(),
                expired_at.try_into_val(&e).unwrap(),
                amount.try_into_val(&e).unwrap(),
            ],
        );

        let reward_token = get_reward_token(&e);

        e.events().publish(
            (Symbol::new(&e, "set_rewards_config"), token_a, token_b),
            (pool_id, reward_token, amount, expired_at),
        );
    }

    fn get_rewards_info(
        e: Env,
        token_a: Address,
        token_b: Address,
        user: Address,
    ) -> Map<Symbol, i128> {
        let (a, b) = crate::utils::sort(&token_a, &token_b);
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), a, b);
        if !pool_exists {
            panic!("pool not exists")
        }
        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_rewards_info"),
            vec![&e, user.clone().into_val(&e)],
        )
    }

    fn get_user_reward(e: Env, token_a: Address, token_b: Address, user: Address) -> i128 {
        let (a, b) = crate::utils::sort(&token_a, &token_b);
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), a, b);
        if !pool_exists {
            panic!("pool not exists")
        }
        e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_user_reward"),
            vec![&e, user.clone().into_val(&e)],
        )
    }

    fn claim(e: Env, token_a: Address, token_b: Address, user: Address) -> i128 {
        let (a, b) = crate::utils::sort(&token_a, &token_b);
        let (pool_exists, pool_id) = Self::get_pool(e.clone(), a, b);
        if !pool_exists {
            panic!("pool not exists")
        }
        let reward_token = get_reward_token(&e);
        let token_client = token::token::Client::new(&e, &reward_token);
        let reward_amount = e.invoke_contract(
            &pool_id,
            &Symbol::new(&e, "get_user_reward"),
            vec![&e, user.clone().into_val(&e)],
        );
        token_client.approve(
            &e.current_contract_address(),
            &pool_id,
            &reward_amount,
            &(e.ledger().sequence() + 1),
        );
        let claimed_amt = e.invoke_contract(
            &pool_id,
            &symbol_short!("claim"),
            vec![&e, user.clone().into_val(&e)],
        );

        e.events().publish(
            (Symbol::new(&e, "claim"), token_a, token_b, user),
            (pool_id, reward_token, claimed_amt),
        );

        claimed_amt
    }
}

#[contractimpl]
impl UpgradeableContract for LiquidityPoolRouter {
    fn init_admin(e: Env, account: Address) {
        if !has_admin(&e) {
            set_admin(&e, &account)
        }
    }

    fn version() -> u32 {
        7
    }

    fn upgrade(e: Env, new_wasm_hash: BytesN<32>) {
        require_admin(&e);
        e.deployer().update_current_contract_wasm(new_wasm_hash);
    }
}
