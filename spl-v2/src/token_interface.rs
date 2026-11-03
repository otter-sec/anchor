//! Interface account types that accept both Token and Token-2022 programs.
//!
//! Provides `TokenAccount` and `Mint` aliases for use with
//! `anchor_lang::prelude::InterfaceAccount`, accepting either
//! `TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA` (Token) or
//! `TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb` (Token-2022).
//!
//! # Usage
//!
//! ```ignore
//! use anchor_lang::prelude::InterfaceAccount;
//! use anchor_spl::token_interface::{Mint, TokenAccount};
//!
//! #[derive(Accounts)]
//! pub struct MyAccounts {
//!     #[account(token::mint = mint, token::authority = owner)]
//!     pub token_account: InterfaceAccount<TokenAccount>,
//!     pub mint: InterfaceAccount<Mint>,
//! }
//! ```

pub use crate::{
    token_2022::{PermanentDelegateInitialize, *},
    token_2022_extensions::*,
};
use {
    anchor_lang::{
        accounts::{InterfaceAccount, SlabInit, SlabSchema},
        programs::{Token, Token2022 as Token2022Program},
        require, require_eq, AccountConstraint, AnchorAccount, Id, Ids,
    },
    bytemuck::{Pod, Zeroable},
    core::ops::Deref,
    pinocchio::account::AccountView,
    solana_address::Address,
    solana_program_error::ProgramError,
    spl_token_2022_interface::{
        extension::{
            BaseStateWithExtensions, ExtensionType as Token2022ExtensionType,
            PodStateWithExtensions,
        },
        pod::{PodAccount, PodMint},
    },
};

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

/// Extension reader for Token-2022 interface mint and token accounts.
///
/// This keeps TLV parsing on the account wrapper, where the underlying
/// [`AccountView`] is available, while preserving Token-2022 owner and
/// extension-family checks.
pub trait TokenInterfaceAccountExtensions {
    fn get_extension<T: crate::extensions::ExtensionType>(&self) -> Result<&T, ProgramError>;
}

impl TokenInterfaceAccountExtensions for InterfaceAccount<Mint> {
    #[inline(always)]
    fn get_extension<T: crate::extensions::ExtensionType>(&self) -> Result<&T, ProgramError> {
        let account = self.account();
        require!(
            account.owned_by(&Token2022Program::id()),
            ProgramError::IllegalOwner
        );

        let data = unsafe { account.borrow_unchecked() };
        let state = PodStateWithExtensions::<PodMint>::unpack(data)?;
        let extension = state.get_extension::<T>()?;
        let extension_ptr = extension as *const T;

        // SAFETY: `PodStateWithExtensions` stores only references into `data`,
        // and `extension_ptr` points into that account data, not into the
        // temporary wrapper value. `data` is borrowed from `account`, which
        // outlives the returned reference.
        Ok(unsafe { &*extension_ptr })
    }
}

impl TokenInterfaceAccountExtensions for InterfaceAccount<TokenAccount> {
    #[inline(always)]
    fn get_extension<T: crate::extensions::ExtensionType>(&self) -> Result<&T, ProgramError> {
        let account = self.account();
        require!(
            account.owned_by(&Token2022Program::id()),
            ProgramError::IllegalOwner
        );

        let data = unsafe { account.borrow_unchecked() };
        let state = PodStateWithExtensions::<PodAccount>::unpack(data)?;
        let extension = state.get_extension::<T>()?;
        let extension_ptr = extension as *const T;

        // SAFETY: `PodStateWithExtensions` stores only references into `data`,
        // and `extension_ptr` points into that account data, not into the
        // temporary wrapper value. `data` is borrowed from `account`, which
        // outlives the returned reference.
        Ok(unsafe { &*extension_ptr })
    }
}

/// Program marker that accepts both Token and Token-2022 executable accounts.
pub struct TokenInterface;

impl Ids for TokenInterface {
    #[inline(always)]
    fn ids() -> &'static [Address] {
        static IDS: [Address; 2] = [
            anchor_lang::address!("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA"),
            anchor_lang::address!("TokenzQdBNbLqP5VEhdkAS6EPFLC1PHnBqCXEpPxuEb"),
        ];
        &IDS
    }
}

// ---------------------------------------------------------------------------
// SlabSchema — Interface<TokenAccount>
// ---------------------------------------------------------------------------

impl SlabSchema for Interface<crate::TokenAccount> {
    const DATA_OFFSET: usize = 0;
    const MIN_DATA_LEN: usize = core::mem::size_of::<Self>();

