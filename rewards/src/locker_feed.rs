mod locker {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_locker_feed_contract.wasm"
    );
}
pub use locker::Client as LockerFeedClient;
