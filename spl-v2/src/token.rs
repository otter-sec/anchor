//! SPL Token account type with `SlabSchema` impl for use with `Account<T>`.
//!
//! Layout mirrors `pinocchio-token` — all fields are alignment-1 to support
//! zerocopy mapping from the account data buffer.

use {
    anchor_lang_v2::{
        accounts::{Account, Program, SlabInit, SlabSchema},
        require, require_eq, AccountConstraint, CpiContext, Id, ToCpiHandle, ToCpiHandleMut,
    },
    bytemuck::{Pod, Zeroable},
    pinocchio::account::AccountView,
    solana_address::Address,
    solana_program_error::ProgramError,
    spl_token_2022_interface as spl_token_2022,
};

pub use {
    crate::mint::Mint,
    anchor_lang_v2::programs::Token,
    spl_token_interface::{self as spl_token, ID},
};

pub(crate) const COPTION_NONE: [u8; 4] = [0, 0, 0, 0];
pub(crate) const COPTION_SOME: [u8; 4] = [1, 0, 0, 0];

#[inline(always)]
pub(crate) fn coption_is_some(tag: &[u8; 4]) -> bool {
    *tag == COPTION_SOME
}

#[inline(always)]
pub(crate) fn validate_coption_tag(data: &[u8], offset: usize) -> Result<(), ProgramError> {
    match data.get(offset..offset + 4) {
        Some(tag) if tag == COPTION_NONE.as_slice() || tag == COPTION_SOME.as_slice() => Ok(()),
        Some(_) => Err(ProgramError::InvalidAccountData),
        None => Err(ProgramError::InvalidAccountData),
    }
}

pub(crate) fn validate_token_account_initialized(data: &[u8]) -> Result<(), ProgramError> {
    const TOKEN_ACCOUNT_DELEGATE_TAG_OFFSET: usize = 32 + 32 + 8;
    const TOKEN_ACCOUNT_STATE_OFFSET: usize = 32 + 32 + 8 + 4 + 32;
    const TOKEN_ACCOUNT_IS_NATIVE_TAG_OFFSET: usize = TOKEN_ACCOUNT_STATE_OFFSET + 1;
    const TOKEN_ACCOUNT_CLOSE_AUTHORITY_TAG_OFFSET: usize =
        TOKEN_ACCOUNT_IS_NATIVE_TAG_OFFSET + 4 + 8 + 8;

    validate_coption_tag(data, TOKEN_ACCOUNT_DELEGATE_TAG_OFFSET)?;
    validate_coption_tag(data, TOKEN_ACCOUNT_IS_NATIVE_TAG_OFFSET)?;
    validate_coption_tag(data, TOKEN_ACCOUNT_CLOSE_AUTHORITY_TAG_OFFSET)?;

    match data.get(TOKEN_ACCOUNT_STATE_OFFSET).copied() {
        Some(1) | Some(2) => Ok(()),
        Some(0) => Err(ProgramError::UninitializedAccount),
        Some(_) => Err(ProgramError::InvalidAccountData),
        None => Err(ProgramError::InvalidAccountData),
    }
}

/// Create a Token-program-owned account, handling PDA signing if needed.
pub(crate) fn create_token_account(
    payer: &AccountView,
    account: &AccountView,
    space: usize,
    signer_seeds: Option<&[&[u8]]>,
) -> Result<(), ProgramError> {
    let token_program_id = Token::id();
    match signer_seeds {
        Some(seeds) => {
            anchor_lang_v2::create_account_signed(payer, account, space, &token_program_id, seeds)
        }
        None => anchor_lang_v2::create_account(payer, account, space, &token_program_id),
    }
}

