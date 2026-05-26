//! SPL Token mint type with `SlabSchema` impl for use with `Account<T>`.
//!
//! Layout mirrors `pinocchio-token` — all fields are alignment-1 to support
//! zerocopy mapping from the account data buffer.

use {
    crate::token::{coption_is_some, create_token_account, validate_coption_tag},
    anchor_lang_v2::{
        accounts::{Account, SlabInit, SlabSchema},
        programs::Token,
        require, require_eq, AccountConstraint, Id,
    },
    bytemuck::{Pod, Zeroable},
    pinocchio::account::AccountView,
    solana_address::Address,
    solana_program_error::ProgramError,
};

pub(crate) fn validate_mint_initialized(data: &[u8]) -> Result<(), ProgramError> {
    const MINT_INITIALIZED_OFFSET: usize = 36 + 8 + 1;
    const MINT_AUTHORITY_TAG_OFFSET: usize = 0;
    const MINT_FREEZE_AUTHORITY_TAG_OFFSET: usize = MINT_INITIALIZED_OFFSET + 1;

    validate_coption_tag(data, MINT_AUTHORITY_TAG_OFFSET)?;
    validate_coption_tag(data, MINT_FREEZE_AUTHORITY_TAG_OFFSET)?;

    match data.get(MINT_INITIALIZED_OFFSET).copied() {
        Some(1) => Ok(()),
        Some(0) => Err(ProgramError::UninitializedAccount),
        Some(_) => Err(ProgramError::InvalidAccountData),
        None => Err(ProgramError::InvalidAccountData),
    }
}

/// SPL Token mint data, zerocopy-mapped (82 bytes).
///
/// All fields are private — use the accessor methods to read data. Mint state
/// is modified only by the SPL Token program via CPI (MintTo, Burn,
/// SetAuthority, etc.); user programs cannot mutate these fields directly
/// anyway because the account is owned by the SPL Token program.
#[repr(C)]
#[derive(Clone, Copy)]
pub struct Mint {
    mint_authority_flag: [u8; 4],
    mint_authority: Address,
    supply: [u8; 8],
    decimals: u8,
    is_initialized: u8,
    freeze_authority_flag: [u8; 4],
    freeze_authority: Address,
}

// SAFETY: Mint is repr(C) with all alignment-1 fields, no padding.
unsafe impl Pod for Mint {}
unsafe impl Zeroable for Mint {}

// Mint is defined by the SPL Token program, not by the user's program — its
// layout is known to any SPL-aware client. Default `__IDL_TYPE_DEF = None` keeps
// it out of the user's IDL `types[]` array (matches v1's `impl_idl_build!`
// behavior for this type).
#[doc(hidden)]
impl anchor_lang_v2::IdlAccountType for Mint {}

// On-chain size — SPL Token program requires 82 bytes. Used by
// `#[account(init, mint::*)]` as the default when `space` is omitted.
impl anchor_lang_v2::Space for Mint {
    const INIT_SPACE: usize = core::mem::size_of::<Self>();
}

impl SlabSchema for Mint {
    // External types start at offset 0 — no Anchor discriminator.
    const DATA_OFFSET: usize = 0;
    const MIN_DATA_LEN: usize = core::mem::size_of::<Self>();

    #[inline(always)]
    fn validate(view: &AccountView, data: &[u8]) -> Result<(), ProgramError> {
        require!(view.owned_by(&Token::id()), ProgramError::IllegalOwner);
        require_eq!(
            data.len(),
            core::mem::size_of::<Self>(),
            ProgramError::InvalidAccountData
        );
        validate_mint_initialized(data)?;
        Ok(())
    }
}

/// Init params for `#[account(init, mint::decimals = 6, mint::authority = ..., ...)]`.
#[derive(Default)]
pub struct MintInitParams<'a> {
    pub decimals: Option<u8>,
    pub authority: Option<&'a AccountView>,
    pub freeze_authority: Option<&'a AccountView>,
}

impl SlabInit for Mint {
    type Params<'a> = MintInitParams<'a>;

