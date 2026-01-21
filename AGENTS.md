# AGENTS

## Repository overview
- Rust workspace of Soroban smart contracts for the Aquarius AMM on Stellar.
- Contract crates live in sibling directories; compiled WASM artifacts are checked in.

## Layout
- Contract crates: `liquidity_pool`, `liquidity_pool_stableswap`, `liquidity_pool_router`, `rewards`, `token`, `fees_collector`, `locker_feed`, `rewards_gauge`, `liquidity_pool_reward_gauge`, `config_storage`, `liquidity_pool_config_storage`.
- Shared crates: `utils`, `access_control`, `liquidity_pool_events`, `liquidity_pool_validation_errors`, `upgrade`, `liquidity_pool_plane`, `liquidity_pool_liquidity_calculator`.
- Tests: `integration_tests`.
- Generated artifacts: `contracts/` (`*.wasm`, do not edit by hand).

## Tooling and commands
- Toolchain is pinned in `rust-toolchain.toml` (Rust 1.92, target `wasm32v1-none`).
- Use the Taskfile runner:
  - `task build` builds and optimizes core contracts.
  - `task test` runs tests across all crates.
  - `task fmt` runs rustfmt across all crates.
  - Per crate: `task -d <crate> build|test|fmt|check`.
- Prefer `soroban contract build` via Taskfiles for WASM builds.

## Development notes
- Keep `contracts/*.wasm` in sync only when requested; builds will update these files.
- When modifying contract interfaces, update related event/error crates and integration tests.
- Prefer shared helpers in `utils` and `access_control` rather than duplicating logic.
- Add dependencies to workspace `Cargo.toml` and use `workspace = true` in crate manifests.

## Code style and testing
- Run `task fmt` before finishing changes.
- Favor explicit error handling and minimize storage reads/writes for Soroban costs.
- Run targeted crate tests; use `task -d integration_tests test` for cross-contract flows.
