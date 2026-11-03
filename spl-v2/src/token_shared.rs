//! Shared base token CPI helpers used by `token` and `token_2022`.
//!
//! Authority-bearing helpers support both single authorities and SPL
//! multisigs. For a multisig, pass the member signer handles in canonical
//! order through [`CpiContext::with_remaining_accounts`].

extern crate alloc;

#[cfg(feature = "guardrails")]
use anchor_lang::{require, Id};
use {
    alloc::vec::Vec,
    anchor_lang::{
        CpiContext, CpiHandle, CpiHandleMut, CpiInstruction, FixedVec, ToCpiAccounts,
        ToCpiAccountsWithCapacity,
    },
    pinocchio::{address::Address, instruction::InstructionAccount},
    solana_instruction::Instruction,
    solana_program_error::ProgramError,
    spl_token_2022_interface as spl_token_2022,
};

/// Private bridge for canonical SPL builders until their Pinocchio migration.
///
/// This keeps the owned Solana instruction type out of `anchor-lang`'s target
/// API while borrowing its fields into the caller-selected CPI capacity.
pub(crate) struct CanonicalInstruction<'a, const N: usize> {
    program_id: &'a Address,
    data: &'a [u8],
    accounts: FixedVec<InstructionAccount<'a>, N>,
}

impl<'a, const N: usize> CanonicalInstruction<'a, N> {
    fn try_new(instruction: &'a Instruction) -> Result<Self, ProgramError> {
        let mut accounts = FixedVec::new();
        for account in &instruction.accounts {
            accounts
                .try_push(InstructionAccount::new(
                    &account.pubkey,
                    account.is_writable,
                    account.is_signer,
                ))
                .map_err(|_| ProgramError::InvalidArgument)?;
        }

        Ok(Self {
            program_id: &instruction.program_id,
            data: &instruction.data,
            accounts,
        })
    }

    pub(crate) fn invoke<'ctx, T>(
        ctx: &CpiContext<'ctx, T, N>,
        instruction: &'a Instruction,
    ) -> Result<(), ProgramError>
    where
        T: ToCpiAccountsWithCapacity<'ctx, N>,
    {
        ctx.invoke_ix(Self::try_new(instruction)?)
    }
}

impl<const N: usize> CpiInstruction for CanonicalInstruction<'_, N> {
    fn program_id(&self) -> &Address {
        self.program_id
    }

    fn data(&self) -> &[u8] {
        self.data
    }

    fn with_instruction_accounts<R>(
        &self,
        visit: impl FnOnce(&[InstructionAccount<'_>]) -> R,
    ) -> R {
        visit(&self.accounts)
    }
}

/// SPL Token encodes a multisig authority by marking the authority account
/// non-signer and appending each member signer to the instruction. Token CPI
/// callers provide those member handles through `remaining_accounts`.
pub(crate) fn multisig_signer_addresses<'a>(accounts: &[CpiHandle<'a>]) -> Vec<&'a Address> {
    accounts.iter().map(CpiHandle::address).collect()
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
    #[signer]
    pub authority: CpiHandle<'a>,
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
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct MintTo<'a> {
    pub mint: CpiHandleMut<'a>,
    pub to: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct MintToChecked<'a> {
    pub mint: CpiHandleMut<'a>,
    pub to: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct Burn<'a> {
    pub from: CpiHandleMut<'a>,
    pub mint: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct BurnChecked<'a> {
    pub from: CpiHandleMut<'a>,
    pub mint: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct Approve<'a> {
    pub to: CpiHandleMut<'a>,
    pub delegate: CpiHandle<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct ApproveChecked<'a> {
    pub to: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    pub delegate: CpiHandle<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct Revoke<'a> {
    pub source: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct SetAuthority<'a> {
    pub account_or_mint: CpiHandleMut<'a>,
    #[signer]
    pub current_authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct CloseAccount<'a> {
    pub account: CpiHandleMut<'a>,
    pub destination: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct FreezeAccount<'a> {
    pub account: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct ThawAccount<'a> {
    pub account: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct SyncNative<'a> {
    pub account: CpiHandleMut<'a>,
}

