//! Account types that can be used in the account validation struct.

pub mod account;
pub mod account_info;
pub mod account_loader;
pub mod boxed;
pub mod interface;
pub mod interface_account;
pub mod migration;
pub mod option;
pub mod program;
pub mod signer;
pub mod system_account;
pub mod sysvar;
pub mod unchecked_account;

#[cfg(feature = "lazy-account")]
pub mod lazy_account;
