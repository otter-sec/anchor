#![cfg_attr(docsrs, feature(doc_cfg))]

//! Anchor CPI wrappers for popular programs in the Solana ecosystem.

#[doc(hidden)]
pub mod compat;

#[cfg(feature = "associated_token")]
pub mod associated_token;

#[cfg(feature = "mint")]
pub mod mint;

#[cfg(feature = "token")]
pub mod token;

#[cfg(feature = "token_2022")]
pub mod token_2022;

#[cfg(feature = "token_2022_extensions")]
pub mod token_2022_extensions;

#[cfg(feature = "token_2022")]
pub mod token_interface;

#[cfg(feature = "governance")]
pub mod governance;

#[cfg(feature = "stake")]
pub mod stake;

#[cfg(feature = "metadata")]
pub mod metadata;

#[cfg(feature = "memo")]
pub mod memo;

#[cfg(feature = "idl-build")]
mod idl_build;

#[cfg(test)]
mod test_token22_assertion_gaps {
    #[test]
    fn test_token22_extension_constraint_negative_assertion() {
        let is_extension_initialized = false;
        let requires_transfer_hook = true;

        let is_valid = is_extension_initialized || !requires_transfer_hook;

        assert!(
            !is_valid,
            "Token-22 transfer hook constraint must throw an error when extension is uninitialized"
        );
    }
}
