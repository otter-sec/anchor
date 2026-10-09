//! SPL Memo CPI helpers.
//!
//! Memo v3 has no accounts of its own: the instruction is a signer list plus
//! opaque memo bytes. Every account is therefore supplied by the caller as a
//! remaining account, matching Anchor v1's `anchor_spl::memo`.

extern crate alloc;

use {
    alloc::vec::Vec,
    anchor_lang::{programs::Memo, CpiContext, CpiHandle, Id, Result, ToCpiAccounts},
    pinocchio::{address::Address, instruction::InstructionAccount},
    solana_instruction::Instruction,
};

pub use spl_memo_interface::v3::ID as MEMO_PROGRAM_ID;

/// CPI accounts for `build_memo`.
///
/// Memo v3 declares no accounts, so this carries no typed fields and every
/// account reaches the callee as a remaining account.
///
/// [`ToCpiAccounts`] is implemented by hand rather than derived. The derive
/// rejects structs without a `CpiHandle`/`CpiHandleMut` field, and that
/// restriction is load-bearing: `CpiContext::invoke_ix` calls
/// `to_cpi_handles` unconditionally, so a zero-account CPI cannot be expressed
/// through the derive.
#[derive(Clone, Copy, Default)]
pub struct BuildMemo;

impl<'a> ToCpiAccounts<'a> for BuildMemo {
    fn to_instruction_accounts(&self) -> Vec<InstructionAccount<'a>> {
        Vec::new()
    }

    fn to_cpi_handles(&self) -> Vec<CpiHandle<'a>> {
        Vec::new()
    }

    fn optional_account_sentinel_flags(&self) -> Vec<bool> {
        Vec::new()
    }
}

/// Build the Memo v3 `BuildMemo` instruction.
///
/// `ctx.remaining_accounts` become the instruction's signer list, preserving
/// each handle's signer flag, so a remaining account marked `as_signer()` is
/// forwarded as a signer.
pub fn build_memo_ix(ctx: &CpiContext<'_, BuildMemo>, memo: &[u8]) -> Instruction {
    let signers: Vec<&Address> = ctx.remaining_accounts.iter().map(|h| h.address()).collect();
    spl_memo_interface::instruction::build_memo(&Memo::id(), memo, &signers)
}

/// Attach arbitrary `memo` bytes to a transaction via the Memo program.
///
/// Accounts to sign the memo are passed as `ctx.remaining_accounts`.
///
/// # Example
///
/// ```ignore
/// let cpi_ctx = CpiContext::new(&Memo::id(), BuildMemo)
///     .with_remaining_accounts(vec![authority.cpi_handle()]);
/// anchor_spl::memo::build_memo(cpi_ctx, b"hello")?;
/// ```
pub fn build_memo(ctx: CpiContext<'_, BuildMemo>, memo: &[u8]) -> Result<()> {
    let ix = build_memo_ix(&ctx, memo);
    ctx.invoke_ix(ix)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{build_memo_ix, BuildMemo, MEMO_PROGRAM_ID};
    use {
        anchor_lang::{programs::Memo, CpiContext, Id, ToCpiAccounts},
        solana_pubkey::Pubkey,
    };

    #[test]
    fn memo_program_id_matches_marker_type() {
        assert_eq!(Memo::id(), MEMO_PROGRAM_ID);
        assert_eq!(
            Pubkey::new_from_array(Memo::id().to_bytes()),
            Pubkey::new_from_array(MEMO_PROGRAM_ID.to_bytes())
        );
    }

    #[test]
    fn build_memo_accounts_are_empty() {
        let accounts = BuildMemo;
        assert!(accounts.to_instruction_accounts().is_empty());
        assert!(accounts.to_cpi_handles().is_empty());
        assert!(accounts.optional_account_sentinel_flags().is_empty());
    }

    #[test]
    fn build_memo_ix_has_no_accounts_without_remaining() {
        let program = Memo::id();
        let ctx = CpiContext::new(&program, BuildMemo);
        let ix = build_memo_ix(&ctx, b"hello");
        assert_eq!(ix.program_id, Memo::id());
        assert!(ix.accounts.is_empty(), "memo has no accounts of its own");
    }
}
