//! SPL Token account type with `SlabSchema` impl for use with `Account<T>`.
//!
//! Layout mirrors `pinocchio-token` — all fields are alignment-1 to support
//! zerocopy mapping from the account data buffer.

extern crate alloc;

use {
    alloc::{vec, vec::Vec},
    anchor_lang_v2::{
        accounts::{Account, SlabInit, SlabSchema},
        solana_program::program,
        AccountConstraint, CpiContext, CpiHandle, Id, ToCpiAccounts,
    },
    bytemuck::{Pod, Zeroable},
    pinocchio::{account::AccountView, instruction::InstructionAccount},
    solana_address::Address,
    solana_instruction::{AccountMeta, Instruction},
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
// — its layout is known to any SPL-aware client. Default `__IDL_TYPE = None`
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

    #[inline(always)]
    fn validate(
        view: &AccountView,
        data: &[u8],
        _program_id: &Address,
    ) -> Result<(), ProgramError> {
        if !view.owned_by(&Token::id()) {
            return Err(ProgramError::IllegalOwner);
        }
        // Exact size distinguishes TokenAccount (165) from Mint (82).
        if data.len() != core::mem::size_of::<Self>() {
            return Err(ProgramError::InvalidAccountData);
        }
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
        _program_id: &Address,
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
        if !anchor_lang_v2::address_eq(account.mint(), expected) {
            Err(ProgramError::InvalidAccountData)
        } else {
            Ok(())
        }
    }
}

impl AccountConstraint<Account<TokenAccount>> for AuthorityConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(account: &Account<TokenAccount>, expected: &Address) -> Result<(), ProgramError> {
        if !anchor_lang_v2::address_eq(account.owner(), expected) {
            Err(ProgramError::InvalidAccountData)
        } else {
            Ok(())
        }
    }
}

/// `token::TokenProgram = token_program` — check account is owned by given program.
impl AccountConstraint<Account<TokenAccount>> for TokenProgramConstraint {
    type Value = Address;
    #[inline(always)]
    fn check(account: &Account<TokenAccount>, expected: &Address) -> Result<(), ProgramError> {
        if !AsRef::<AccountView>::as_ref(account).owned_by(expected) {
            Err(ProgramError::IllegalOwner)
        } else {
            Ok(())
        }
    }
}

// ---------------------------------------------------------------------------
// CPI helpers for base token-program invocations (`anchor_spl_v2::token`)
// ---------------------------------------------------------------------------
//
// The v2 helpers accept either the Token Program or Token-2022 program for
// instructions shared by both programs. Each helper routes through
// `CpiContext::invoke` (pinocchio `invoke_signed_unchecked`), bypassing the
// borrow-state check that rejects direct pinocchio-token invoke on Slab-loaded
// accounts.
//
/// Accounts structs consumed by each CPI helper. Each field is a
/// `CpiHandle<'a>` obtained from `AnchorAccount::cpi_handle{,_mut}`.
pub mod accounts {
    use super::*;

