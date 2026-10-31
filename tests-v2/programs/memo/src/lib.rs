//! Fixture exercising the `anchor_spl::memo` CPI helper.
//!
//! Memo v3 has no fixed accounts: every account that signs the memo arrives as
//! a remaining account. The Memo program account is still declared so the
//! runtime can load the CPI callee.

use {
    anchor_lang::{prelude::*, programs::Memo},
    anchor_spl::memo::build_memo,
};

declare_id!("MemoCp1Test11111111111111111111111111111111");

#[program]
pub mod memo {
    use super::*;

    /// Attach `text` to the transaction, signed by the remaining accounts.
    #[discrim = 0]
    pub fn sign_memo(ctx: &mut Context<SignMemo>, text: String) -> Result<()> {
        build_memo(&ctx.remaining_accounts(), &text)?;
        Ok(())
    }

    /// Require an exact remaining-account count before forwarding, so the test
    /// can prove the signer list is built from the caller's accounts.
    #[discrim = 1]
    pub fn sign_memo_expecting(ctx: &mut Context<SignMemo>, text: String, count: u8) -> Result<()> {
        let remaining = ctx.remaining_accounts();
        if remaining.len() != count as usize {
            return Err(ProgramError::InvalidArgument);
        }
        build_memo(&remaining, &text)?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct SignMemo {
    /// Included in the caller's account list so the CPI callee can be loaded.
    pub memo_program: Program<Memo>,
}
