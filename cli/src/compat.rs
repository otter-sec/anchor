//! Aliases the active Solana cohort back to its unsuffixed crate name.
//!
//! See `anchor_lang::compat` for the full rationale. The CLI is a binary and
//! has no downstream API to protect, but Cargo unifies features across the
//! workspace, so it has to build as the same cohort as the library crates.

#[cfg(all(feature = "v3", feature = "v4"))]
compile_error!(
    "anchor-cli: features `v3` and `v4` are mutually exclusive, but both are enabled. Build the \
     workspace as a single cohort, e.g. `--no-default-features --features v4`."
);

#[cfg(not(any(feature = "v3", feature = "v4")))]
compile_error!("anchor-cli: enable exactly one of the `v3` or `v4` features.");

// `anchor test --profile` / `anchor debugger` only, and non-Windows only.
#[cfg(all(feature = "v3", not(windows)))]
pub use solana_compute_budget_v3 as solana_compute_budget;
#[cfg(all(feature = "v4", not(windows)))]
pub use solana_compute_budget_v4 as solana_compute_budget;
#[cfg(feature = "v3")]
pub use {
    solana_cli_config_v3 as solana_cli_config, solana_client_v3 as solana_client,
    solana_clock_v3 as solana_clock, solana_loader_v3_interface_v3 as solana_loader_v3_interface,
    solana_message_v3 as solana_message, solana_packet_v3 as solana_packet,
    solana_pubkey_v3 as solana_pubkey, solana_pubsub_client_v3 as solana_pubsub_client,
    solana_rpc_client_api_v3 as solana_rpc_client_api, solana_rpc_client_v3 as solana_rpc_client,
    solana_transaction_status_client_types_v3 as solana_transaction_status_client_types,
    solana_transaction_v3 as solana_transaction,
};
#[cfg(feature = "v4")]
pub use {
    solana_cli_config_v4 as solana_cli_config, solana_client_v4 as solana_client,
    solana_clock_v4 as solana_clock, solana_loader_v3_interface_v4 as solana_loader_v3_interface,
    solana_message_v4 as solana_message, solana_packet_v4 as solana_packet,
    solana_pubkey_v4 as solana_pubkey, solana_pubsub_client_v4 as solana_pubsub_client,
    solana_rpc_client_api_v4 as solana_rpc_client_api, solana_rpc_client_v4 as solana_rpc_client,
    solana_transaction_status_client_types_v4 as solana_transaction_status_client_types,
    solana_transaction_v4 as solana_transaction,
};

/// A default [`ComputeBudget`] for trace cost attribution.
///
/// `new_with_defaults` lost its second parameter between the cohorts, so the
/// arity is pinned here rather than at each of the five call sites.
///
/// Gated on `not(windows)` like the alias above: the profiling and debugger
/// commands this serves are non-Windows only, and naming the type in the
/// signature would otherwise fail to resolve on Windows.
///
/// [`ComputeBudget`]: solana_compute_budget::compute_budget::ComputeBudget
#[cfg(not(windows))]
pub fn default_compute_budget() -> solana_compute_budget::compute_budget::ComputeBudget {
    #[cfg(feature = "v3")]
    return solana_compute_budget::compute_budget::ComputeBudget::new_with_defaults(false, false);
    #[cfg(feature = "v4")]
    solana_compute_budget::compute_budget::ComputeBudget::new_with_defaults(false)
}
