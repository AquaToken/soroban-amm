# Rewards system review (updated)

Current view of the rewards distribution logic with boost and self-exclusion support.

## Working-balance returns are intentionally stale
- `Manager::update_working_balance` still returns the previous working balance and working supply so `checkpoint_user` consumes
  stale values during the current block. This mirrors the legacy behavior and is now intentional to avoid retroactive boosting
  when lock balances change mid-epoch.【F:rewards/src/manager.rs†L417-L448】
- Impact: reward weighting shifts are applied from the next checkpoint forward rather than immediately. This can delay the effect
  of opt-ins/outs or boost changes by one checkpoint but avoids time-travel reward amplification.

## Opt-out supply tracking now applied
- Self-exclusion updates the global excluded-share total both when toggling reward state and when opted-out users change their
  share balances, keeping `effective_total_share` aligned with who is actually earning rewards.【F:rewards/src/manager.rs†L38-L83】【F:rewards/src/manager.rs†L425-L448】
- Impact: opting out fully removes a user’s stake from supply-based boost math, preventing large LPs who exclude themselves
  from diluting rewards for others. Remaining risk is limited to correctness of callers supplying the true `total_shares` value
  when invoking checkpoints.