/// SPL Token account data, zerocopy-mapped (165 bytes).
///
/// All fields are private — use the accessor methods to read data. Token
/// account state is modified only by the SPL Token program via CPI (Transfer,
/// MintTo, etc.); user programs cannot mutate these fields directly anyway
/// because the account is owned by the SPL Token program.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct TokenAccount {
    mint: Address,
    owner: Address,
    amount: [u8; 8],
    delegate_flag: [u8; 4],
    delegate: Address,
    state: u8,
    is_native_flag: [u8; 4],
    native_amount: [u8; 8],
    delegated_amount: [u8; 8],
    close_authority_flag: [u8; 4],
    close_authority: Address,
}

// SAFETY: TokenAccount is repr(C) with all alignment-1 fields, no padding.
unsafe impl Pod for TokenAccount {}
unsafe impl Zeroable for TokenAccount {}

// TokenAccount is defined by the SPL Token program, not by the user's program
// — its layout is known to any SPL-aware client. Default `__IDL_TYPE_DEF = None`
// keeps it out of the user's IDL `types[]` array (matches v1's
// `impl_idl_build!` behavior for this type).
#[doc(hidden)]
impl anchor_lang_v2::IdlAccountType for TokenAccount {}

// On-chain size — SPL Token program requires 165 bytes. Used by
// `#[account(init, token::*)]` as the default when `space` is omitted.
impl anchor_lang_v2::Space for TokenAccount {
    const INIT_SPACE: usize = core::mem::size_of::<Self>();
}

impl SlabSchema for TokenAccount {
    // External types start at offset 0 — no Anchor discriminator.
    const DATA_OFFSET: usize = 0;
    const MIN_DATA_LEN: usize = core::mem::size_of::<Self>();

    #[inline(always)]
    fn validate(view: &AccountView, data: &[u8]) -> Result<(), ProgramError> {
        require!(view.owned_by(&Token::id()), ProgramError::IllegalOwner);
        // Exact size distinguishes TokenAccount (165) from Mint (82).
        require_eq!(
            data.len(),
            core::mem::size_of::<Self>(),
            ProgramError::InvalidAccountData
        );
        validate_token_account_initialized(data)?;
        Ok(())
    }
}

/// Init params for `#[account(init, token::mint = ..., token::authority = ...)]`.
#[derive(Default)]
pub struct TokenAccountInitParams<'a> {
    pub mint: Option<&'a AccountView>,
    pub authority: Option<&'a AccountView>,
}

impl SlabInit for TokenAccount {
    type Params<'a> = TokenAccountInitParams<'a>;

    #[cold]
    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        _space: usize,
        params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<(), ProgramError> {
        let mint = params.mint.ok_or(ProgramError::InvalidArgument)?;
        let authority = params.authority.ok_or(ProgramError::InvalidArgument)?;

        create_token_account(payer, account, core::mem::size_of::<Self>(), signer_seeds)?;

        pinocchio_token::instructions::InitializeAccount3 {
            account,
            mint,
            owner: authority.address(),
        }
        .invoke()
    }
}

impl TokenAccount {
    pub const LEN: usize = core::mem::size_of::<Self>();

    /// The mint associated with this token account.
    pub fn mint(&self) -> &Address {
        &self.mint
    }

    /// The owner of this token account.
    pub fn owner(&self) -> &Address {
        &self.owner
    }

    /// The token balance.
    pub fn amount(&self) -> u64 {
        u64::from_le_bytes(self.amount)
    }

    /// The amount currently delegated.
    pub fn delegated_amount(&self) -> u64 {
        u64::from_le_bytes(self.delegated_amount)
    }

    /// Whether a delegate is currently approved.
    pub fn has_delegate(&self) -> bool {
        coption_is_some(&self.delegate_flag)
    }

    /// The approved delegate, if any.
    pub fn delegate(&self) -> Option<&Address> {
        if self.has_delegate() {
            Some(&self.delegate)
        } else {
            None
        }
    }

    /// Account state (0 = Uninitialized, 1 = Initialized, 2 = Frozen).
    pub fn state(&self) -> u8 {
        self.state
    }