    pub struct InitializeAccount<'a> {
        pub account: CpiHandle<'a>,
        pub mint: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
        pub rent: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for InitializeAccount<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.account.address()),
                InstructionAccount::new(self.mint.address(), false, false),
                InstructionAccount::new(self.authority.address(), false, false),
                InstructionAccount::new(self.rent.address(), false, false),
            ]
        }

        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.account, self.mint, self.authority, self.rent]
        }
    }

    pub struct InitializeAccount3<'a> {
        pub account: CpiHandle<'a>,
        pub mint: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for InitializeAccount3<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.account.address()),
                InstructionAccount::new(self.mint.address(), false, false),
            ]
        }

        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.account, self.mint]
        }
    }

    pub struct InitializeMint<'a> {
        pub mint: CpiHandle<'a>,
        pub rent: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for InitializeMint<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.mint.address()),
                InstructionAccount::new(self.rent.address(), false, false),
            ]
        }

        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.mint, self.rent]
        }
    }

    pub struct InitializeMint2<'a> {
        pub mint: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for InitializeMint2<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![InstructionAccount::writable(self.mint.address())]
        }

        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.mint]
        }
    }

    /// `spl_token::instruction::transfer` — accounts list:
    ///   0. `[writable]` from
    ///   1. `[writable]` to
    ///   2. `[signer]` authority (owner/delegate)
    pub struct Transfer<'a> {
        pub from: CpiHandle<'a>,
        pub to: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for Transfer<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.from.address()),
                InstructionAccount::writable(self.to.address()),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }

        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.from, self.to, self.authority]
        }
    }

    /// `spl_token::instruction::transfer_checked` — adds the mint and
    /// verifies the declared decimals match on-chain.
    ///   0. `[writable]` from
    ///   1. `[]` mint
    ///   2. `[writable]` to
    ///   3. `[signer]` authority
    pub struct TransferChecked<'a> {
        pub from: CpiHandle<'a>,
        pub mint: CpiHandle<'a>,
        pub to: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for TransferChecked<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.from.address()),
                InstructionAccount::new(self.mint.address(), false, false),
                InstructionAccount::writable(self.to.address()),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }

        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.from, self.mint, self.to, self.authority]
        }
    }

    pub struct MintTo<'a> {
        pub mint: CpiHandle<'a>,
        pub to: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for MintTo<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.mint.address()),
                InstructionAccount::writable(self.to.address()),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.mint, self.to, self.authority]
        }
    }

    pub struct MintToChecked<'a> {
        pub mint: CpiHandle<'a>,
        pub to: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for MintToChecked<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.mint.address()),
                InstructionAccount::writable(self.to.address()),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.mint, self.to, self.authority]
        }
    }

    pub struct Burn<'a> {
        pub mint: CpiHandle<'a>,
        pub from: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for Burn<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.from.address()),
                InstructionAccount::writable(self.mint.address()),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.from, self.mint, self.authority]
        }
    }

    pub struct BurnChecked<'a> {
        pub mint: CpiHandle<'a>,
        pub from: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for BurnChecked<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.from.address()),
                InstructionAccount::writable(self.mint.address()),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.from, self.mint, self.authority]
        }
    }

    pub struct Approve<'a> {
        pub to: CpiHandle<'a>,
        pub delegate: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for Approve<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.to.address()),
                InstructionAccount::new(self.delegate.address(), false, false),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.to, self.delegate, self.authority]
        }
    }

    pub struct ApproveChecked<'a> {
        pub to: CpiHandle<'a>,
        pub mint: CpiHandle<'a>,
        pub delegate: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for ApproveChecked<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.to.address()),
                InstructionAccount::new(self.mint.address(), false, false),
                InstructionAccount::new(self.delegate.address(), false, false),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.to, self.mint, self.delegate, self.authority]
        }
    }

    pub struct Revoke<'a> {
        pub source: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for Revoke<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.source.address()),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.source, self.authority]
        }
    }

    pub struct SetAuthority<'a> {
        pub current_authority: CpiHandle<'a>,
        pub account_or_mint: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for SetAuthority<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.account_or_mint.address()),
                InstructionAccount::readonly_signer(self.current_authority.address()),
            ]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.account_or_mint, self.current_authority]
        }
    }

    pub struct CloseAccount<'a> {
        pub account: CpiHandle<'a>,
        pub destination: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for CloseAccount<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.account.address()),
                InstructionAccount::writable(self.destination.address()),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.account, self.destination, self.authority]
        }
    }

    pub struct FreezeAccount<'a> {
        pub account: CpiHandle<'a>,
        pub mint: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for FreezeAccount<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.account.address()),
                InstructionAccount::new(self.mint.address(), false, false),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.account, self.mint, self.authority]
        }
    }

    pub struct ThawAccount<'a> {
        pub account: CpiHandle<'a>,
        pub mint: CpiHandle<'a>,
        pub authority: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for ThawAccount<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![
                InstructionAccount::writable(self.account.address()),
                InstructionAccount::new(self.mint.address(), false, false),
                InstructionAccount::readonly_signer(self.authority.address()),
            ]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.account, self.mint, self.authority]
        }
    }

    pub struct SyncNative<'a> {
        pub account: CpiHandle<'a>,
    }

    impl<'a> ToCpiAccounts<'a> for SyncNative<'a> {
        fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
            vec![InstructionAccount::writable(self.account.address())]
        }
        fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
            vec![self.account]
        }
    }
}

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

#[cfg(feature = "guardrails")]
#[inline]
pub(crate) fn validate_token_program(program_id: &Address) -> Result<(), ProgramError> {
    // Base token instructions are shared by Token and Token-2022. Keep this
    // broad so callers can use `anchor_spl_v2::token::*` with either program.
    if anchor_lang_v2::address_eq(program_id, &Token::id())
        || anchor_lang_v2::address_eq(program_id, &anchor_lang_v2::programs::Token2022::id())
    {
        Ok(())
    } else {
        Err(ProgramError::IncorrectProgramId)
    }
}

#[cfg(not(feature = "guardrails"))]
#[inline]
pub(crate) fn validate_token_program(_program_id: &Address) -> Result<(), ProgramError> {
    Ok(())
}

