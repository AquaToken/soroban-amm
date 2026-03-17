# AGENTS

This document provides comprehensive architecture details and development guidelines for the Aquarius AMM smart contract system on Stellar/Soroban. It is designed to assist AI agents (Claude, Codex) in understanding, navigating, and modifying the codebase effectively.

---

## Table of Contents

1. [Repository Overview](#repository-overview)
2. [Architecture Diagram](#architecture-diagram)
3. [Project Layout](#project-layout)
4. [Core Contracts](#core-contracts)
5. [Shared Libraries](#shared-libraries)
6. [Access Control & Roles](#access-control--roles)
7. [Key Data Flows](#key-data-flows)
8. [Storage Patterns](#storage-patterns)
9. [Rewards System](#rewards-system)
10. [Error Handling](#error-handling)
11. [Upgrade Mechanism](#upgrade-mechanism)
12. [Tooling & Commands](#tooling--commands)
13. [Development Guidelines](#development-guidelines)
14. [Testing](#testing)
15. [Common Modification Scenarios](#common-modification-scenarios)

---

## Repository Overview

- **Purpose**: Rust workspace of Soroban smart contracts for the Aquarius AMM on Stellar blockchain
- **Token**: AQUA governance token powers the DAO voting system
- **Version**: 1.8.0
- **Rust Version**: 1.92+
- **Soroban SDK**: 25.0.2
- **Core Features**:
  - Constant Product AMM (xy=k) for standard pairs
  - StableSwap AMM (Curve-style) for correlated assets
  - Liquidity rewards distribution with boost mechanism
  - Multi-pool routing for optimal swaps
  - Protocol fee collection

---

## Architecture Diagram

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           User / Frontend                                    │
└────────────────────────────────────┬────────────────────────────────────────┘
                                     │
                                     ▼
┌─────────────────────────────────────────────────────────────────────────────┐
│                        liquidity_pool_router                                 │
│  ┌──────────────────────────────────────────────────────────────────────┐   │
│  │  Entry point for all pool operations:                                │   │
│  │  • Pool discovery & creation (init_standard_pool, init_stableswap,   │   │
│  │    init_concentrated_pool)                                          │   │
│  │  • Deposits, swaps, withdrawals (proxied to specific pools)          │   │
│  │  • Multi-hop swaps (swap_chained, swap_chained_strict_receive)       │   │
│  │  • Global rewards configuration (config_global_rewards)               │   │
│  │  • Pool rewards distribution (config_pool_rewards)                    │   │
│  └──────────────────────────────────────────────────────────────────────┘   │
└────────────────────────────────────┬────────────────────────────────────────┘
                                     │
      ┌─────────────────┼─────────────────┬──────────────────────┐
      │                 │                 │                      │
      ▼                 ▼                 ▼                      ▼
┌───────────────┐ ┌───────────────┐ ┌──────────────────┐ ┌─────────────────┐
│ liquidity_    │ │ liquidity_    │ │ liquidity_pool_  │ │ liquidity_pool_ │
│ pool          │ │ pool_         │ │ concentrated     │ │ plane           │
│ (Const Prod)  │ │ stableswap    │ │ (Uni V3-style)   │ │                 │
│               │ │ (Curve-style) │ │                  │ │ Lightweight     │
│ xy=k formula  │ │               │ │ Tick-based conc. │ │ data store for  │
│ 2 tokens only │ │ StableSwap    │ │ liquidity, custom│ │ pool metadata:  │
│ Fee tiers:    │ │ math, 2-4     │ │ price ranges     │ │ • pool type     │
│ 0.1%/0.3%/1%  │ │ tokens, amp A │ │ 2 tokens, fee    │ │ • reserves      │
│               │ │               │ │ tiers: 10/30/100 │ │ • init params   │
└───────┬───────┘ └───────┬───────┘ └────────┬─────────┘ └─────────────────┘
        │                 │                  │                    ▲
        │                 │                  │                    │
        └────────┬────────┴──────────────────┘                   │
                 │                                               │
                       ▼                                       │
          ┌────────────────────────┐                           │
          │      token_share       │                           │
          │   (LP Token Contract)  │                           │
          │   SEP-0041 compliant   │                           │
          │   Mints/burns on       │                           │
          │   deposit/withdraw     │          Updates plane ───┘
          └────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                           Rewards Subsystem                                  │
├─────────────────────┬─────────────────────────┬─────────────────────────────┤
│  rewards (library)  │   rewards_gauge         │  liquidity_pool_reward_     │
│                     │   (standalone contract) │  gauge (embedded module)    │
│  Core rewards math: │                         │                             │
│  • TPS calculation  │  External gauge for     │  Per-pool gauge management  │
│  • User checkpoints │  third-party rewards    │  within pool contracts      │
│  • Boost logic      │  distribution           │  • checkpoint_user          │
│  • Working balance  │                         │  • claim rewards            │
│                     │  Supports multiple      │  • add/remove gauges        │
│  Plugins:           │  reward configs         │                             │
│  • BoostManager     │  (MAX_REWARD_CONFIGS=5) │                             │
│  • OptOutManager    │                         │                             │
└─────────────────────┴─────────────────────────┴─────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                          Support Contracts                                   │
├──────────────────────┬────────────────────────┬─────────────────────────────┤
│   locker_feed        │   fees_collector       │  config_storage /           │
│                      │                        │  liquidity_pool_config_     │
│   Provides locked    │   Collects protocol    │  storage                    │
│   token supply data  │   fees from pools      │                             │
│   for boost calc     │                        │  Stores shared config       │
│                      │   (placeholder for     │  data for pools             │
│   Updated by         │   future expansion)    │                             │
│   operations_admin   │                        │                             │
└──────────────────────┴────────────────────────┴─────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                        Utility Contracts                                     │
├────────────────────────────────────┬────────────────────────────────────────┤
│   batcher                          │   guard                                │
│                                    │                                        │
│   Multicall: batches multiple      │   Return-value validator: invokes a   │
│   contract calls into a single     │   contract call and asserts the       │
│   atomic transaction.              │   result matches an expected value.   │
│                                    │                                        │
│   Used for multi-claim rewards     │   Used with batcher to verify         │
│   across pools and safe            │   upgrade correctness:                │
│   atomic upgrades.                 │   batch(pool.upgrade,                 │
│                                    │     guard(pool.get_version),          │
│                                    │     guard(pool.get_type))             │
└────────────────────────────────────┴────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────┐
│                    liquidity_pool_liquidity_calculator                       │
│                                                                              │
│   Batch liquidity calculation for multiple pools                             │
│   Used for rewards distribution proportionally to liquidity                  │
│   Reads data from liquidity_pool_plane for efficiency                        │
└─────────────────────────────────────────────────────────────────────────────┘
```

---

## Project Layout

### Contract Crates (deployable WASM)

| Crate | Description | Key Files |
|-------|-------------|-----------|
| `liquidity_pool` | Constant product AMM (xy=k) | `contract.rs`, `pool.rs`, `pool_interface.rs` |
| `liquidity_pool_stableswap` | StableSwap AMM (Curve-style) | `contract.rs`, `pool_interface.rs`, `normalize.rs` |
| `liquidity_pool_concentrated` | Concentrated liquidity AMM (Uniswap V3-style) | `contract/`, `math.rs`, `u512.rs`, `storage.rs` |
| `liquidity_pool_router` | Entry point, pool factory, rewards orchestration | `contract.rs`, `router_interface.rs`, `pool_interface.rs` |
| `liquidity_pool_plane` | Lightweight pool metadata store | `contract.rs`, `interface.rs` |
| `liquidity_pool_liquidity_calculator` | Batch liquidity calculations | `contract.rs`, `calculator.rs` |
| `rewards_gauge` | Standalone rewards gauge contract | `contract.rs`, `gauge.rs` |
| `fees_collector` | Protocol fees collection | `contract.rs` |
| `locker_feed` | Locked token supply oracle | `contract.rs` |
| `config_storage` | Shared configuration storage | `contract.rs` |
| `batcher` | Multicall — atomic batch execution of multiple contract calls | `src/lib.rs` |
| `guard` | Return-value validator — asserts contract call results match expected values | `src/lib.rs` |

### Shared Libraries (non-deployable)

| Crate | Purpose |
|-------|---------|
| `utils` | Math utilities, storage helpers, bump extensions |
| `access_control` | Role-based access control, ownership transfer |
| `rewards` | Core rewards calculation logic |
| `token_share` | LP token operations (mint/burn shares) |
| `upgrade` | Contract upgrade logic with delay |
| `liquidity_pool_events` | Standardized event emission |
| `liquidity_pool_validation_errors` | Common validation error codes |
| `liquidity_pool_reward_gauge` | Embedded gauge module for pools |
| `liquidity_pool_config_storage` | Config storage interface for pools |

### Generated Artifacts

- `contracts/*.wasm` - Compiled and optimized contract binaries
- **DO NOT edit by hand** - regenerate via `task build`

---

## Core Contracts

### liquidity_pool (Constant Product)

**Formula**: `x * y = k`

**Key Characteristics**:
- Exactly 2 tokens per pool
- Pre-defined fee tiers: 0.1% (10), 0.3% (30), 1% (100)
- Pool identified by `(token_a, token_b, fee)` → unique salt → pool_index hash
- Supports rebase tokens (positive rebases only via `_sync_reserves`)

**Main Functions**:
```rust
fn deposit(user, desired_amounts, min_shares) -> (Vec<u128>, u128)
fn swap(user, in_idx, out_idx, in_amount, out_min) -> u128
fn swap_strict_receive(user, in_idx, out_idx, out_amount, in_max) -> u128
fn withdraw(user, share_amount, min_amounts) -> Vec<u128>
fn claim(user) -> u128  // claim AQUA rewards
```

**Pool Math** (in `pool.rs`):
```rust
// Amount out calculation
fn get_amount_out(reserve_in, reserve_out, amount_in, fee) -> u128
// amount_out = (amount_in * (1 - fee) * reserve_out) / (reserve_in + amount_in * (1 - fee))
```

**Storage Keys** (in `storage.rs`):
- `TokenA`, `TokenB` - Token addresses
- `ReserveA`, `ReserveB` - Current reserves
- `FeeFraction` - Pool fee (10, 30, or 100)
- `ProtocolFeeFraction` - Portion of fee to protocol
- `ProtocolFeeA`, `ProtocolFeeB` - Accumulated protocol fees
- `Router` - Router contract address
- `Plane` - Plane contract address

### liquidity_pool_stableswap (Curve-style)

**Formula**: StableSwap invariant with amplification coefficient A

**Key Characteristics**:
- 2-4 tokens per pool (`STABLESWAP_MAX_TOKENS = 4`)
- Configurable fee (denominator 10000; 1 = 0.01%)
- Dynamic amplification coefficient A (can be ramped over time)
- Virtual price for profit tracking
- Normalized reserves using token decimals

**Unique Functions**:
```rust
fn a() -> u128  // Current amplification coefficient
fn get_virtual_price() -> u128  // Portfolio value per LP token (scaled 1e7)
fn calc_token_amount(amounts, deposit) -> u128  // Estimate without fees
fn remove_liquidity_imbalance(user, amounts, max_burn) -> u128
fn withdraw_one_coin(user, share_amount, coin_idx, min_amount) -> Vec<u128>
fn ramp_a(admin, future_a, future_time)  // Gradually change A
fn stop_ramp_a(admin)  // Stop A ramping
fn commit_new_fee(admin, new_fee)  // Stage new fee
fn apply_new_fee(admin)  // Apply staged fee
```

**Amplification Parameter (A)**:
- Controls pool concentration around price equilibrium
- Higher A = tighter price range, lower slippage for pegged assets
- Range: 1 to MAX_A (defined in `pool_constants.rs`)
- Ramping constraints: `MIN_RAMP_TIME`, `MAX_A_CHANGE`

**Normalization** (in `normalize.rs`):
- `xp()` - Normalize reserves to common precision (1e7)
- Handles tokens with different decimals

### liquidity_pool_router

**Role**: Central orchestrator and factory

**Pool Management**:
```rust
fn init_standard_pool(user, tokens, fee_fraction) -> (BytesN<32>, Address)
fn init_stableswap_pool(user, tokens, fee_fraction) -> (BytesN<32>, Address)
fn get_pool(tokens, pool_index) -> Address
fn get_pools(tokens) -> Map<BytesN<32>, Address>
fn remove_pool(admin, tokens, pool_hash)
```

**Multi-hop Swaps**:
```rust
fn swap_chained(
    user,
    swaps_chain: Vec<(Vec<Address>, BytesN<32>, Address)>,  // (pool_tokens, pool_index, token_out)
    token_in,
    in_amount,
    out_min
) -> u128

fn swap_chained_strict_receive(
    user,
    swaps_chain,
    token_in,
    out_amount,
    in_max
) -> u128
```

**Rewards Configuration**:
```rust
fn config_global_rewards(admin, reward_tps, expired_at, tokens_votes)
fn fill_liquidity(admin, tokens)  // Must call before config_pool_rewards
fn config_pool_rewards(admin, tokens, pool_index) -> u128  // Returns pool TPS
fn distribute_outstanding_reward(user, from, tokens, pool_index) -> u128
```

**Pool Creation Payment**:
- Can require payment to create pools
- Configured via `configure_init_pool_payment(admin, token, standard_amount, stable_amount, to)`
- Different amounts for standard vs stableswap pools

### liquidity_pool_concentrated (Uniswap V3-style)

**Purpose**: Tick-based concentrated liquidity pool with custom price ranges.

**Key Files**:
- `contract/mod.rs` — imports, struct, submodule declarations
- `contract/internal.rs` — core logic (swap loop, fee growth, tick/bitmap management)
- `contract/extensions.rs` — public trait: `deposit_position`, `withdraw_position`, `claim_position_fees`, getters
- `contract/liquidity_pool_interface.rs` — router-compatible trait: `deposit` (full-range), `withdraw`, `swap`
- `contract/admin.rs` — admin operations (kill switches, protocol fees)
- `math.rs` — pure math (tick↔sqrt_price, amount deltas, fee growth)
- `u512.rs` — 512-bit mul-div for overflow-safe arithmetic
- `storage.rs` — storage accessors, constants
- `errors.rs` — error enum (shared 1xx/2xx codes + concentrated-specific 21xx)

**Pool Creation & Price Initialization**:

The initial price is set by the first deposit's token ratio (`sqrt_price = sqrt(amount1/amount0)`).
To prevent front-running price manipulation, pool creation and first deposit should be
batched atomically via the batcher contract:

1. `router.init_concentrated_pool(tokens, fee)` — deploys pool, sets placeholder price at tick 0
2. `pool.deposit_position(sender, tick_lower, tick_upper, [amount0, amount1], min_liq)` — sets actual price from amounts

Or with a full-range deposit:

1. `router.init_concentrated_pool(tokens, fee)` — deploys pool
2. `router.deposit(user, tokens, pool_index, [amount0, amount1], min_shares)` — full-range deposit, sets price

After the first deposit (`total_raw_liquidity > 0`), the price cannot be re-initialized.
Without batching, there is a window between pool creation and first deposit where an
attacker could front-run with a dust deposit at a manipulated price ratio.

**Deposit Behavior**:
- First deposit on empty pool: requires both tokens (`AllCoinsRequired`), derives price from ratio
- Subsequent deposits: use existing price, compute max liquidity from desired amounts
- Out-of-range deposit: takes only one token (above range → token1 only, below → token0 only)
- Auth-deterministic: transfers full `desired_amounts`, refunds excess

**Swap Behavior**:
- Exact-input (`swap`): transfers `in_amount` from user, no refund — pool keeps full input.
  If the swap partially fills (hits tick bounds / exhausts liquidity), the unswapped portion
  is distributed to LPs at the last active tick via `fee_growth_global` (see Surplus Distribution below).
  User is protected by `out_min` slippage parameter.
- Exact-output (`swap_strict_receive`): transfers `in_max` from user, refunds excess (`in_max - actual_in`);
  reverts with `InMaxNotSatisfied` if actual cost exceeds `in_max`;
  reverts with `InsufficientLiquidity` if the pool cannot produce the requested output.
- Both modes are auth-deterministic: transfer amounts are function parameters known at signing time

**Surplus Distribution (Partial Exact-Input Swaps)**:
When an exact-input swap cannot fully execute, the unswapped input tokens are distributed
to LPs as bonus fees via the existing `fee_growth_global` mechanism:

1. During `swap_loop`, `last_nonzero_liquidity` tracks the active liquidity value before
   it drops to zero after the last tick crossing.
2. After the swap loop, if `unswapped = user_max_in - actual_in > 0`, the contract calls
   `add_fee_growth_global(zero_for_one, unswapped, last_nonzero_liquidity)`.
3. This converts the surplus into fee_growth_delta and adds it to the global fee accumulator.
4. LPs whose positions cover the last active tick receive the surplus proportionally to their
   liquidity, claimable via the standard `claim_position_fees` flow.

This design incentivizes LPs to provide liquidity at wider/extreme tick ranges to capture
surplus from partial swaps. Unswapped tokens are NOT added to reserves — they are tracked
entirely through fee_growth and claimed by LPs.

**Interface (Extensions — direct pool calls)**:
```rust
fn deposit_position(sender, tick_lower, tick_upper, desired_amounts, min_liquidity) -> (Vec<u128>, u128)
fn withdraw_position(owner, tick_lower, tick_upper, amount, min_amounts) -> Vec<u128>
fn claim_position_fees(owner, tick_lower, tick_upper) -> Vec<u128>
fn claim_all_position_fees(owner) -> Vec<u128>
fn get_slot0() -> Slot0                    // current sqrt_price and tick
fn get_position(owner, tick_lower, tick_upper) -> PositionData
fn tick_from_amounts(amount0, amount1) -> i32  // helper for frontends
```

**Interface (Router-compatible — full-range)**:
```rust
fn deposit(user, desired_amounts, min_shares) -> (Vec<u128>, u128)
fn withdraw(user, share_amount, min_amounts) -> Vec<u128>
fn swap(user, in_idx, out_idx, in_amount, out_min) -> u128
fn swap_strict_receive(user, in_idx, out_idx, out_amount, in_max) -> u128
```

### liquidity_pool_plane

**Purpose**: Lightweight external storage for pool metadata

**Why it exists**:
- Reduces cross-contract call costs
- Enables batch queries for multiple pools
- Updated atomically with every pool action

**Interface**:
```rust
fn update(pool, pool_type, init_args, reserves)  // Called by pools
fn get(pools: Vec<Address>) -> Vec<(Symbol, Vec<u128>, Vec<u128>)>
```

**Data Structure**:
```rust
struct PoolPlane {
    pool_type: Symbol,      // "constant_product" or "stable"
    init_args: Vec<u128>,   // [fee] for constant product, [fee, A, decimals...] for stableswap
    reserves: Vec<u128>,
}
```

### liquidity_pool_liquidity_calculator

**Purpose**: Batch liquidity calculation for multiple pools

**Interface**:
```rust
fn init_admin(account: Address)
fn set_pools_plane(admin, plane)
fn get_pools_plane() -> Address
fn get_liquidity(pools: Vec<Address>) -> Vec<U256>
```

**Calculation Logic**:
- For standard pools: Sum of liquidity for both swap directions
- For stableswap pools: Uses invariant D calculation
- Reads pool data from plane for efficiency

### batcher

**Purpose**: Multicall contract — executes multiple contract calls in a single atomic transaction.

**Interface**:
```rust
fn batch(
    auth_users: Vec<Address>,       // addresses that must authorize the batch
    batch: Vec<(Address, Symbol, Vec<Val>)>,  // [(contract, fn_name, args), ...]
    return_result: bool,            // whether to collect and return results
) -> Vec<Val>
```

**Key Properties**:
- All calls execute atomically — if any call fails, the entire batch reverts
- Requires authorization from all `auth_users` before execution
- Results are optionally collected and returned as `Vec<Val>`

**Use Cases**:
- Multi-claim: batch reward claims across multiple pools in one transaction
- Atomic upgrade + validation (combined with guard contract)
- Any multi-step operation that must succeed or fail as a unit

### guard

**Purpose**: Return-value validator — invokes a contract call and asserts the result matches an expected value.

**Interface**:
```rust
fn assert_result(
    auth_users: Vec<Address>,    // addresses that must authorize
    contract: Address,           // target contract
    fn_name: Symbol,             // function to call
    args: Vec<Val>,              // function arguments
    expected_result: Val,        // expected return value
) -> Val                         // actual result (if assertion passes)
```

**Error Codes**:
```rust
ResultsMismatch = 101,   // actual != expected
UnsupportedType = 102,   // ScVal type not supported for comparison
InvalidValue = 103,      // type extraction failed
TypesMismatch = 104,     // comparing values of different types
```

**Key Properties**:
- Deep recursive comparison supporting nested `Vec`, `Map`, scalars, `Address`, `Symbol`, etc.
- Panics with `ResultsMismatch` if the actual result differs from expected
- Type-safe: panics with `TypesMismatch` if comparing incompatible types

**Combined Batcher + Guard Pattern**:
```
// Atomic upgrade with post-upgrade validation:
batcher.batch(
    [admin],
    [
        (pool, "upgrade", [new_wasm_hash]),
        (guard, "assert_result", [[], pool, "get_version", [], expected_version]),
        (guard, "assert_result", [[], pool, "get_type", [], expected_type]),
    ],
    false,
)
// If upgrade breaks version or type → guard panics → entire batch reverts
```

---

## Shared Libraries

### access_control

**Roles** (defined in `role.rs`):
```rust
pub enum Role {
    Admin,              // Full control, delayed transfer
    EmergencyAdmin,     // Delayed transfer, emergency mode
    RewardsAdmin,       // Configure rewards
    OperationsAdmin,    // Pool parameters, remove pools
    PauseAdmin,         // Pause/unpause operations
    EmergencyPauseAdmin,// Pause only (multiple addresses allowed)
    SystemFeeAdmin,     // Protocol fee configuration
}
```

**Role Properties**:
- `has_many_users()` - Only `EmergencyPauseAdmin` can have multiple addresses
- `is_transfer_delayed()` - Only `Admin` and `EmergencyAdmin` have delayed transfer

**Key Traits**:
- `AccessControlTrait` - Role checking
- `SingleAddressManagementTrait` - Single-address roles
- `MultipleAddressesManagementTrait` - Multi-address roles (EmergencyPauseAdmin)
- `TransferOwnershipTrait` - Delayed ownership transfer

**Usage Pattern**:
```rust
use access_control::access::{AccessControl, AccessControlTrait};
use access_control::utils::require_operations_admin_or_owner;

fn some_admin_function(e: Env, admin: Address) {
    admin.require_auth();
    require_operations_admin_or_owner(&e, &admin);
    // ... perform action
}
```

### rewards

**Core Components**:
- `Manager` - Orchestrates reward calculation
- `Storage` - Reward data persistence
- `BoostManagerPlugin` - Locked token boost calculation
- `OptOutManagerPlugin` - User opt-out from rewards

**Key Concepts**:
- **TPS (Tokens Per Second)**: Reward distribution rate
- **Working Balance**: User's effective share considering boost
- **Working Supply**: Total effective shares in pool
- **Checkpoint**: Snapshot user's reward state at a point in time

**Reward Data Structures**:
```rust
struct PoolRewardConfig {
    tps: u128,         // Tokens per second
    expired_at: u64,   // Expiration timestamp
}

struct PoolRewardData {
    block: u64,        // Checkpoint block
    accumulated: u128, // Total accumulated rewards
    claimed: u128,     // Total claimed rewards
    last_time: u64,    // Last update timestamp
}

struct UserRewardData {
    accumulated: u128,    // User's accumulated rewards
    last_block: u64,      // Last checkpoint block
    to_claim: u128,       // Pending claimable amount
    last_reward_index: u128, // Reward index at last checkpoint
}
```

### token_share

**LP Token Management**:
```rust
pub fn get_token_share(e) -> Address
pub fn put_token_share(e, contract)
pub fn get_user_balance_shares(e, user) -> u128
pub fn get_total_shares(e) -> u128
pub fn mint_shares(e, to, amount)
pub fn burn_shares(e, from, amount)
```

**Implementation Note**: Uses imported SEP-0041 token contract (`soroban_token_contract.wasm`)

### upgrade

**Upgrade Flow**:
```rust
pub fn commit_upgrade(e, new_wasm_hash)  // Starts delay timer
pub fn apply_upgrade(e) -> BytesN<32>     // Applies after delay
pub fn revert_upgrade(e)                  // Cancels pending upgrade
```

**Constants**:
- `UPGRADE_DELAY` - Time before upgrade can be applied (3 days = 259200 seconds)

---

## Access Control & Roles

### Role Hierarchy

```
Admin (Owner)
├── Can do everything
├── Upgrade contracts
├── Transfer ownership (delayed)
└── Manage all privileged addresses

EmergencyAdmin
├── Set emergency mode (bypass upgrade delay)
└── Cannot be transferred instantly

RewardsAdmin
└── Configure rewards (TPS, expiration, votes)

OperationsAdmin
├── Remove pools
├── Ramp A (stableswap)
├── Commit/apply fees (stableswap)
└── Update locker feed total supply

PauseAdmin
├── Pause deposits/swaps/claims
└── Unpause deposits/swaps/claims

EmergencyPauseAdmin (multiple allowed)
└── Pause only (no unpause)

SystemFeeAdmin
└── Set protocol fee fraction
```

### Killswitch System

Each pool has three independent kill switches:
- `kill_deposit` / `unkill_deposit` - Stop/resume deposits
- `kill_swap` / `unkill_swap` - Stop/resume swaps
- `kill_claim` / `unkill_claim` - Stop/resume reward claims

Additionally for gauges:
- `kill_gauges_claim` / `unkill_gauges_claim` - Stop/resume gauge reward claims

### Privileged Address Management

```rust
fn set_privileged_addrs(
    e: Env,
    admin: Address,              // EmergencyAdmin
    rewards_admin: Address,
    operations_admin: Address,
    pause_admin: Address,
    emergency_pause_admins: Vec<Address>,  // Multiple allowed
    system_fee_admin: Address,
)

fn get_privileged_addrs(e: Env) -> Map<Symbol, Vec<Address>>
```

---

## Key Data Flows

### Deposit Flow

```
User → Router.deposit(tokens, pool_index, amounts, min_shares)
       │
       ▼
     Pool.deposit(user, amounts, min_shares)
       │
       ├── require_auth(user)
       ├── Check !is_killed_deposit
       ├── _sync_reserves()
       ├── Checkpoint rewards (before balance change)
       ├── Calculate shares to mint
       ├── Transfer tokens from user → pool
       ├── mint_shares(user, shares)
       ├── Update reserves
       ├── update_plane()
       ├── Checkpoint working balance (after balance change)
       └── Emit deposit_liquidity event
```

### Swap Flow

```
User → Router.swap(tokens, token_in, token_out, pool_index, in_amount, out_min)
       │
       ▼
     Pool.swap(user, in_idx, out_idx, in_amount, out_min)
       │
       ├── require_auth(user)
       ├── Check !is_killed_swap
       ├── _sync_reserves()
       ├── Calculate out_amount (with fee)
       ├── Calculate protocol_fee portion
       ├── Transfer token_in: user → pool
       ├── Transfer token_out: pool → user
       ├── Update reserves & protocol_fees
       ├── update_plane()
       └── Emit trade event
```

### Rewards Distribution Flow

```
Admin → Router.config_global_rewards(tps, expired_at, tokens_votes)
        │
        └── Stores global config, marks tokens for reward

Admin → Router.fill_liquidity(tokens)  // For each token set
        │
        └── Aggregates liquidity from all pools for tokens

Admin → Router.config_pool_rewards(tokens, pool_index)
        │
        ├── Calculates pool's share of total rewards
        └── Calls pool.set_rewards_config(expired_at, pool_tps)

User  → Pool.claim(user) or Router.claim(user, tokens, pool_index)
        │
        ├── Checkpoint user rewards
        ├── Calculate claimable amount
        ├── Transfer reward tokens to user
        └── Reset user's to_claim
```

### Multi-hop Swap Flow

```
User → Router.swap_chained(swaps_chain, token_in, in_amount, out_min)
       │
       ├── require_auth(user)
       ├── For each swap in chain:
       │   ├── Get pool from (pool_tokens, pool_index)
       │   ├── Execute swap with current in_amount
       │   └── Use output as next swap's input
       ├── Validate final output >= out_min
       └── Return final output amount
```

---

## Storage Patterns

### Instance Storage
Used for contract-level persistent data that should extend instance TTL:
```rust
e.storage().instance().set(&DataKey::SomeKey, &value);
e.storage().instance().get(&DataKey::SomeKey).unwrap_or(default);
```

### Persistent Storage
Used for user-specific or infrequently accessed data:
```rust
e.storage().persistent().set(&DataKey::UserData(user), &data);
```

### Temporary Storage
Used for temporary data within a transaction (rarely used):
```rust
e.storage().temporary().set(&key, &value);
```

### Common DataKey Patterns
```rust
#[contracttype]
enum DataKey {
    // Singleton keys
    TokenA,
    TokenB,
    ReserveA,
    ReserveB,
    TotalShares,
    FeeConfig,
    
    // Keyed by address
    UserRewardData(Address),
    WorkingBalance(Address),
    
    // Keyed by composite
    PoolIndex(Vec<Address>, BytesN<32>),
}
```

---

## Rewards System

### Reward Components

1. **Global AQUA Rewards** (via Router):
   - Admin configures `(tps, expired_at, tokens_votes)`
   - Distributed proportionally to pool liquidity
   - Uses `rewards` library within each pool

2. **Gauge Rewards** (third-party):
   - `rewards_gauge` contract per reward token
   - Multiple configs supported (up to `MAX_REWARD_CONFIGS = 5`)
   - Scheduled by distributors with token transfer

3. **Embedded Pool Gauges** (`liquidity_pool_reward_gauge`):
   - Module embedded in pool contracts
   - Manages multiple gauges per pool (up to `MAX_GAUGES = 10`)
   - Checkpointed on every user action

### Boost Mechanism

Users can boost their rewards by locking tokens:
- `locker_feed` provides total locked supply
- `BoostManagerPlugin` calculates boosted working balance
- Boost factor up to 2.5x based on lock ratio

**Working Balance Formula**:
```rust
// Simplified boost calculation
working_balance = min(
    user_share + (total_share * boost_balance / total_locked * 0.6),
    user_share * 2.5
)
```

### Opt-Out Feature

Users can opt out of AQUA rewards:
- `OptOutManagerPlugin` tracks excluded shares
- Useful for protocols/contracts that don't want rewards
- Reduces working supply for remaining users

```rust
fn set_rewards_state(e, user, state: bool)  // User toggles own state
fn admin_set_rewards_state(e, admin, user, state: bool)  // Admin override
fn get_rewards_state(e, user) -> bool
```

### Gauge Reward Scheduling

```rust
fn pool_gauge_schedule_reward(
    distributor,           // Must transfer reward tokens
    pool_tokens,           // Pool token pair
    pool_hash,             // Pool index
    reward_token,          // Token to distribute
    tps,                   // Tokens per second
    start_at,              // Optional start time (default: now)
    duration,              // Duration in seconds
    swaps_chain_proof,     // Proof that reward_token can be swapped to AQUA
) -> Address              // Returns gauge address
```

---

## Error Handling

### Error Code Ranges

| Range | Category | Crate |
|-------|----------|-------|
| 1xx | Access Control | `access_control` |
| 2xx | Pool Operations (shared) | `liquidity_pool`, `liquidity_pool_stableswap`, `liquidity_pool_concentrated` |
| 3xx | Router Operations | `liquidity_pool_router` |
| 20xx | Validation Errors (shared) | `liquidity_pool_validation_errors` |
| 21xx | Concentrated Pool Specific | `liquidity_pool_concentrated` |
| 29xx | Stableswap Specific | `liquidity_pool_stableswap` |

### Access Control Errors
```rust
AdminAlreadySet = 101,
AdminNotSet = 102,
BadRoleUsage = 103,
UnauthorizedRole = 104,
// ...
```

### Pool Errors (201-210)
```rust
AlreadyInitialized = 201,
PlaneAlreadyInitialized = 202,
RewardsAlreadyInitialized = 203,
InvariantDoesNotHold = 204,
PoolDepositKilled = 205,
PoolSwapKilled = 206,
PoolClaimKilled = 207,
FutureShareIdNotSet = 208,
MaxIterationsReached = 209,  // Stableswap only
ZeroTokenNotAllowed = 210,   // Stableswap only
```

### Router Errors (301-321)
```rust
PoolNotFound = 301,
BadFee = 302,
StableswapHashMissing = 303,
PoolsOverMax = 305,
StableswapPoolsOverMax = 306,
PathIsEmpty = 307,
TokensAreNotForReward = 308,
LiquidityNotFilled = 309,
LiquidityAlreadyFilled = 310,
VotingShareExceedsMax = 311,
LiquidityCalculationError = 312,
RewardsNotConfigured = 313,
RewardsAlreadyConfigured = 314,
DuplicatesNotAllowed = 315,
InvalidPoolType = 316,
RewardDurationTooShort = 317,
RewardAmountTooLow = 318,
GaugeRewardsDisabledForPool = 319,
UnsupportedTokensNum = 320,
PathMustEndWithRewardToken = 321,
TokensNotSorted = 2002,
InMaxNotSatisfied = 2020,
```

### Validation Errors (2001-2020)
```rust
WrongInputVecSize = 2001,
FeeOutOfBounds = 2003,
AllCoinsRequired = 2004,
InMinNotSatisfied = 2005,
OutMinNotSatisfied = 2006,
CannotSwapSameToken = 2007,
InTokenOutOfBounds = 2008,
OutTokenOutOfBounds = 2009,
EmptyPool = 2010,
InvalidDepositAmount = 2011,
AdminFeeOutOfBounds = 2012,
UnknownPoolType = 2013,
ZeroSharesBurned = 2014,
TooManySharesBurned = 2015,
CannotComparePools = 2017,
ZeroAmount = 2018,
InsufficientBalance = 2019,
InMaxNotSatisfied = 2020,
```

### Concentrated Pool Errors (21xx)
```rust
InvalidTickRange = 2101,
InvalidAmount = 2103,
InvalidSqrtPrice = 2104,
InvalidTickSpacing = 2106,
TickOutOfBounds = 2107,
PriceOutOfBounds = 2108,
TickNotSpacedCorrectly = 2109,
TickLowerNotLessThanUpper = 2110,
TickLowerTooLow = 2111,
TickUpperTooHigh = 2112,
InvalidPriceLimit = 2113,
PositionNotFound = 2118,
TooManyPositions = 2119,
LiquidityAmountTooLarge = 2120,
InsufficientLiquidity = 2121,
LiquidityOverflow = 2122,
LiquidityUnderflow = 2123,
```

### Stableswap Specific Errors (2902-2908)
```rust
RampTooEarly = 2902,
RampTimeLessThanMinimum = 2903,
RampOverMax = 2904,
RampTooFast = 2905,
AnotherActionActive = 2906,
NoActionActive = 2907,
ActionNotReadyYet = 2908,
```

---

## Upgrade Mechanism

### Delayed Upgrade Process

```rust
// 1. Commit upgrade (starts delay timer)
fn commit_upgrade(e, admin, new_wasm_hash)
    // Stores deadline = now + UPGRADE_DELAY (3 days)

// 2. Wait for UPGRADE_DELAY

// 3. Apply upgrade (after delay)
fn apply_upgrade(e, admin)
    // If emergency_mode: skip delay check
    // Updates contract WASM

// Optional: Cancel pending upgrade
fn revert_upgrade(e, admin)
```

### Emergency Mode

```rust
fn set_emergency_mode(e, emergency_admin, true)
// Allows immediate upgrade without delay
// Only EmergencyAdmin can set
```

### Multi-component Upgrades

Pools store future WASM hashes for sub-components:
```rust
fn commit_upgrade(
    e,
    admin,
    new_wasm_hash,           // Pool contract
    new_token_wasm_hash,     // LP token contract
    gauges_new_wasm_hash,    // Gauge contracts
)

fn apply_upgrade(e, admin) -> (BytesN<32>, BytesN<32>)
    // Returns (pool_wasm_hash, token_wasm_hash)
    // Upgrades gauges via liquidity_pool_reward_gauge::operations::upgrade()
```

---

## Tooling & Commands

### Prerequisites

- [Task](https://taskfile.dev/) - Task runner
- Rust 1.92+ with `wasm32v1-none` target
- [Stellar CLI](https://github.com/stellar/stellar-cli)

### Global Commands

```bash
# Build all contracts (with optimization)
task build

# Run all tests
task test

# Format all code
task fmt
```

### Per-Crate Commands

Each crate with a `Taskfile.yaml` supports:
```bash
# Build specific crate
task -d liquidity_pool build

# Test specific crate
task -d liquidity_pool test

# Format specific crate
task -d liquidity_pool fmt
```

### Contract Compilation

```bash
# Builds and optimizes to contracts/*.wasm
task build

# Individual contract
cd liquidity_pool && cargo build --release --target wasm32v1-none
```

---

## Development Guidelines

### Code Style

1. **Run `task fmt` before committing**
2. **Explicit error handling** - Use `panic_with_error!` with typed errors
3. **Minimize storage operations** - Batch reads/writes for Soroban costs
4. **Use shared helpers** from `utils` and `access_control`
5. **Code comments must be in English only** - Do not add non-English comments in source code

### Adding Dependencies

1. Add to workspace `Cargo.toml` `[workspace.dependencies]`
2. Reference in crate with `workspace = true`:
```toml
[dependencies]
some-dep = { workspace = true }
```

### Modifying Contract Interfaces

When changing a contract's public interface:
1. Update trait definitions in `*_interface.rs`
2. Update implementation in `contract.rs`
3. Update corresponding events in `liquidity_pool_events`
4. Update error codes if needed in `liquidity_pool_validation_errors`
5. Update integration tests in `integration_tests`
6. Rebuild contracts: `task build`

### Storage Best Practices

```rust
// ✅ Good: Batch operations
let (reserve_a, reserve_b) = get_reserves(e);
// process...
put_reserves(e, new_a, new_b);

// ❌ Bad: Multiple individual reads
let reserve_a = get_reserve_a(e);
let reserve_b = get_reserve_b(e);
```

### Event Emission

Use standardized events from `liquidity_pool_events`:
```rust
use liquidity_pool_events::{Events as PoolEvents, LiquidityPoolEvents};

PoolEvents::new(&e).deposit_liquidity(tokens, amounts, share_amount);
PoolEvents::new(&e).trade(user, token_in, token_out, in_amount, out_amount, fee_amount);
```

### Authorization Pattern

Always require auth and check role:
```rust
fn admin_function(e: Env, admin: Address, ...) {
    admin.require_auth();
    AccessControl::new(&e).assert_address_has_role(&admin, &Role::Admin);
    // or use utility
    require_operations_admin_or_owner(&e, &admin);
    // ... implementation
}
```

---

## Testing

### Test Structure

```
crate/
├── src/
│   ├── test.rs           # Main test module
│   ├── test_permissions.rs # Permission-related tests
│   └── testutils.rs      # Test helpers
└── test_snapshots/       # Soroban test snapshots
```

### Running Tests

```bash
# All tests
task test

# Specific crate
task -d liquidity_pool test

# Integration tests (cross-contract)
task -d integration_tests test

# With release optimizations (faster)
cargo test -p integration-tests --release
```

### Test Utilities

Each contract provides `testutils.rs` with helpers:
```rust
// Example from liquidity_pool/testutils.rs
pub fn create_token_contract(e: &Env, admin: &Address) -> token::Client
pub fn install_liquidity_pool(e: &Env) -> BytesN<32>
pub fn create_pool(...) -> LiquidityPoolClient
```

### Integration Tests

Located in `integration_tests/src/`:
- Tests cross-contract interactions
- Full workflow testing (create pool → deposit → swap → withdraw)
- Rewards distribution testing

---

## Common Modification Scenarios

### Adding a New Admin Role

1. Add role to `access_control/src/role.rs`:
```rust
pub enum Role {
    // ...existing roles
    NewRole,
}
```

2. Implement `has_many_users()` and `is_transfer_delayed()` for the role

3. Add symbol representation in `SymbolRepresentation` impl

4. Create utility function in `access_control/src/utils.rs`:
```rust
pub fn require_new_role_or_owner(e: &Env, address: &Address) {
    let access_control = AccessControl::new(e);
    if !access_control.check_address_has_role(address, &Role::NewRole)
        && !access_control.check_address_has_role(address, &Role::Admin)
    {
        panic_with_error!(e, AccessControlError::UnauthorizedRole);
    }
}
```

### Adding a New Pool Parameter

1. Add storage key in `storage.rs`:
```rust
#[contracttype]
enum DataKey {
    // ...existing
    NewParameter,
}

pub fn get_new_parameter(e: &Env) -> u32 { ... }
pub fn put_new_parameter(e: &Env, value: u32) { ... }
```

2. Update `initialize` to accept parameter

3. Update `get_info()` to return parameter

4. Update plane data if affects routing

### Adding a New Validation Error

1. Add to `liquidity_pool_validation_errors/src/lib.rs`:
```rust
pub enum LiquidityPoolValidationError {
    // ...existing
    NewError = 2021,
}
```

2. Use in contract:
```rust
use liquidity_pool_validation_errors::LiquidityPoolValidationError;

if invalid_condition {
    panic_with_error!(e, LiquidityPoolValidationError::NewError);
}
```

### Creating a New Pool Type

1. Create new crate: `liquidity_pool_newtype/`
2. Implement required traits:
   - `LiquidityPoolTrait` or equivalent
   - `UpgradeableContract`
   - `TransferableContract`
   - `RewardsTrait` (if supports rewards)
3. Add to router's pool deployment logic in `pool_utils.rs`
4. Add to liquidity calculator's liquidity estimation
5. Add new pool type constant to plane interface
6. Add integration tests

### Adding a New Event

1. Add trait method to `liquidity_pool_events/src/lib.rs`:
```rust
pub trait LiquidityPoolEvents {
    // ...existing
    fn new_event(&self, param1: Type1, param2: Type2);
}
```

2. Implement the event:
```rust
impl LiquidityPoolEvents for Events {
    fn new_event(&self, param1: Type1, param2: Type2) {
        let e = self.env();
        e.events().publish(
            (Symbol::new(e, "new_event"), param1),
            param2,
        );
    }
}
```

---

## Key Constants Reference

### Fee Constants

```rust
// liquidity_pool
FEE_MULTIPLIER = 10000  // 1 = 0.01%
CONSTANT_PRODUCT_FEE_AVAILABLE = [10, 30, 100]  // 0.1%, 0.3%, 1%

// liquidity_pool_stableswap
FEE_DENOMINATOR = 10000
STABLESWAP_MAX_FEE = 5000  // 50%
```

### Pool Limits

```rust
STABLESWAP_MAX_TOKENS = 4
MAX_A = 1_000_000
MAX_A_CHANGE = 10
MIN_RAMP_TIME = 86400  // 1 day
```

### Rewards Constants

```rust
REWARD_PRECISION = 1_000_000_000_000  // 1e12
MAX_REWARD_CONFIGS = 5  // per gauge
MAX_GAUGES = 10  // per pool
```

### Upgrade Delay

```rust
ADMIN_ACTIONS_DELAY = 259200  // 3 days (in access_control/constants.rs)
UPGRADE_DELAY = 259200  // 3 days (in upgrade/constants.rs)
```

---

## Quick Reference: File Locations

| Need to modify... | Look at... |
|-------------------|------------|
| Pool math (constant product) | `liquidity_pool/src/pool.rs` |
| Pool math (stableswap) | `liquidity_pool_stableswap/src/contract.rs` |
| Router logic | `liquidity_pool_router/src/contract.rs` |
| Access control | `access_control/src/` |
| Rewards calculation | `rewards/src/manager.rs` |
| Error codes | `liquidity_pool_validation_errors/src/lib.rs` |
| Events | `liquidity_pool_events/src/lib.rs` |
| LP token logic | `token_share/src/lib.rs` |
| Upgrade logic | `upgrade/src/lib.rs` |
| Gauge management | `liquidity_pool_reward_gauge/src/operations.rs` |
| Integration tests | `integration_tests/src/` |

---

## Contract Addresses and Deployment

### Pool Salt Generation

Pools are identified by deterministic salts:

**Standard Pool**:
```rust
// Salt = hash(token_a, token_b, fee)
fn get_standard_pool_salt(tokens: &Vec<Address>, fee: u32) -> BytesN<32>
```

**Stableswap Pool**:
```rust
// Salt = hash(sorted_tokens, "stableswap")
fn get_stableswap_pool_salt(tokens: &Vec<Address>) -> BytesN<32>
```

### Token Sorting

**CRITICAL**: Tokens must always be sorted before pool operations:
```rust
fn assert_tokens_sorted(e: &Env, tokens: &Vec<Address>)
// Panics with TokensNotSorted if not sorted
```

---

## Security Considerations

### Re-entrancy Protection

- Rewards checkpoint BEFORE balance changes
- Working balance checkpoint AFTER balance changes
- LP token uses `checkpoint_reward` callback to pool

### Slippage Protection

- `min_shares` on deposits
- `out_min` on swaps
- `min_amounts` on withdrawals
- `in_max` on strict receive swaps
- `max_burn_amount` on imbalanced withdrawals

### Flash Loan Resistance

- No flash loan functionality
- All operations require auth and actual token transfers

### Oracle Independence

- No external price oracles for pool math
- `locker_feed` is admin-updated, not real-time
