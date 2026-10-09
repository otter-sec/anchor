//! Fixture exercising the `anchor_spl::memo` CPI helper.
//!
//! Memo v3 takes no accounts of its own, so every account to sign the memo
//! arrives as a remaining account. The Memo program itself is an explicit
//! field so the CPI callee is part of the instruction's account keys.

use {
    anchor_lang::{prelude::*, programs::Memo, CpiContext, ToCpiHandle},
    anchor_spl::memo::{build_memo, BuildMemo},
};

declare_id!("MemoCp1Test11111111111111111111111111111111");

#[program]
pub mod memo {
    use super::*;

    /// Attach `text` to the transaction, signed by the remaining accounts.
    #[discrim = 0]
    pub fn sign_memo(ctx: &mut Context<SignMemo>, text: String) -> Result<()> {
        let remaining = ctx.remaining_accounts()?;
        let signers: Vec<_> = remaining
            .iter()
            .map(|account| account.to_cpi_handle().as_signer())
            .collect();
        let program = *ctx.accounts.memo_program.address();
        let cpi_ctx = CpiContext::new(&program, BuildMemo).with_remaining_accounts(signers);
        build_memo(cpi_ctx, text.as_bytes())
    }

    /// Require an exact remaining-account count before forwarding, so the test
    /// can prove the signer list is built from the caller's accounts.
    #[discrim = 1]
    pub fn sign_memo_expecting(ctx: &mut Context<SignMemo>, text: String, count: u8) -> Result<()> {
        let remaining = ctx.remaining_accounts()?;
        if remaining.len() != count as usize {
            return Err(ProgramError::InvalidArgument);
        }
        let signers: Vec<_> = remaining
            .iter()
            .map(|account| account.to_cpi_handle().as_signer())
            .collect();
        let program = *ctx.accounts.memo_program.address();
        let cpi_ctx = CpiContext::new(&program, BuildMemo).with_remaining_accounts(signers);
        build_memo(cpi_ctx, text.as_bytes())
    }
}

#[derive(Accounts)]
pub struct SignMemo {
    pub memo_program: Program<Memo>,
}