    /// Whether this is a wrapped SOL account.
    pub fn is_native(&self) -> bool {
        coption_is_some(&self.is_native_flag)
    }

    /// The rent-exempt reserve for native SOL accounts, if this is a native
    /// token account.
    pub fn native_amount(&self) -> Option<u64> {
        if self.is_native() {
            Some(u64::from_le_bytes(self.native_amount))
        } else {
            None
        }
    }

    /// Whether a close authority is set.
    pub fn has_close_authority(&self) -> bool {
        coption_is_some(&self.close_authority_flag)
    }

    /// The close authority, if any.
    pub fn close_authority(&self) -> Option<&Address> {
        if self.has_close_authority() {
            Some(&self.close_authority)
        } else {
            None
        }
    }

    /// Whether the account has been initialized (state != 0).
    pub fn is_initialized(&self) -> bool {
        self.state != 0
    }

    /// Whether the account is frozen (state == 2).
    pub fn is_frozen(&self) -> bool {
        self.state == 2
    }
}

// ---------------------------------------------------------------------------
// Constraint markers for `#[account(token::*)]`
// ---------------------------------------------------------------------------

pub struct MintConstraint;
pub struct AuthorityConstraint;
pub struct TokenProgramConstraint;

impl AccountConstraint<Account<TokenAccount>> for MintConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(account: &Account<TokenAccount>, expected: &Address) -> Result<(), ProgramError> {
        require!(
            anchor_lang_v2::address_eq(account.mint(), expected),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }
}

impl AccountConstraint<Account<TokenAccount>> for AuthorityConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(account: &Account<TokenAccount>, expected: &Address) -> Result<(), ProgramError> {
        require!(
            anchor_lang_v2::address_eq(account.owner(), expected),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }
}

/// `token::token_program = token_program` — check account is owned by given program.
impl AccountConstraint<Account<TokenAccount>> for TokenProgramConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(account: &Account<TokenAccount>, expected: &Address) -> Result<(), ProgramError> {
        require!(
            AsRef::<AccountView>::as_ref(account).owned_by(expected),
            ProgramError::IllegalOwner
        );
        Ok(())
    }
}

/// Account structs consumed by each CPI helper. Re-exported through
/// `token::accounts::*` for compatibility with the v1-style module shape.
pub mod accounts {
    pub use crate::token_shared::{
        Approve, ApproveChecked, Burn, BurnChecked, CloseAccount, FreezeAccount, InitializeAccount,
        InitializeAccount3, InitializeMint, InitializeMint2, MintTo, MintToChecked, Revoke,
        SetAuthority, SyncNative, ThawAccount, Transfer, TransferChecked,
    };
}

pub use crate::token_shared::{
    approve, approve_checked, burn, burn_checked, close_account, freeze_account,
    initialize_account, initialize_account3, initialize_mint, initialize_mint2, mint_to,
    mint_to_checked, revoke, sync_native, thaw_account, transfer, transfer_checked,
};
pub use accounts::{
    Approve, ApproveChecked, Burn, BurnChecked, CloseAccount, FreezeAccount, InitializeAccount,
    InitializeAccount3, InitializeMint, InitializeMint2, MintTo, MintToChecked, Revoke,
    SetAuthority, SyncNative, ThawAccount, Transfer, TransferChecked,
};

#[inline]
fn token_2022_authority_type(
    authority_type: spl_token::instruction::AuthorityType,
) -> spl_token_2022::instruction::AuthorityType {
    match authority_type {
        spl_token::instruction::AuthorityType::MintTokens => {
            spl_token_2022::instruction::AuthorityType::MintTokens
        }
        spl_token::instruction::AuthorityType::FreezeAccount => {
            spl_token_2022::instruction::AuthorityType::FreezeAccount
        }
        spl_token::instruction::AuthorityType::AccountOwner => {
            spl_token_2022::instruction::AuthorityType::AccountOwner
        }
        spl_token::instruction::AuthorityType::CloseAccount => {
            spl_token_2022::instruction::AuthorityType::CloseAccount
        }
    }
}