pub fn initialize_account<'a, const N: usize>(
    ctx: CpiContext<'a, InitializeAccount<'a>, N>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn initialize_account3<'a, const N: usize>(
    ctx: CpiContext<'a, InitializeAccount3<'a>, N>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::initialize_account3(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn initialize_mint<'a, const N: usize>(
    ctx: CpiContext<'a, InitializeMint<'a>, N>,
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
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn initialize_mint2<'a, const N: usize>(
    ctx: CpiContext<'a, InitializeMint2<'a>, N>,
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
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn transfer<'a, const N: usize>(
    ctx: CpiContext<'a, Transfer<'a>, N>,
    amount: u64,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    #[allow(deprecated)]
    let ix = spl_token_2022::instruction::transfer(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn transfer_checked<'a, const N: usize>(
    ctx: CpiContext<'a, TransferChecked<'a>, N>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::transfer_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
        decimals,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn mint_to<'a, const N: usize>(
    ctx: CpiContext<'a, MintTo<'a>, N>,
    amount: u64,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::mint_to(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn mint_to_checked<'a, const N: usize>(
    ctx: CpiContext<'a, MintToChecked<'a>, N>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::mint_to_checked(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.to.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
        decimals,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn burn<'a, const N: usize>(
    ctx: CpiContext<'a, Burn<'a>, N>,
    amount: u64,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::burn(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn burn_checked<'a, const N: usize>(
    ctx: CpiContext<'a, BurnChecked<'a>, N>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::burn_checked(
        ctx.program,
        ctx.accounts.from.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
        decimals,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn approve<'a, const N: usize>(
    ctx: CpiContext<'a, Approve<'a>, N>,
    amount: u64,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::approve(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn approve_checked<'a, const N: usize>(
    ctx: CpiContext<'a, ApproveChecked<'a>, N>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::approve_checked(
        ctx.program,
        ctx.accounts.to.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.delegate.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        amount,
        decimals,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn revoke<'a, const N: usize>(ctx: CpiContext<'a, Revoke<'a>, N>) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::revoke(
        ctx.program,
        ctx.accounts.source.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn set_authority<'a, const N: usize>(
    ctx: CpiContext<'a, SetAuthority<'a>, N>,
    authority_type: spl_token_2022::instruction::AuthorityType,
    new_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::set_authority(
        ctx.program,
        ctx.accounts.account_or_mint.address(),
        new_authority,
        authority_type,
        ctx.accounts.current_authority.address(),
        &signer_addresses,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn close_account<'a, const N: usize>(
    ctx: CpiContext<'a, CloseAccount<'a>, N>,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::close_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.destination.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn freeze_account<'a, const N: usize>(
    ctx: CpiContext<'a, FreezeAccount<'a>, N>,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::freeze_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn thaw_account<'a, const N: usize>(
    ctx: CpiContext<'a, ThawAccount<'a>, N>,
) -> Result<(), ProgramError> {
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::instruction::thaw_account(
        ctx.program,
        ctx.accounts.account.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
    )?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

pub fn sync_native<'a, const N: usize>(
    ctx: CpiContext<'a, SyncNative<'a>, N>,
) -> Result<(), ProgramError> {
    let ix = spl_token_2022::instruction::sync_native(ctx.program, ctx.accounts.account.address())?;
    crate::token_shared::CanonicalInstruction::<N>::invoke(&ctx, &ix)
}

#[cfg(test)]
mod tests {
    use {
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
    fn remaining_signers_encode_canonical_multisig_layout() {
        let member_one = signer([4; 32]);
        let member_two = signer([5; 32]);
        let member_one_view = unsafe { member_one.view() };
        let member_two_view = unsafe { member_two.view() };
        let handles = [
            CpiHandle::readonly(&member_one_view),
            CpiHandle::readonly(&member_two_view),
        ];
        let signer_addresses = multisig_signer_addresses(&handles);

        #[allow(deprecated)]
        let ix = spl_token_2022::instruction::transfer(
            &Token::id(),
            &Address::new_from_array([1; 32]),
            &Address::new_from_array([2; 32]),
            &Address::new_from_array([3; 32]),
            &signer_addresses,
            7,
        )
        .unwrap();

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

}