fn invoke_token<'a, T: ToCpiAccounts<'a>>(
    ctx: &CpiContext<'a, T>,
    mut ix: Instruction,
) -> Result<(), ProgramError> {
    let mut instruction_accounts = ctx.accounts.to_instruction_accounts();
    let mut handles = ctx.accounts.to_cpi_handles();

    for handle in &ctx.remaining_accounts {
        instruction_accounts.push(InstructionAccount::new(
            handle.address(),
            handle.is_writable(),
            handle.is_signer(),
        ));
        handles.push(*handle);
    }

    ix.accounts = instruction_accounts
        .iter()
        .map(|account| AccountMeta {
            pubkey: *account.address,
            is_writable: account.is_writable,
            is_signer: account.is_signer,
        })
        .collect();

    program::invoke_signed(&ix, &handles, ctx.signer_seeds)
}

pub fn initialize_account<'a>(
    ctx: CpiContext<'a, accounts::InitializeAccount<'a>>,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::initialize_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
    )?;
    invoke_token(&ctx, ix)
}

pub fn initialize_account3<'a>(
    ctx: CpiContext<'a, accounts::InitializeAccount3<'a>>,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::initialize_account3(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
    )?;
    invoke_token(&ctx, ix)
}

pub fn initialize_mint<'a>(
    ctx: CpiContext<'a, accounts::InitializeMint<'a>>,
    decimals: u8,
    authority: &Address,
    freeze_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::initialize_mint(
        ctx.program,
        ctx.accounts.mint.address(),
        authority,
        freeze_authority,
        decimals,
    )?;
    invoke_token(&ctx, ix)
}

pub fn initialize_mint2<'a>(
    ctx: CpiContext<'a, accounts::InitializeMint2<'a>>,
    decimals: u8,
    authority: &Address,
    freeze_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::initialize_mint2(
        ctx.program,
        ctx.accounts.mint.address(),
        authority,
        freeze_authority,
        decimals,
    )?;
    invoke_token(&ctx, ix)
}

pub fn transfer<'a>(
    ctx: CpiContext<'a, accounts::Transfer<'a>>,
    amount: u64,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    #[allow(deprecated)]
    let ix = spl_token_2022::instruction::transfer(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    invoke_token(&ctx, ix)
}

pub fn transfer_checked<'a>(
    ctx: CpiContext<'a, accounts::TransferChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::transfer_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    invoke_token(&ctx, ix)
}

pub fn mint_to<'a>(
    ctx: CpiContext<'a, accounts::MintTo<'a>>,
    amount: u64,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::mint_to(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    invoke_token(&ctx, ix)
}

pub fn mint_to_checked<'a>(
    ctx: CpiContext<'a, accounts::MintToChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::mint_to_checked(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    invoke_token(&ctx, ix)
}

pub fn burn<'a>(ctx: CpiContext<'a, accounts::Burn<'a>>, amount: u64) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::burn(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    invoke_token(&ctx, ix)
}

pub fn burn_checked<'a>(
    ctx: CpiContext<'a, accounts::BurnChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::burn_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    invoke_token(&ctx, ix)
}

pub fn approve<'a>(
    ctx: CpiContext<'a, accounts::Approve<'a>>,
    amount: u64,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::approve(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    invoke_token(&ctx, ix)
}

pub fn approve_checked<'a>(
    ctx: CpiContext<'a, accounts::ApproveChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::approve_checked(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    invoke_token(&ctx, ix)
}

pub fn revoke<'a>(ctx: CpiContext<'a, accounts::Revoke<'a>>) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::revoke(
        ctx.program,
        ctx.accounts.source.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    invoke_token(&ctx, ix)
}

pub fn set_authority<'a>(
    ctx: CpiContext<'a, accounts::SetAuthority<'a>>,
    authority_type: spl_token::instruction::AuthorityType,
    new_authority: Option<Address>,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::set_authority(
        ctx.program,
        ctx.accounts.account_or_mint.address(),
        new_authority.as_ref(),
        token_2022_authority_type(authority_type),
        ctx.accounts.current_authority.address(),
        &[],
    )?;
    invoke_token(&ctx, ix)
}

pub fn close_account<'a>(
    ctx: CpiContext<'a, accounts::CloseAccount<'a>>,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::close_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.destination.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    invoke_token(&ctx, ix)
}

pub fn freeze_account<'a>(
    ctx: CpiContext<'a, accounts::FreezeAccount<'a>>,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::freeze_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    invoke_token(&ctx, ix)
}

pub fn thaw_account<'a>(
    ctx: CpiContext<'a, accounts::ThawAccount<'a>>,
) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::thaw_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    invoke_token(&ctx, ix)
}

pub fn sync_native<'a>(ctx: CpiContext<'a, accounts::SyncNative<'a>>) -> Result<(), ProgramError> {
    validate_token_program(ctx.program)?;
    let ix = spl_token_2022::instruction::sync_native(ctx.program, ctx.accounts.account.address())?;
    invoke_token(&ctx, ix)
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