pub fn set_authority<'a>(
    ctx: CpiContext<'a, accounts::SetAuthority<'a>>,
    authority_type: spl_token::instruction::AuthorityType,
    new_authority: Option<Address>,
) -> Result<(), ProgramError> {
    crate::token_shared::set_authority(
        ctx,
        token_2022_authority_type(authority_type),
        new_authority.as_ref(),
    )
}

/// Ergonomic CPI helpers for the legacy Token program.
///
/// These methods are a thin layer over the existing account structs and free
/// functions. They keep the same account ordering and validation path while
/// letting callers avoid manually building `CpiContext` for simple Token CPIs.
pub trait TokenCpiExt {
    fn mint_to<'a, M, T, A>(
        &'a self,
        mint: &'a mut M,
        to: &'a mut T,
        authority: &'a A,
        amount: u64,
    ) -> Result<(), ProgramError>
    where
        M: ToCpiHandleMut + ?Sized,
        T: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn transfer<'a, F, T, A>(
        &'a self,
        from: &'a mut F,
        to: &'a mut T,
        authority: &'a A,
        amount: u64,
    ) -> Result<(), ProgramError>
    where
        F: ToCpiHandleMut + ?Sized,
        T: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn transfer_checked<'a, F, M, T, A>(
        &'a self,
        from: &'a mut F,
        mint: &'a M,
        to: &'a mut T,
        authority: &'a A,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError>
    where
        F: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        T: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn burn<'a, F, M, A>(
        &'a self,
        from: &'a mut F,
        mint: &'a mut M,
        authority: &'a A,
        amount: u64,
    ) -> Result<(), ProgramError>
    where
        F: ToCpiHandleMut + ?Sized,
        M: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn approve<'a, S, D, A>(
        &'a self,
        source: &'a mut S,
        delegate: &'a D,
        authority: &'a A,
        amount: u64,
    ) -> Result<(), ProgramError>
    where
        S: ToCpiHandleMut + ?Sized,
        D: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn revoke<'a, S, A>(&'a self, source: &'a mut S, authority: &'a A) -> Result<(), ProgramError>
    where
        S: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn close_account<'a, Acc, Dest, A>(
        &'a self,
        account: &'a mut Acc,
        destination: &'a mut Dest,
        authority: &'a A,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        Dest: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn mint_to_checked<'a, M, T, A>(
        &'a self,
        mint: &'a mut M,
        to: &'a mut T,
        authority: &'a A,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError>
    where
        M: ToCpiHandleMut + ?Sized,
        T: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn burn_checked<'a, F, M, A>(
        &'a self,
        from: &'a mut F,
        mint: &'a mut M,
        authority: &'a A,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError>
    where
        F: ToCpiHandleMut + ?Sized,
        M: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn approve_checked<'a, S, M, D, A>(
        &'a self,
        source: &'a mut S,
        mint: &'a M,
        delegate: &'a D,
        authority: &'a A,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError>
    where
        S: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        D: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn freeze_account<'a, Acc, M, A>(
        &'a self,
        account: &'a mut Acc,
        mint: &'a M,
        authority: &'a A,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn thaw_account<'a, Acc, M, A>(
        &'a self,
        account: &'a mut Acc,
        mint: &'a M,
        authority: &'a A,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn sync_native<'a, Acc>(&'a self, account: &'a mut Acc) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized;

    fn initialize_mint<'a, M, R>(
        &'a self,
        mint: &'a mut M,
        rent: &'a R,
        decimals: u8,
        authority: &Address,
        freeze_authority: Option<&Address>,
    ) -> Result<(), ProgramError>
    where
        M: ToCpiHandleMut + ?Sized,
        R: ToCpiHandle + ?Sized;

    fn initialize_mint2<'a, M>(
        &'a self,
        mint: &'a mut M,
        decimals: u8,
        authority: &Address,
        freeze_authority: Option<&Address>,
    ) -> Result<(), ProgramError>
    where
        M: ToCpiHandleMut + ?Sized;

    fn initialize_account<'a, Acc, M, A, R>(
        &'a self,
        account: &'a mut Acc,
        mint: &'a M,
        authority: &'a A,
        rent: &'a R,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized,
        R: ToCpiHandle + ?Sized;

    fn initialize_account3<'a, Acc, M, A>(
        &'a self,
        account: &'a mut Acc,
        mint: &'a M,
        authority: &'a A,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized;

    fn set_authority<'a, Acc, A>(
        &'a self,
        account_or_mint: &'a mut Acc,
        current_authority: &'a A,
        authority_type: spl_token::instruction::AuthorityType,
        new_authority: Option<Address>,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized;
}

