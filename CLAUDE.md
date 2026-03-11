# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

Aquarius AMM — a Rust workspace of Soroban smart contracts for the Aquarius decentralized exchange on Stellar blockchain. The workspace contains 28 crates: deployable contracts (WASM), shared libraries, and integration tests. AQUA governance token powers rewards distribution and DAO voting.

**Stack:** Rust 1.92+ / Soroban SDK 25.0.2 / WebAssembly (wasm32v1-none) target

## Build & Test Commands

[Task](https://taskfile.dev/) is the task runner. Each contract crate has its own `Taskfile.yaml`.

```bash
task build                              # Build all contracts (includes fmt + stellar contract optimize)
task test                               # Run all test suites
task fmt                                # Format all code (cargo fmt per subdirectory)
task all                                # clean → fmt → build → check_bindings → test
task check_bindings                     # Validate upgrade/transfer interface bindings on built WASMs

task -d liquidity_pool build            # Build a single contract
task -d liquidity_pool test             # Test a single contract
task -d liquidity_pool_stableswap test  # Test stableswap

cargo test -p soroban-liquidity-pool-contract                 # Run tests for a single crate by package name
cargo test -p soroban-liquidity-pool-contract -- test_deposit  # Run a single test
cargo test -p integration-tests --release                      # Integration tests (faster with --release)
```

Note: `task build` only builds a subset of top-level contracts (router, provider_swap_fee, fees_collector, rewards_gauge) since other contracts are built as dependencies. Individual crates can be built with `task -d <crate> build`.

## Architecture

```
User/Frontend
      │
      ▼
liquidity_pool_router ──────── Entry point: pool factory, deposits, swaps,
      │                         withdrawals, multi-hop routing, rewards config
      │
      ├── liquidity_pool              (xy=k, 2 tokens, fee tiers: 0.1%/0.3%/1%)
      ├── liquidity_pool_stableswap   (Curve-style, 2-4 tokens, amp coeff A)
      ├── liquidity_pool_concentrated (Uniswap v3-style, tick-based, WIP on feature branch)
      │
      ├── liquidity_pool_plane        (lightweight pool metadata store, updated on every action)
      └── liquidity_pool_liquidity_calculator (batch liquidity queries from plane data)
```

**Support contracts:** `fees_collector`, `locker_feed` (locked supply oracle for boost), `config_storage`, `rewards_gauge`, `token` (SEP-0041 LP token)

**Shared libraries (not deployed):** `access_control`, `rewards`, `token_share`, `upgrade`, `utils`, `liquidity_pool_events`, `liquidity_pool_validation_errors`, `liquidity_pool_reward_gauge`, `liquidity_pool_config_storage`

### Key Patterns

- **Router proxies all user operations** to specific pool contracts. Pools are identified by `(sorted_tokens, pool_index_hash)`.
- **Tokens must always be sorted** before pool operations (`assert_tokens_sorted`).
- **Pool plane** is updated atomically on every deposit/swap/withdraw for efficient batch queries.
- **Rewards checkpoint** happens BEFORE balance changes; working balance checkpoint happens AFTER.
- **Upgrade mechanism** has a 3-day delay (`UPGRADE_DELAY = 259200s`), bypassable via emergency mode.
- **Kill switches** per pool: independent pause for deposit, swap, claim, and gauge claims.

### Dependencies

Workspace dependencies are defined in root `Cargo.toml` `[workspace.dependencies]`. Crates reference them with `workspace = true`. When adding a dependency, add to workspace first.

## Error Code Ranges

| Range | Category |
|-------|----------|
| 1xx | Access Control |
| 2xx | Pool Operations |
| 3xx | Router Operations |
| 20xx | Validation Errors (`liquidity_pool_validation_errors`) |
| 29xx | StableSwap Specific |

## Modifying Contract Interfaces

1. Update trait in `*_interface.rs`
2. Update implementation in `contract.rs`
3. Update events in `liquidity_pool_events` if applicable
4. Update error codes in `liquidity_pool_validation_errors` if applicable
5. Update integration tests in `integration_tests`
6. Rebuild: `task build`

## Code Conventions

- Run `task fmt` before committing (CI enforces zero diff after formatting)
- Use `panic_with_error!` with typed error enums, not raw panics
- Minimize storage operations — batch reads/writes (Soroban execution costs)
- Comments in English only
- Authorization pattern: `admin.require_auth()` then `AccessControl::new(&e).assert_address_has_role(...)` or use helpers like `require_operations_admin_or_owner`
- Events use `liquidity_pool_events`: `PoolEvents::new(&e).deposit_liquidity(...)`, `PoolEvents::new(&e).trade(...)`

## Development Workflow

- **Work incrementally**: make a small change, verify it compiles/passes tests, then continue. Prefer a sequence of small validated edits over one large change. Do not attempt to rewrite or restructure large portions of a codebase in a single step.
- **Follow existing conventions**: before modifying a file, read it first and understand the code style, patterns, and imports used. When creating new modules or tests, look at existing ones in the same crate and mimic the approach.
- **Check library availability**: never assume a dependency is available. Before using a new crate, check `Cargo.toml` (workspace and local) to confirm it's already in use. If it's new, add it to workspace dependencies first.
- **Concurrent agents**: if you notice unexpected changes in the worktree or staging area that you did not make, ignore them and continue with your task. Multiple agents or the user may work in the codebase concurrently.
- **Verify after changes**: after modifying contract code, run `cargo test -p <package-name>` (or at minimum a compile check) to confirm nothing is broken before moving on.
- **Check regression at the end**: check implementation doesn't break existing functionality by running complete tests suite: `task test`

## CI

GitHub Actions (`.github/workflows/rust.yml`) runs on PRs to master/develop/audit: format check → bindings check → all tests. Uses `soroban-cli` v23.3.0.

## Reference

See `AGENTS.md` for exhaustive architecture documentation including data flows, storage patterns, rewards system details, boost mechanism, and common modification scenarios with code examples.