    #[inline(always)]
    fn validate(view: &AccountView, data: &[u8]) -> Result<(), ProgramError> {
        require!(
            view.owned_by(&Token::id()) || view.owned_by(&Token2022Program::id()),
            ProgramError::IllegalOwner
        );
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
    const MIN_DATA_LEN: usize = core::mem::size_of::<Self>();

    #[inline(always)]
    fn validate(view: &AccountView, data: &[u8]) -> Result<(), ProgramError> {
        require!(
            view.owned_by(&Token::id()) || view.owned_by(&Token2022Program::id()),
            ProgramError::IllegalOwner
        );
        PodStateWithExtensions::<PodMint>::unpack(data)?;
        crate::mint::validate_mint_initialized(data)?;
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Space
// ---------------------------------------------------------------------------

impl anchor_lang::Space for Interface<crate::TokenAccount> {
    const INIT_SPACE: usize = core::mem::size_of::<crate::TokenAccount>();
}

impl anchor_lang::Space for Interface<crate::Mint> {
    const INIT_SPACE: usize = core::mem::size_of::<crate::Mint>();
}

// ---------------------------------------------------------------------------
// IDL — keep interface types out of the user's types[] array
// ---------------------------------------------------------------------------

#[doc(hidden)]
impl anchor_lang::IdlAccountType for Interface<crate::TokenAccount> {}

#[doc(hidden)]
impl anchor_lang::IdlAccountType for Interface<crate::Mint> {}

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
        params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
        payer_signer_seeds: Option<&[&[u8]]>,
    ) -> Result<(), ProgramError> {
        let mint = params.mint.ok_or(ProgramError::InvalidArgument)?;
        let authority = params.authority.ok_or(ProgramError::InvalidArgument)?;
        let token_program = params.token_program.ok_or(ProgramError::InvalidArgument)?;
        let program_id = token_program.address();
        crate::token_shared::validate_token_interface_program(program_id)?;

        let space = token_account_init_space(mint, program_id)?;
        anchor_lang::create_account_with_signers(
            payer,
            account,
            space,
            program_id,
            signer_seeds,
            payer_signer_seeds,
        )?;

        pinocchio_token_2022::instructions::InitializeAccount3 {
            account,
            mint,
            owner: authority.address(),
            token_program: program_id,
        }
        .invoke()
    }
}

#[inline(always)]
fn token_account_init_space(
    mint: &AccountView,
    token_program: &Address,
) -> Result<usize, ProgramError> {
    if !anchor_lang::address_eq(token_program, &Token2022Program::id()) {
        return Ok(core::mem::size_of::<crate::TokenAccount>());
    }

    let mint_data = unsafe { mint.borrow_unchecked() };
    let mint_state = PodStateWithExtensions::<PodMint>::unpack(mint_data)?;
    let mint_extensions = mint_state.get_extension_types()?;
    let required_extensions =
        Token2022ExtensionType::get_required_init_account_extensions(&mint_extensions);

    Token2022ExtensionType::try_calculate_account_len::<PodAccount>(&required_extensions)
}

// ---------------------------------------------------------------------------
// SlabInit — Interface<Mint>
// ---------------------------------------------------------------------------

/// Init params for `InterfaceAccount<Mint>`.
///
/// `extensions::*` fields are Token-2022 only. They are initialized after
/// `create_account` and **before** `InitializeMint2`.
#[derive(Default)]
pub struct InterfaceMintInitParams<'a> {
    pub decimals: Option<u8>,
    pub authority: Option<&'a AccountView>,
    pub freeze_authority: Option<&'a AccountView>,
    pub token_program: Option<&'a AccountView>,
    pub metadata_pointer_authority: Option<Address>,
    pub metadata_pointer_metadata_address: Option<Address>,
    pub group_pointer_authority: Option<Address>,
    pub group_pointer_group_address: Option<Address>,
    pub group_member_pointer_authority: Option<Address>,
    pub group_member_pointer_member_address: Option<Address>,
    pub close_authority_authority: Option<Address>,
    pub transfer_hook_authority: Option<Address>,
    pub transfer_hook_program_id: Option<Address>,
    pub permanent_delegate_delegate: Option<Address>,
}

