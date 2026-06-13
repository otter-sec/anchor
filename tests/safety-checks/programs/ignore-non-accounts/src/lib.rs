use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod ignore_non_accounts {
    use super::*;
    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// CHECK:
    checked1: UncheckedAccount<'info>,
    /// CHECK:
    checked2: UncheckedAccount<'info>,
}

#[derive(Debug)]
pub struct ShouldIgnore1<'info> {
    unchecked1: UncheckedAccount<'info>,
    unchecked2: UncheckedAccount<'info>,
}

pub struct ShouldIgnore2<'info> {
    unchecked1: UncheckedAccount<'info>,
    unchecked2: UncheckedAccount<'info>,
}
