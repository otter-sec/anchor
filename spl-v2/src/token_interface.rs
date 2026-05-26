//! Interface account types that accept both Token and Token-2022 programs.
//!
//! Provides `InterfaceAccount<T>` — a type alias for accounts that can be
//! owned by either `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` (Token) or
//! `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` (Token-2022).
//!
//! # Usage
//!
//! ```ignore
//! use anchor_lang_v2::prelude::InterfaceAccount;
//! use anchor_spl_v2::token_interface::{Mint, TokenAccount};
//!
//! #[derive(Accounts)]
//! pub struct MyAccounts {
//!     #[account(token::mint = mint, token::authority = owner)]
//!     pub token_account: InterfaceAccount<TokenAccount>,
//!     pub mint: InterfaceAccount<Mint>,
//! }
//! ```

use {
    anchor_lang_v2::{
        accounts::{InterfaceAccount, SlabInit, SlabSchema},
        programs::{Token, Token2022 as Token2022Program},
        AccountConstraint, Id, Ids,
    },
    bytemuck::{Pod, Zeroable},
    core::ops::Deref,
    pinocchio::account::AccountView,
    solana_address::Address,
    solana_program_error::ProgramError,
    spl_token_2022_interface::{
        extension::PodStateWithExtensions,
        pod::{PodAccount, PodMint},
    },
};

pub use crate::token_2022::PermanentDelegateInitialize;
pub use crate::token_2022::*;
pub use crate::token_2022_extensions::*;

// ---------------------------------------------------------------------------
// Interface<T> — transparent wrapper that changes validation to accept both
// Token and Token-2022 program ownership.
// ---------------------------------------------------------------------------

/// Transparent wrapper around an SPL type `T` that relaxes ownership
/// validation to accept both the Token and Token-2022 programs.
///
/// Users should not reference this type directly — use `InterfaceAccount<T>`.
#[repr(transparent)]
#[derive(Clone, Copy)]
pub struct Interface<T>(T);

// SAFETY: Interface<T> is #[repr(transparent)] over T.
// If T is Pod+Zeroable, so is Interface<T>.
unsafe impl<T: Pod> Pod for Interface<T> {}
unsafe impl<T: Zeroable> Zeroable for Interface<T> {}

impl<T> Deref for Interface<T> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        &self.0
    }
}

/// SPL token account data used with `InterfaceAccount<TokenAccount>`.
pub type TokenAccount = Interface<crate::TokenAccount>;

/// SPL mint account data used with `InterfaceAccount<Mint>`.
pub type Mint = Interface<crate::Mint>;

/// Program marker that accepts both Token and Token-2022 executable accounts.
pub struct TokenInterface;

impl Ids for TokenInterface {
    #[inline(always)]
    fn ids() -> &'static [Address] {
        static IDS: [Address; 2] = [
            Address::from_str_const("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
            Address::from_str_const("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
        ];
        &IDS
    }
}

// ---------------------------------------------------------------------------
// SlabSchema — Interface<TokenAccount>
// ---------------------------------------------------------------------------

impl SlabSchema for Interface<crate::TokenAccount> {
    const DATA_OFFSET: usize = 0;