impl TokenCpiExt for Program<Token> {
    fn mint_to<'a, M, T, A>(
        &'a self,
        mint: &'a mut M,
        to: &'a mut T,
        authority: &'a A,
        amount: u64,
    ) -> Result<(), ProgramError>
    where
        M: ToCpiHandleMut + ?Sized,
        T: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        mint_to(
            CpiContext::new(
                self.address(),
                accounts::MintTo {
                    mint: mint.try_to_cpi_handle_mut()?,
                    to: to.try_to_cpi_handle_mut()?,
                    authority: authority.to_cpi_handle(),
                },
            ),
            amount,
        )
    }

    fn transfer<'a, F, T, A>(
        &'a self,
        from: &'a mut F,
        to: &'a mut T,
        authority: &'a A,
        amount: u64,
    ) -> Result<(), ProgramError>
    where
        F: ToCpiHandleMut + ?Sized,
        T: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        transfer(
            CpiContext::new(
                self.address(),
                accounts::Transfer {
                    from: from.try_to_cpi_handle_mut()?,
                    to: to.try_to_cpi_handle_mut()?,
                    authority: authority.to_cpi_handle(),
                },
            ),
            amount,
        )
    }

    fn transfer_checked<'a, F, M, T, A>(
        &'a self,
        from: &'a mut F,
        mint: &'a M,
        to: &'a mut T,
        authority: &'a A,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError>
    where
        F: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        T: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        transfer_checked(
            CpiContext::new(
                self.address(),
                accounts::TransferChecked {
                    from: from.try_to_cpi_handle_mut()?,
                    mint: mint.to_cpi_handle(),
                    to: to.try_to_cpi_handle_mut()?,
                    authority: authority.to_cpi_handle(),
                },
            ),
            amount,
            decimals,
        )
    }

    fn burn<'a, F, M, A>(
        &'a self,
        from: &'a mut F,
        mint: &'a mut M,
        authority: &'a A,
        amount: u64,
    ) -> Result<(), ProgramError>
    where
        F: ToCpiHandleMut + ?Sized,
        M: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        burn(
            CpiContext::new(
                self.address(),
                accounts::Burn {
                    from: from.try_to_cpi_handle_mut()?,
                    mint: mint.try_to_cpi_handle_mut()?,
                    authority: authority.to_cpi_handle(),
                },
            ),
            amount,
        )
    }

    fn approve<'a, S, D, A>(
        &'a self,
        source: &'a mut S,
        delegate: &'a D,
        authority: &'a A,
        amount: u64,
    ) -> Result<(), ProgramError>
    where
        S: ToCpiHandleMut + ?Sized,
        D: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        approve(
            CpiContext::new(
                self.address(),
                accounts::Approve {
                    to: source.try_to_cpi_handle_mut()?,
                    delegate: delegate.to_cpi_handle(),
                    authority: authority.to_cpi_handle(),
                },
            ),
            amount,
        )
    }

    fn revoke<'a, S, A>(&'a self, source: &'a mut S, authority: &'a A) -> Result<(), ProgramError>
    where
        S: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        revoke(CpiContext::new(
            self.address(),
            accounts::Revoke {
                source: source.try_to_cpi_handle_mut()?,
                authority: authority.to_cpi_handle(),
            },
        ))
    }

    fn close_account<'a, Acc, Dest, A>(
        &'a self,
        account: &'a mut Acc,
        destination: &'a mut Dest,
        authority: &'a A,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        Dest: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        close_account(CpiContext::new(
            self.address(),
            accounts::CloseAccount {
                account: account.try_to_cpi_handle_mut()?,
                destination: destination.try_to_cpi_handle_mut()?,
                authority: authority.to_cpi_handle(),
            },
        ))
    }

    fn mint_to_checked<'a, M, T, A>(
        &'a self,
        mint: &'a mut M,
        to: &'a mut T,
        authority: &'a A,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError>
    where
        M: ToCpiHandleMut + ?Sized,
        T: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        mint_to_checked(
            CpiContext::new(
                self.address(),
                accounts::MintToChecked {
                    mint: mint.try_to_cpi_handle_mut()?,
                    to: to.try_to_cpi_handle_mut()?,
                    authority: authority.to_cpi_handle(),
                },
            ),
            amount,
            decimals,
        )
    }

    fn burn_checked<'a, F, M, A>(
        &'a self,
        from: &'a mut F,
        mint: &'a mut M,
        authority: &'a A,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError>
    where
        F: ToCpiHandleMut + ?Sized,
        M: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        burn_checked(
            CpiContext::new(
                self.address(),
                accounts::BurnChecked {
                    from: from.try_to_cpi_handle_mut()?,
                    mint: mint.try_to_cpi_handle_mut()?,
                    authority: authority.to_cpi_handle(),
                },
            ),
            amount,
            decimals,
        )
    }

    fn approve_checked<'a, S, M, D, A>(
        &'a self,
        source: &'a mut S,
        mint: &'a M,
        delegate: &'a D,
        authority: &'a A,
        amount: u64,
        decimals: u8,
    ) -> Result<(), ProgramError>
    where
        S: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        D: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        approve_checked(
            CpiContext::new(
                self.address(),
                accounts::ApproveChecked {
                    to: source.try_to_cpi_handle_mut()?,
                    mint: mint.to_cpi_handle(),
                    delegate: delegate.to_cpi_handle(),
                    authority: authority.to_cpi_handle(),
                },
            ),
            amount,
            decimals,
        )
    }

    fn freeze_account<'a, Acc, M, A>(
        &'a self,
        account: &'a mut Acc,
        mint: &'a M,
        authority: &'a A,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        freeze_account(CpiContext::new(
            self.address(),
            accounts::FreezeAccount {
                account: account.try_to_cpi_handle_mut()?,
                mint: mint.to_cpi_handle(),
                authority: authority.to_cpi_handle(),
            },
        ))
    }

    fn thaw_account<'a, Acc, M, A>(
        &'a self,
        account: &'a mut Acc,
        mint: &'a M,
        authority: &'a A,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        thaw_account(CpiContext::new(
            self.address(),
            accounts::ThawAccount {
                account: account.try_to_cpi_handle_mut()?,
                mint: mint.to_cpi_handle(),
                authority: authority.to_cpi_handle(),
            },
        ))
    }

    fn sync_native<'a, Acc>(&'a self, account: &'a mut Acc) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
    {
        sync_native(CpiContext::new(
            self.address(),
            accounts::SyncNative {
                account: account.try_to_cpi_handle_mut()?,
            },
        ))
    }

    fn initialize_mint<'a, M, R>(
        &'a self,
        mint: &'a mut M,
        rent: &'a R,
        decimals: u8,
        authority: &Address,
        freeze_authority: Option<&Address>,
    ) -> Result<(), ProgramError>
    where
        M: ToCpiHandleMut + ?Sized,
        R: ToCpiHandle + ?Sized,
    {
        initialize_mint(
            CpiContext::new(
                self.address(),
                accounts::InitializeMint {
                    mint: mint.try_to_cpi_handle_mut()?,
                    rent: rent.to_cpi_handle(),
                },
            ),
            decimals,
            authority,
            freeze_authority,
        )
    }

    fn initialize_mint2<'a, M>(
        &'a self,
        mint: &'a mut M,
        decimals: u8,
        authority: &Address,
        freeze_authority: Option<&Address>,
    ) -> Result<(), ProgramError>
    where
        M: ToCpiHandleMut + ?Sized,
    {
        initialize_mint2(
            CpiContext::new(
                self.address(),
                accounts::InitializeMint2 {
                    mint: mint.try_to_cpi_handle_mut()?,
                },
            ),
            decimals,
            authority,
            freeze_authority,
        )
    }

    fn initialize_account<'a, Acc, M, A, R>(
        &'a self,
        account: &'a mut Acc,
        mint: &'a M,
        authority: &'a A,
        rent: &'a R,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized,
        R: ToCpiHandle + ?Sized,
    {
        initialize_account(CpiContext::new(
            self.address(),
            accounts::InitializeAccount {
                account: account.try_to_cpi_handle_mut()?,
                mint: mint.to_cpi_handle(),
                authority: authority.to_cpi_handle(),
                rent: rent.to_cpi_handle(),
            },
        ))
    }

    fn initialize_account3<'a, Acc, M, A>(
        &'a self,
        account: &'a mut Acc,
        mint: &'a M,
        authority: &'a A,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        M: ToCpiHandle + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        initialize_account3(CpiContext::new(
            self.address(),
            accounts::InitializeAccount3 {
                account: account.try_to_cpi_handle_mut()?,
                mint: mint.to_cpi_handle(),
                authority: authority.to_cpi_handle(),
            },
        ))
    }

    fn set_authority<'a, Acc, A>(
        &'a self,
        account_or_mint: &'a mut Acc,
        current_authority: &'a A,
        authority_type: spl_token::instruction::AuthorityType,
        new_authority: Option<Address>,
    ) -> Result<(), ProgramError>
    where
        Acc: ToCpiHandleMut + ?Sized,
        A: ToCpiHandle + ?Sized,
    {
        set_authority(
            CpiContext::new(
                self.address(),
                accounts::SetAuthority {
                    account_or_mint: account_or_mint.try_to_cpi_handle_mut()?,
                    current_authority: current_authority.to_cpi_handle(),
                },
            ),
            authority_type,
            new_authority,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_account_validation_rejects_non_canonical_coption_tags() {
        let mut data = [0u8; TokenAccount::LEN];
        data[32 + 32 + 8 + 4 + 32] = 1;

        assert_eq!(validate_token_account_initialized(&data), Ok(()));

        data[32 + 32 + 8] = 1;
        data[32 + 32 + 8 + 1] = 2;
        assert!(matches!(
            validate_token_account_initialized(&data),
            Err(ProgramError::InvalidAccountData)
        ));
    }

    #[test]
    fn token_account_accessors_require_canonical_some_tags() {
        let account = TokenAccount {
            mint: Address::new_from_array([0; 32]),
            owner: Address::new_from_array([0; 32]),
            amount: [0; 8],
            delegate_flag: [1, 2, 0, 0],
            delegate: Address::new_from_array([1; 32]),
            state: 1,
            is_native_flag: [1, 0, 1, 0],
            native_amount: [0; 8],
            delegated_amount: [0; 8],
            close_authority_flag: [1, 0, 0, 1],
            close_authority: Address::new_from_array([2; 32]),
        };

        assert!(!account.has_delegate());
        assert!(!account.is_native());
        assert!(!account.has_close_authority());
    }
}