impl InterfaceMintInitParams<'_> {
    fn has_extensions(&self) -> bool {
        self.metadata_pointer_authority.is_some()
            || self.metadata_pointer_metadata_address.is_some()
            || self.group_pointer_authority.is_some()
            || self.group_pointer_group_address.is_some()
            || self.group_member_pointer_authority.is_some()
            || self.group_member_pointer_member_address.is_some()
            || self.close_authority_authority.is_some()
            || self.transfer_hook_authority.is_some()
            || self.transfer_hook_program_id.is_some()
            || self.permanent_delegate_delegate.is_some()
    }

    fn extension_types(&self) -> ([Token2022ExtensionType; 6], usize) {
        let mut types = [Token2022ExtensionType::Uninitialized; 6];
        let mut n = 0usize;
        let mut push = |ext: Token2022ExtensionType| {
            types[n] = ext;
            n += 1;
        };
        if self.group_pointer_authority.is_some() || self.group_pointer_group_address.is_some() {
            push(Token2022ExtensionType::GroupPointer);
        }
        if self.group_member_pointer_authority.is_some()
            || self.group_member_pointer_member_address.is_some()
        {
            push(Token2022ExtensionType::GroupMemberPointer);
        }
        if self.metadata_pointer_authority.is_some()
            || self.metadata_pointer_metadata_address.is_some()
        {
            push(Token2022ExtensionType::MetadataPointer);
        }
        if self.close_authority_authority.is_some() {
            push(Token2022ExtensionType::MintCloseAuthority);
        }
        if self.transfer_hook_authority.is_some() || self.transfer_hook_program_id.is_some() {
            push(Token2022ExtensionType::TransferHook);
        }
        if self.permanent_delegate_delegate.is_some() {
            push(Token2022ExtensionType::PermanentDelegate);
        }
        (types, n)
    }
}

fn mint_extension_space(params: &InterfaceMintInitParams<'_>) -> Result<usize, ProgramError> {
    let (types, n) = params.extension_types();
    if n == 0 {
        return Ok(core::mem::size_of::<crate::Mint>());
    }
    Token2022ExtensionType::try_calculate_account_len::<PodMint>(&types[..n])
}

impl SlabInit for Interface<crate::Mint> {
    type Params<'a> = InterfaceMintInitParams<'a>;

