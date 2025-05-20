mod boost_feed {
    soroban_sdk::contractimport!(
        file = "../target/wasm32v1-none/release/soroban_locker_feed_contract.wasm"
    );
}
pub use boost_feed::Client as RewardBoostFeedClient;