    #[cold]
    fn create_and_initialize<'a>(
        payer: &AccountView,
        account: &AccountView,
        _space: usize,
        params: &Self::Params<'a>,
        signer_seeds: Option<&[&[u8]]>,
    ) -> Result<(), ProgramError> {
        let decimals = params.decimals.ok_or(ProgramError::InvalidArgument)?;
        let authority = params.authority.ok_or(ProgramError::InvalidArgument)?;

        create_token_account(payer, account, core::mem::size_of::<Self>(), signer_seeds)?;

        pinocchio_token::instructions::InitializeMint2 {
            mint: account,
            decimals,
            mint_authority: authority.address(),
            freeze_authority: params.freeze_authority.map(|v| v.address()),
        }
        .invoke()
    }
}

impl Mint {
    pub const LEN: usize = core::mem::size_of::<Self>();

    /// Total supply of tokens.
    pub fn supply(&self) -> u64 {
        u64::from_le_bytes(self.supply)
    }

    /// Number of base-10 digits to the right of the decimal place.
    pub fn decimals(&self) -> u8 {
        self.decimals
    }

    /// Whether a mint authority is currently set.
    pub fn has_mint_authority(&self) -> bool {
        coption_is_some(&self.mint_authority_flag)
    }

    /// The mint authority, if any.
    pub fn mint_authority(&self) -> Option<&Address> {
        if self.has_mint_authority() {
            Some(&self.mint_authority)
        } else {
            None
        }
    }

    /// Whether the mint has been initialized.
    pub fn is_initialized(&self) -> bool {
        self.is_initialized == 1
    }

    /// Whether a freeze authority is currently set.
    pub fn has_freeze_authority(&self) -> bool {
        coption_is_some(&self.freeze_authority_flag)
    }

    /// The freeze authority, if any.
    pub fn freeze_authority(&self) -> Option<&Address> {
        if self.has_freeze_authority() {
            Some(&self.freeze_authority)
        } else {
            None
        }
    }
}

// ---------------------------------------------------------------------------
// Constraint markers for `#[account(mint::*)]`
// ---------------------------------------------------------------------------

pub struct AuthorityConstraint;
pub struct FreezeAuthorityConstraint;
pub struct DecimalsConstraint;
pub struct TokenProgramConstraint;

impl AccountConstraint<Account<Mint>> for AuthorityConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(account: &Account<Mint>, expected: &Address) -> Result<(), ProgramError> {
        require_eq!(
            account.mint_authority(),
            Some(expected),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }
}

impl AccountConstraint<Account<Mint>> for FreezeAuthorityConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(account: &Account<Mint>, expected: &Address) -> Result<(), ProgramError> {
        require_eq!(
            account.freeze_authority(),
            Some(expected),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }
}

/// `mint::decimals = 6` — non-address constraint, compares u8.
impl AccountConstraint<Account<Mint>> for DecimalsConstraint {
    type Value = u8;
    #[inline(always)]
    fn check(account: &Account<Mint>, expected: &u8) -> Result<(), ProgramError> {
        require_eq!(
            account.decimals(),
            *expected,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }
}

/// `mint::token_program = token_program` — check mint is owned by given program.
impl AccountConstraint<Account<Mint>> for TokenProgramConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(account: &Account<Mint>, expected: &Address) -> Result<(), ProgramError> {
        require!(
            AsRef::<AccountView>::as_ref(account).owned_by(expected),
            ProgramError::IllegalOwner
        );
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mint_validation_rejects_non_canonical_coption_tags() {
        let mut data = [0u8; Mint::LEN];
        data[36 + 8 + 1] = 1;

        assert_eq!(validate_mint_initialized(&data), Ok(()));

        data[0] = 1;
        data[1] = 2;
        assert!(matches!(
            validate_mint_initialized(&data),
            Err(ProgramError::InvalidAccountData)
        ));
    }

    #[test]
    fn mint_accessors_require_canonical_some_tags() {
        let mint = Mint {
            mint_authority_flag: [1, 2, 0, 0],
            mint_authority: Address::new_from_array([1; 32]),
            supply: [0; 8],
            decimals: 6,
            is_initialized: 1,
            freeze_authority_flag: [1, 0, 0, 1],
            freeze_authority: Address::new_from_array([2; 32]),
        };

        assert!(!mint.has_mint_authority());
        assert!(!mint.has_freeze_authority());
    }
}