    #[cold]
    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        space: usize,
        params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
        payer_signer_seeds: Option<&[&[u8]]>,
    ) -> Result<(), ProgramError> {
        let decimals = params.decimals.ok_or(ProgramError::InvalidArgument)?;
        let authority = params.authority.ok_or(ProgramError::InvalidArgument)?;
        let token_program = params.token_program.ok_or(ProgramError::InvalidArgument)?;
        let program_id = token_program.address();
        crate::token_shared::validate_token_interface_program(program_id)?;

        require!(
            space >= core::mem::size_of::<crate::Mint>(),
            ProgramError::AccountDataTooSmall
        );
        if params.has_extensions() {
            require!(
                anchor_lang::address_eq(program_id, &Token2022Program::id()),
                ProgramError::IncorrectProgramId
            );
        }

        let space = core::cmp::max(space, mint_extension_space(params)?);
        anchor_lang::create_account_with_signers(
            payer,
            account,
            space,
            program_id,
            signer_seeds,
            payer_signer_seeds,
        )?;

        initialize_mint_extensions(account, program_id, params)?;

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

fn initialize_mint_extensions(
    mint: &AccountView,
    token_program: &Address,
    params: &InterfaceMintInitParams<'_>,
) -> Result<(), ProgramError> {
    if params.group_pointer_authority.is_some() || params.group_pointer_group_address.is_some() {
        pinocchio_token_2022::instructions::group_pointer::Initialize {
            mint,
            authority: params.group_pointer_authority.as_ref(),
            group_address: params.group_pointer_group_address.as_ref(),
            token_program,
        }
        .invoke()?;
    }
    if params.group_member_pointer_authority.is_some()
        || params.group_member_pointer_member_address.is_some()
    {
        pinocchio_token_2022::instructions::group_member_pointer::Initialize {
            mint,
            authority: params.group_member_pointer_authority.as_ref(),
            member_address: params.group_member_pointer_member_address.as_ref(),
            token_program,
        }
        .invoke()?;
    }
    if params.metadata_pointer_authority.is_some()
        || params.metadata_pointer_metadata_address.is_some()
    {
        pinocchio_token_2022::instructions::metadata_pointer::Initialize {
            mint,
            authority: params.metadata_pointer_authority.as_ref(),
            metadata_address: params.metadata_pointer_metadata_address.as_ref(),
            token_program,
        }
        .invoke()?;
    }
    if let Some(close_authority) = params.close_authority_authority.as_ref() {
        pinocchio_token_2022::instructions::mint_close_authority::InitializeMintCloseAuthority {
            mint,
            close_authority: Some(close_authority),
            token_program,
        }
        .invoke()?;
    }
    if params.transfer_hook_authority.is_some() || params.transfer_hook_program_id.is_some() {
        pinocchio_token_2022::instructions::transfer_hook::InitializeTransferHook {
            mint,
            authority: params.transfer_hook_authority.as_ref(),
            program_id: params.transfer_hook_program_id.as_ref(),
            token_program,
        }
        .invoke()?;
    }
    if let Some(delegate) = params.permanent_delegate_delegate.as_ref() {
        pinocchio_token_2022::instructions::permanent_delegate::InitializePermanentDelegate {
            mint,
            delegate,
            token_program,
        }
        .invoke()?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Constraint impls — token::* on InterfaceAccount<TokenAccount>
// ---------------------------------------------------------------------------

impl AccountConstraint<InterfaceAccount<TokenAccount>> for crate::token::MintConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(
        account: &InterfaceAccount<TokenAccount>,
        expected: &Address,
    ) -> Result<(), ProgramError> {
        require!(
            anchor_lang::address_eq(account.mint(), expected),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }
}

impl AccountConstraint<InterfaceAccount<TokenAccount>> for crate::token::AuthorityConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(
        account: &InterfaceAccount<TokenAccount>,
        expected: &Address,
    ) -> Result<(), ProgramError> {
        require!(
            anchor_lang::address_eq(account.owner(), expected),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }
}

impl AccountConstraint<InterfaceAccount<TokenAccount>> for crate::token::TokenProgramConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(
        account: &InterfaceAccount<TokenAccount>,
        expected: &Address,
    ) -> Result<(), ProgramError> {
        require!(
            AsRef::<AccountView>::as_ref(account).owned_by(expected),
            ProgramError::IllegalOwner
        );
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Constraint impls — mint::* on InterfaceAccount<Mint>
// ---------------------------------------------------------------------------

impl AccountConstraint<InterfaceAccount<Mint>> for crate::mint::AuthorityConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(account: &InterfaceAccount<Mint>, expected: &Address) -> Result<(), ProgramError> {
        require_eq!(
            account.mint_authority(),
            Some(expected),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }
}

impl AccountConstraint<InterfaceAccount<Mint>> for crate::mint::FreezeAuthorityConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(account: &InterfaceAccount<Mint>, expected: &Address) -> Result<(), ProgramError> {
        require_eq!(
            account.freeze_authority(),
            Some(expected),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }
}

impl AccountConstraint<InterfaceAccount<Mint>> for crate::mint::DecimalsConstraint {
    type Value = u8;
    #[inline(always)]
    fn check(account: &InterfaceAccount<Mint>, expected: &u8) -> Result<(), ProgramError> {
        require_eq!(
            account.decimals(),
            *expected,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }
}

impl AccountConstraint<InterfaceAccount<Mint>> for crate::mint::TokenProgramConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(account: &InterfaceAccount<Mint>, expected: &Address) -> Result<(), ProgramError> {
        require!(
            AsRef::<AccountView>::as_ref(account).owned_by(expected),
            ProgramError::IllegalOwner
        );
        Ok(())
    }
}

macro_rules! impl_mint_extension_constraint {
    ($constraint:ty, $ext:ty, $field:ident) => {
        impl AccountConstraint<InterfaceAccount<Mint>> for $constraint {
            type Value = Address;
            #[inline(always)]
            fn check(
                account: &InterfaceAccount<Mint>,
                expected: &Address,
            ) -> Result<(), ProgramError> {
                let ext = account.get_extension::<$ext>()?;
                require_eq!(
                    crate::extensions::optional_address(&ext.$field),
                    Some(expected),
                    ProgramError::InvalidAccountData
                );
                Ok(())
            }
        }
    };
}

impl_mint_extension_constraint!(
    crate::extensions::MetadataPointerAuthorityConstraint,
    crate::extensions::MetadataPointer,
    authority
);
impl_mint_extension_constraint!(
    crate::extensions::MetadataPointerMetadataAddressConstraint,
    crate::extensions::MetadataPointer,
    metadata_address
);
impl_mint_extension_constraint!(
    crate::extensions::GroupPointerAuthorityConstraint,
    crate::extensions::GroupPointer,
    authority
);
impl_mint_extension_constraint!(
    crate::extensions::GroupPointerGroupAddressConstraint,
    crate::extensions::GroupPointer,
    group_address
);
impl_mint_extension_constraint!(
    crate::extensions::GroupMemberPointerAuthorityConstraint,
    crate::extensions::GroupMemberPointer,
    authority
);
impl_mint_extension_constraint!(
    crate::extensions::GroupMemberPointerMemberAddressConstraint,
    crate::extensions::GroupMemberPointer,
    member_address
);
impl_mint_extension_constraint!(
    crate::extensions::CloseAuthorityAuthorityConstraint,
    crate::extensions::MintCloseAuthority,
    close_authority
);
impl_mint_extension_constraint!(
    crate::extensions::TransferHookAuthorityConstraint,
    crate::extensions::TransferHook,
    authority
);
impl_mint_extension_constraint!(
    crate::extensions::TransferHookProgramIdConstraint,
    crate::extensions::TransferHook,
    program_id
);
impl_mint_extension_constraint!(
    crate::extensions::PermanentDelegateDelegateConstraint,
    crate::extensions::PermanentDelegate,
    delegate
);
