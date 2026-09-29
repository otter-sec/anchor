//! Aliases the active Solana cohort back to its unsuffixed crate name.
//!
//! These re-exports are `pub` because several of them are re-exported again
//! from the crate root; they are not part of anchor-client's supported surface.

#[cfg(all(feature = "solana-v3", feature = "solana-v4"))]
compile_error!(
    "anchor-client: features `solana-v3` and `solana-v4` are mutually exclusive, but both are \
     enabled. This usually means a dependency re-enabled anchor-client's default features while \
     something else asked for `solana-v4`. Add `default-features = false` to that dependency, or \
     settle the whole dependency graph on one cohort."
);

#[cfg(not(any(feature = "solana-v3", feature = "solana-v4")))]
compile_error!(
    "anchor-client: enable exactly one of the `solana-v3` or `solana-v4` features. `solana-v3` is \
     the default, so this usually means `default-features = false` was set without naming a \
     replacement cohort."
);

#[cfg(feature = "solana-v3")]
pub use {
    solana_account_decoder_v3 as solana_account_decoder, solana_hash_v3 as solana_hash,
    solana_message_v3 as solana_message, solana_pubsub_client_v3 as solana_pubsub_client,
    solana_rpc_client_api_v3 as solana_rpc_client_api, solana_rpc_client_v3 as solana_rpc_client,
    solana_transaction_v3 as solana_transaction,
};
#[cfg(feature = "solana-v4")]
pub use {
    solana_account_decoder_v4 as solana_account_decoder, solana_hash_v4 as solana_hash,
    solana_message_v4 as solana_message, solana_pubsub_client_v4 as solana_pubsub_client,
    solana_rpc_client_api_v4 as solana_rpc_client_api, solana_rpc_client_v4 as solana_rpc_client,
    solana_transaction_v4 as solana_transaction,
};
