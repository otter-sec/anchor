//! Shared base token CPI helpers used by `token` and `token_2022`.
//!
//! Authority-bearing helpers support both single authorities and SPL
//! multisigs. Signer handles are part of each CPI accounts struct, while
//! unrelated trailing accounts remain on [`CpiContext::with_remaining_accounts`].

extern crate alloc;

#[cfg(feature = "guardrails")]
use anchor_lang::{require, Id};
use {
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_instruction::{AccountMeta, Instruction},
    solana_program_error::ProgramError,
    spl_token_2022_interface as spl_token_2022,
};

/// Add explicit signer accounts after the authority account without creating
/// a temporary `Vec<&Address>`. SPL instruction builders currently accept that
/// temporary representation, so the instruction is built with no signers and
/// its account metas are completed here instead.
pub(crate) fn add_signers(
    instruction: &mut Instruction,
    authority_index: usize,
    signers: &[CpiHandle<'_>],
) {
    if signers.is_empty() {
        return;
    }

    instruction.accounts[authority_index].is_signer = false;
    instruction.accounts.reserve(signers.len());
    instruction.accounts.splice(
        authority_index + 1..authority_index + 1,
        signers
            .iter()
            .map(|signer| AccountMeta::new_readonly(*signer.address(), true)),
    );
}

#[cfg(feature = "guardrails")]
#[inline]
pub(crate) fn validate_token_interface_program(program_id: &Address) -> Result<(), ProgramError> {
    require!(
        anchor_lang::address_eq(program_id, &anchor_lang::programs::Token::id())
            || anchor_lang::address_eq(program_id, &anchor_lang::programs::Token2022::id()),
        ProgramError::IncorrectProgramId
    );
    Ok(())
}

#[cfg(not(feature = "guardrails"))]
#[inline]
pub(crate) fn validate_token_interface_program(_program_id: &Address) -> Result<(), ProgramError> {
    Ok(())
}

#[derive(ToCpiAccounts)]
pub struct InitializeAccount<'a> {
    pub account: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    pub authority: CpiHandle<'a>,
    pub rent: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct InitializeAccount3<'a> {
    pub account: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    #[account_meta(skip)]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct InitializeMint<'a> {
    pub mint: CpiHandleMut<'a>,
    pub rent: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct InitializeMint2<'a> {
    pub mint: CpiHandleMut<'a>,
}

/// Token / Token-2022 transfer instruction — accounts list:
///   0. `[writable]` from
///   1. `[writable]` to
///   2. `[signer]` authority, or `[]` multisig authority followed by member signers
#[derive(ToCpiAccounts)]
pub struct Transfer<'a> {
    pub from: CpiHandleMut<'a>,
    pub to: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

/// Token / Token-2022 checked transfer instruction — adds the mint and verifies
/// the declared decimals match on-chain.
///   0. `[writable]` from
///   1. `[]` mint
///   2. `[writable]` to
///   3. `[signer]` authority, or `[]` multisig authority followed by member signers
#[derive(ToCpiAccounts)]
pub struct TransferChecked<'a> {
    pub from: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    pub to: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct MintTo<'a> {
    pub mint: CpiHandleMut<'a>,
    pub to: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct MintToChecked<'a> {
    pub mint: CpiHandleMut<'a>,
    pub to: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct Burn<'a> {
    pub from: CpiHandleMut<'a>,
    pub mint: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct BurnChecked<'a> {
    pub from: CpiHandleMut<'a>,
    pub mint: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct Approve<'a> {
    pub to: CpiHandleMut<'a>,
    pub delegate: CpiHandle<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct ApproveChecked<'a> {
    pub to: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    pub delegate: CpiHandle<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct Revoke<'a> {
    pub source: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct SetAuthority<'a> {
    pub account_or_mint: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub current_authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct CloseAccount<'a> {
    pub account: CpiHandleMut<'a>,
    pub destination: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct FreezeAccount<'a> {
    pub account: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct ThawAccount<'a> {
    pub account: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

#[derive(ToCpiAccounts)]
pub struct SyncNative<'a> {
    pub account: CpiHandleMut<'a>,
}

