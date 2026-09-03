//! Anchor v2 SPL account types and constraint markers.
//!
//! Separate crate (like v1's `anchor-spl`) that provides zero-copy `TokenAccount`
//! and `Mint` types for use with `Account<T>`, plus namespaced constraint markers
//! for `token::mint`, `token::authority`, `mint::decimals`, etc.
//!
//! The `token` CPI helpers intentionally accept either the Token Program or
//! Token-2022 for base token instructions. The `token` account types themselves
//! remain strict Token Program account layouts. For account validation that must
//! accept both Token and Token-2022 owners, use
//! `anchor_lang::prelude::InterfaceAccount` with the `token_interface`
//! module's token account types.

#![no_std]

extern crate alloc;

pub mod associated_token;
pub mod extensions;
#[cfg(feature = "metadata")]
pub mod metadata;
pub mod mint;
pub mod token;
pub mod token_2022;
pub mod token_2022_extensions;
pub mod token_interface;
mod token_shared;

pub mod prelude {
    pub use crate::{
        associated_token, mint, token, token::TokenCpiExt, token_2022, token_2022_extensions,
        token_interface,
    };
}

/// Re-export `pinocchio-token-2022` for Token-2022 extension CPI instructions.
pub use pinocchio_token_2022 as token_2022_cpi;
pub use {
    associated_token::{
        get_associated_token_address, get_associated_token_address_with_program_id,
    },
    mint::{Mint, MintInitParams},
    token::{TokenAccount, TokenAccountInitParams},
    token_interface::Interface,
};
