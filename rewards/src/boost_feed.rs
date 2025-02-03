mod boost_feed {
    soroban_sdk::contractimport!(
        file = "../target/wasm32-unknown-unknown/release/soroban_locker_feed_contract.wasm"
    );
}
pub use boost_feed::Client as RewardBoostFeedClient;
