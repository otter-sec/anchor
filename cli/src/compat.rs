//! Solana dependencies used by the v2 CLI, kept under the same import paths as master.

#[cfg(not(windows))]
pub use solana_compute_budget;
pub use {
    solana_cli_config, solana_client, solana_clock, solana_loader_v3_interface, solana_message,
    solana_packet, solana_pubkey, solana_pubsub_client, solana_rpc_client, solana_rpc_client_api,
    solana_transaction, solana_transaction_status_client_types,
};

#[cfg(not(windows))]
pub fn default_compute_budget() -> solana_compute_budget::compute_budget::ComputeBudget {
    solana_compute_budget::compute_budget::ComputeBudget::new_with_defaults(false, false)
}
