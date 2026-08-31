use anchor_lang::prelude::*;

declare_id!("BZLiJ62bzRryYp9mRobz47uA66WDgtfTXhhgM25tJyx5");

#[program]
mod hello_anchor {
    use super::*;
    pub fn test_instruction(ctx: Context<InstructionAccounts>) -> Result<()> {
        msg!("PDA: {}", ctx.accounts.pda_account.key());
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InstructionAccounts<'info> {
    pub signer: Signer<'info>,
    #[account(
        seeds = [b"hello_world", signer.key().as_ref()],
        bump,
    )]
    pub pda_account: SystemAccount<'info>,
}
