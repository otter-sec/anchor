//! Aliases the active Solana/SPL cohort back to its unsuffixed crate name.
//!
//! These re-exports are `pub` because several are re-exported again from the
//! modules below; they are not part of anchor-spl's supported surface.

#[cfg(all(feature = "v3", feature = "v4"))]
compile_error!(
    "anchor-spl: features `v3` and `v4` are mutually exclusive, but both are enabled. This \
     usually means a dependency re-enabled anchor-spl's default features while something else \
     asked for `v4`. Add `default-features = false` to that dependency, or settle the whole \
     dependency graph on one cohort."
);

#[cfg(not(any(feature = "v3", feature = "v4")))]
compile_error!(
    "anchor-spl: enable exactly one of the `v3` or `v4` features. `v3` is the default, so this \
     usually means `default-features = false` was set without naming a replacement cohort."
);

#[cfg(feature = "v3")]
pub use {
    solana_stake_interface_v3 as solana_stake_interface, solana_sysvar_v3 as solana_sysvar,
    spl_token_2022_interface_v3 as spl_token_2022_interface,
    spl_token_interface_v3 as spl_token_interface,
    spl_token_metadata_interface_v3 as spl_token_metadata_interface,
};
#[cfg(feature = "v4")]
pub use {
    solana_stake_interface_v4 as solana_stake_interface, solana_sysvar_v4 as solana_sysvar,
    spl_token_2022_interface_v4 as spl_token_2022_interface,
    spl_token_interface_v4 as spl_token_interface,
    spl_token_metadata_interface_v4 as spl_token_metadata_interface,
};

/// The "optional pubkey" type stored in token-2022 extension fields.
///
/// `#[derive(Accounts)]` generates constraint checks against these fields and
/// names this path directly, so it has to resolve under both cohorts. A proc
/// macro cannot see which features anchor-spl was built with, so the generated
/// code is written once against `solana_nullable::MaybeNull` and the name is
/// supplied here rather than branched on in the macro.
///
/// v4 re-exports the real `solana-nullable`; v3's token-2022 interface predates
/// it and stores `spl_pod`'s `OptionalNonZeroPubkey`, which is the same shape
/// and supports the same `TryFrom<Option<Pubkey>>` conversion.
#[cfg(all(feature = "v3", feature = "token_2022_extensions"))]
pub mod solana_nullable {
    pub use spl_pod::optional_keys::OptionalNonZeroPubkey as MaybeNull;
}

#[cfg(all(feature = "v4", feature = "token_2022_extensions"))]
pub use solana_nullable;