pub fn initialize_account<'a>(
    ctx: CpiContext<'a, InitializeAccount<'a>>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_account3<'a>(
    ctx: CpiContext<'a, InitializeAccount3<'a>>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_account3(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_mint<'a>(
    ctx: CpiContext<'a, InitializeMint<'a>>,
    decimals: u8,
    authority: &Address,
    freeze_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_mint(
        ctx.program,
        ctx.accounts.mint.address(),
        authority,
        freeze_authority,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn initialize_mint2<'a>(
    ctx: CpiContext<'a, InitializeMint2<'a>>,
    decimals: u8,
    authority: &Address,
    freeze_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_mint2(
        ctx.program,
        ctx.accounts.mint.address(),
        authority,
        freeze_authority,
        decimals,
    )?;
    ctx.invoke_ix(ix)
}

pub fn transfer<'a>(ctx: CpiContext<'a, Transfer<'a>>, amount: u64) -> Result<(), ProgramError> {
    #[allow(deprecated)]
    let mut ix = spl_token_2022::instruction::transfer(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    add_signers(&mut ix, 2, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn transfer_checked<'a>(
    ctx: CpiContext<'a, TransferChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::transfer_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    add_signers(&mut ix, 3, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn mint_to<'a>(ctx: CpiContext<'a, MintTo<'a>>, amount: u64) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::mint_to(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    add_signers(&mut ix, 2, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn mint_to_checked<'a>(
    ctx: CpiContext<'a, MintToChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::mint_to_checked(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    add_signers(&mut ix, 2, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn burn<'a>(ctx: CpiContext<'a, Burn<'a>>, amount: u64) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::burn(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    add_signers(&mut ix, 2, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn burn_checked<'a>(
    ctx: CpiContext<'a, BurnChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::burn_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    add_signers(&mut ix, 2, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn approve<'a>(ctx: CpiContext<'a, Approve<'a>>, amount: u64) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::approve(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
    )?;
    add_signers(&mut ix, 2, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn approve_checked<'a>(
    ctx: CpiContext<'a, ApproveChecked<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::approve_checked(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &[],
        amount,
        decimals,
    )?;
    add_signers(&mut ix, 3, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn revoke<'a>(ctx: CpiContext<'a, Revoke<'a>>) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::revoke(
        ctx.program,
        ctx.accounts.source.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    add_signers(&mut ix, 1, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn set_authority<'a>(
    ctx: CpiContext<'a, SetAuthority<'a>>,
    authority_type: spl_token_2022::instruction::AuthorityType,
    new_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::set_authority(
        ctx.program,
        ctx.accounts.account_or_mint.address(),
        new_authority,
        authority_type,
        ctx.accounts.current_authority.address(),
        &[],
    )?;
    add_signers(&mut ix, 1, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn close_account<'a>(ctx: CpiContext<'a, CloseAccount<'a>>) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::close_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.destination.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    add_signers(&mut ix, 2, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn freeze_account<'a>(ctx: CpiContext<'a, FreezeAccount<'a>>) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::freeze_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    add_signers(&mut ix, 2, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn thaw_account<'a>(ctx: CpiContext<'a, ThawAccount<'a>>) -> Result<(), ProgramError> {
    let mut ix = spl_token_2022::instruction::thaw_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
    )?;
    add_signers(&mut ix, 2, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn sync_native<'a>(ctx: CpiContext<'a, SyncNative<'a>>) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::sync_native(ctx.program, ctx.accounts.account.address())?;
    ctx.invoke_ix(ix)
}

#[cfg(test)]
mod tests {
    use {
        alloc::vec,
        super::*,
        anchor_lang::{
            programs::Token,
            testing::{AccountBuffer, MIN_ACCOUNT_BUF},
            Id,
        },
    };

    fn signer(address: [u8; 32]) -> AccountBuffer<{ MIN_ACCOUNT_BUF + 8 }> {
        let buffer = AccountBuffer::new();
        buffer.init(address, [9; 32], 8, true, false, false);
        buffer
    }

    #[test]
    fn signer_handles_encode_canonical_multisig_layout() {
        let member_one = signer([4; 32]);
        let member_two = signer([5; 32]);
        let member_one_view = unsafe { member_one.view() };
        let member_two_view = unsafe { member_two.view() };
        let handles = [
            CpiHandle::readonly(&member_one_view),
            CpiHandle::readonly(&member_two_view),
        ];
        #[allow(deprecated)]
        let mut ix = spl_token_2022::instruction::transfer(
            &Token::id(),
            &Address::new_from_array([1; 32]),
            &Address::new_from_array([2; 32]),
            &Address::new_from_array([3; 32]),
            &[],
            7,
        )
        .unwrap();
        add_signers(&mut ix, 2, &handles);

        assert_eq!(ix.accounts.len(), 5);
        assert!(
            !ix.accounts[2].is_signer,
            "multisig account is not a signer"
        );
        assert!(ix.accounts[3].is_signer);
        assert!(ix.accounts[4].is_signer);
        assert_eq!(ix.accounts[3].pubkey.as_ref(), [4; 32].as_slice());
        assert_eq!(ix.accounts[4].pubkey.as_ref(), [5; 32].as_slice());
    }

    #[test]
    fn signer_metas_are_inserted_before_trailing_accounts() {
        let member = signer([4; 32]);
        let member_view = unsafe { member.view() };
        let member_handle = CpiHandle::readonly(&member_view);

        let mint = Address::new_from_array([1; 32]);
        let authority = Address::new_from_array([2; 32]);
        let trailing = Address::new_from_array([3; 32]);
        let mut ix = Instruction {
            program_id: Token::id(),
            accounts: vec![
                AccountMeta::new(mint, false),
                AccountMeta::new_readonly(authority, true),
                AccountMeta::new_readonly(trailing, false),
            ],
            data: vec![],
        };

        add_signers(&mut ix, 1, &[member_handle]);

        assert_eq!(ix.accounts.len(), 4);
        assert_eq!(ix.accounts[1].pubkey, authority);
        assert!(!ix.accounts[1].is_signer);
        assert_eq!(ix.accounts[2].pubkey, member_handle.address().clone());
        assert!(ix.accounts[2].is_signer);
        assert_eq!(ix.accounts[3].pubkey, trailing);
    }

    #[test]
    fn signer_slices_are_separate_from_generic_remaining_accounts() {
        let from = AccountBuffer::<{ MIN_ACCOUNT_BUF + 8 }>::new();
        from.init([1; 32], [9; 32], 8, false, true, false);
        let to = AccountBuffer::<{ MIN_ACCOUNT_BUF + 8 }>::new();
        to.init([2; 32], [9; 32], 8, false, true, false);
        let authority = AccountBuffer::<{ MIN_ACCOUNT_BUF + 8 }>::new();
        authority.init([3; 32], [9; 32], 8, false, false, false);
        let member = signer([4; 32]);
        let trailing = AccountBuffer::<{ MIN_ACCOUNT_BUF + 8 }>::new();
        trailing.init([5; 32], [9; 32], 8, false, true, false);

        let mut from_view = unsafe { from.view() };
        let mut to_view = unsafe { to.view() };
        let authority_view = unsafe { authority.view() };
        let member_view = unsafe { member.view() };
        let mut trailing_view = unsafe { trailing.view() };
        let member_handle = CpiHandle::readonly(&member_view);
        let signers = [member_handle];
        let accounts = Transfer {
            from: CpiHandleMut::writable(&mut from_view),
            to: CpiHandleMut::writable(&mut to_view),
            authority: CpiHandle::readonly(&authority_view),
            signers: &signers,
        };
        let program = Token::id();
        let ctx = CpiContext::new(&program, accounts)
            .with_remaining_accounts(alloc::vec![CpiHandle::writable(&mut trailing_view)]);

        let metas = ctx.accounts.to_instruction_accounts();
        let handles = ctx.accounts.to_cpi_handles();
        assert_eq!(metas.len(), 4);
        assert_eq!(handles.len(), 4);
        assert!(!metas[2].is_signer);
        assert!(metas[3].is_signer);
        assert_eq!(ctx.remaining_accounts.len(), 1);
        assert_eq!(
            *ctx.remaining_accounts[0].address(),
            Address::new_from_array([5; 32])
        );
    }
}
