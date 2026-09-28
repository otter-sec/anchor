//! Aliases the active Solana cohort back to its unsuffixed crate name.
//!
//! These re-exports are `pub` only because [`crate::solana_program`] re-exports
//! them publicly; they are not part of Anchor's supported surface.

#[cfg(all(feature = "v3", feature = "v4"))]
compile_error!(
    "anchor-lang: features `v3` and `v4` are mutually exclusive, but both are enabled. This \
     usually means a dependency re-enabled anchor-lang's default features while something else \
     asked for `v4`. Add `default-features = false` to that dependency, or settle the whole \
     dependency graph on one cohort."
);

#[cfg(not(any(feature = "v3", feature = "v4")))]
compile_error!(
    "anchor-lang: enable exactly one of the `v3` or `v4` features. `v3` is the default, so this \
     usually means `default-features = false` was set without naming a replacement cohort."
);

#[cfg(feature = "v3")]
pub use {
    solana_clock_v3 as solana_clock, solana_define_syscall_v3 as solana_define_syscall,
    solana_feature_gate_interface_v3 as solana_feature_gate_interface,
    solana_instructions_sysvar_v3 as solana_instructions_sysvar,
    solana_loader_v3_interface_v3 as solana_loader_v3_interface, solana_pubkey_v3 as solana_pubkey,
    solana_stake_interface_v3 as solana_stake_interface, solana_sysvar_v3 as solana_sysvar,
};
#[cfg(feature = "v4")]
pub use {
    solana_clock_v4 as solana_clock, solana_define_syscall_v4 as solana_define_syscall,
    solana_feature_gate_interface_v4 as solana_feature_gate_interface,
    solana_instructions_sysvar_v4 as solana_instructions_sysvar,
    solana_loader_v3_interface_v4 as solana_loader_v3_interface, solana_pubkey_v4 as solana_pubkey,
    solana_stake_interface_v4 as solana_stake_interface, solana_sysvar_v4 as solana_sysvar,
};