    #[inline(always)]
    fn validate(
        view: &AccountView,
        data: &[u8],
        _program_id: &Address,
    ) -> Result<(), ProgramError> {
        if !view.owned_by(&Token::id()) && !view.owned_by(&Token2022Program::id()) {
            return Err(ProgramError::IllegalOwner);
        }
        PodStateWithExtensions::<PodAccount>::unpack(data)?;
        crate::token::validate_token_account_initialized(data)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// SlabSchema — Interface<Mint>
// ---------------------------------------------------------------------------

impl SlabSchema for Interface<crate::Mint> {
    const DATA_OFFSET: usize = 0;

    #[inline(always)]
    fn validate(
        view: &AccountView,
        data: &[u8],
        _program_id: &Address,
    ) -> Result<(), ProgramError> {
        if !view.owned_by(&Token::id()) && !view.owned_by(&Token2022Program::id()) {
            return Err(ProgramError::IllegalOwner);
        }
        PodStateWithExtensions::<PodMint>::unpack(data)?;
        crate::mint::validate_mint_initialized(data)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Space
// ---------------------------------------------------------------------------

impl anchor_lang_v2::Space for Interface<crate::TokenAccount> {
    const INIT_SPACE: usize = core::mem::size_of::<crate::TokenAccount>();
}

impl anchor_lang_v2::Space for Interface<crate::Mint> {
    const INIT_SPACE: usize = core::mem::size_of::<crate::Mint>();
}

// ---------------------------------------------------------------------------
// IDL — keep interface types out of the user's types[] array
// ---------------------------------------------------------------------------

#[doc(hidden)]
impl anchor_lang_v2::IdlAccountType for Interface<crate::TokenAccount> {}

#[doc(hidden)]
impl anchor_lang_v2::IdlAccountType for Interface<crate::Mint> {}

#[cfg(feature = "guardrails")]
#[inline(always)]
fn validate_token_program(program_id: &Address) -> Result<(), ProgramError> {
    if !anchor_lang_v2::address_eq(program_id, &Token::id())
        && !anchor_lang_v2::address_eq(program_id, &Token2022Program::id())
    {
        return Err(ProgramError::IncorrectProgramId);
    }
    Ok(())
}

#[cfg(not(feature = "guardrails"))]
#[inline(always)]
fn validate_token_program(_program_id: &Address) -> Result<(), ProgramError> {
    Ok(())
}

// ---------------------------------------------------------------------------
// SlabInit — Interface<TokenAccount>
// ---------------------------------------------------------------------------

/// Init params for `InterfaceAccount<TokenAccount>`. Requires `token_program`
/// to know which program to create the account through.
#[derive(Default)]
pub struct InterfaceTokenAccountInitParams<'a> {
    pub mint: Option<&'a AccountView>,
    pub authority: Option<&'a AccountView>,
    pub token_program: Option<&'a AccountView>,
}

impl SlabInit for Interface<crate::TokenAccount> {
    type Params<'a> = InterfaceTokenAccountInitParams<'a>;

    #[cold]
    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        _space: usize,
        _program_id: &Address,
        params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<(), ProgramError> {
        let mint = params.mint.ok_or(ProgramError::InvalidArgument)?;
        let authority = params.authority.ok_or(ProgramError::InvalidArgument)?;
        let token_program = params.token_program.ok_or(ProgramError::InvalidArgument)?;
        let program_id = token_program.address();
        validate_token_program(program_id)?;

        let space = core::mem::size_of::<crate::TokenAccount>();
        match signer_seeds {
            Some(seeds) => {
                anchor_lang_v2::create_account_signed(payer, account, space, program_id, seeds)
            }
            None => anchor_lang_v2::create_account(payer, account, space, program_id),
        }?;

        pinocchio_token_2022::instructions::InitializeAccount3 {
            account,
            mint,
            owner: authority.address(),
            token_program: program_id,
        }
        .invoke()
    }
}

// ---------------------------------------------------------------------------
// SlabInit — Interface<Mint>
// ---------------------------------------------------------------------------

/// Init params for `InterfaceAccount<Mint>`.
#[derive(Default)]
pub struct InterfaceMintInitParams<'a> {
    pub decimals: Option<u8>,
    pub authority: Option<&'a AccountView>,
    pub freeze_authority: Option<&'a AccountView>,
    pub token_program: Option<&'a AccountView>,
}

impl SlabInit for Interface<crate::Mint> {
    type Params<'a> = InterfaceMintInitParams<'a>;

    #[cold]
    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        _space: usize,
        _program_id: &Address,
        params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<(), ProgramError> {
        let decimals = params.decimals.ok_or(ProgramError::InvalidArgument)?;
        let authority = params.authority.ok_or(ProgramError::InvalidArgument)?;
        let token_program = params.token_program.ok_or(ProgramError::InvalidArgument)?;
        let program_id = token_program.address();
        validate_token_program(program_id)?;

        let space = core::mem::size_of::<crate::Mint>();
        match signer_seeds {
            Some(seeds) => {
                anchor_lang_v2::create_account_signed(payer, account, space, program_id, seeds)
            }
            None => anchor_lang_v2::create_account(payer, account, space, program_id),
        }?;

        pinocchio_token_2022::instructions::InitializeMint2 {
            mint: account,
            decimals,
            mint_authority: authority.address(),
            freeze_authority: params.freeze_authority.map(|v| v.address()),
            token_program: program_id,
        }
        .invoke()
    }
}

// ---------------------------------------------------------------------------
// Constraint impls — token::* on InterfaceAccount<TokenAccount>
// ---------------------------------------------------------------------------

macro_rules! impl_token_account_constraints {
    ($target:ty) => {
        impl AccountConstraint<$target> for crate::token::MintConstraint {
            type Value = Address;
            #[inline(always)]
            fn check(account: &$target, expected: &Address) -> Result<(), ProgramError> {
                if !anchor_lang_v2::address_eq(account.mint(), expected) {
                    Err(ProgramError::InvalidAccountData)
                } else {
                    Ok(())
                }
            }
        }

        impl AccountConstraint<$target> for crate::token::AuthorityConstraint {
            type Value = Address;
            #[inline(always)]
            fn check(account: &$target, expected: &Address) -> Result<(), ProgramError> {
                if !anchor_lang_v2::address_eq(account.owner(), expected) {
                    Err(ProgramError::InvalidAccountData)
                } else {
                    Ok(())
                }
            }
        }

        impl AccountConstraint<$target> for crate::token::TokenProgramConstraint {
            type Value = Address;
            #[inline(always)]
            fn check(account: &$target, expected: &Address) -> Result<(), ProgramError> {
                if !AsRef::<AccountView>::as_ref(account).owned_by(expected) {
                    Err(ProgramError::IllegalOwner)
                } else {
                    Ok(())
                }
            }
        }
    };
}

impl_token_account_constraints!(InterfaceAccount<TokenAccount>);

// ---------------------------------------------------------------------------
// Constraint impls — mint::* on InterfaceAccount<Mint>
// ---------------------------------------------------------------------------

macro_rules! impl_mint_constraints {
    ($target:ty) => {
        impl AccountConstraint<$target> for crate::mint::AuthorityConstraint {
            type Value = Address;
            #[inline(always)]
            fn check(account: &$target, expected: &Address) -> Result<(), ProgramError> {
                match account.mint_authority() {
                    Some(addr) if addr == expected => Ok(()),
                    _ => Err(ProgramError::InvalidAccountData),
                }
            }
        }

        impl AccountConstraint<$target> for crate::mint::FreezeAuthorityConstraint {
            type Value = Address;
            #[inline(always)]
            fn check(account: &$target, expected: &Address) -> Result<(), ProgramError> {
                match account.freeze_authority() {
                    Some(addr) if addr == expected => Ok(()),
                    _ => Err(ProgramError::InvalidAccountData),
                }
            }
        }

        impl AccountConstraint<$target> for crate::mint::DecimalsConstraint {
            type Value = u8;
            #[inline(always)]
            fn check(account: &$target, expected: &u8) -> Result<(), ProgramError> {
                if account.decimals() != *expected {
                    Err(ProgramError::InvalidAccountData)
                } else {
                    Ok(())
                }
            }
        }

        impl AccountConstraint<$target> for crate::mint::TokenProgramConstraint {
            type Value = Address;
            #[inline(always)]
            fn check(account: &$target, expected: &Address) -> Result<(), ProgramError> {
                if !AsRef::<AccountView>::as_ref(account).owned_by(expected) {
                    Err(ProgramError::IllegalOwner)
                } else {
                    Ok(())
                }
            }
        }
    };
}

impl_mint_constraints!(InterfaceAccount<Mint>);
