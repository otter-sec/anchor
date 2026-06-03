use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod unchecked_account {
    use super::*;
    pub fn initialize_func_one(_ctx: Context<FuncOne>) -> Result<()> {
        Ok(())
    }

    pub fn initialize_func_two(_ctx: Context<FuncTwo>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct FuncOne<'info> {
    /// CHECK: This account is checked in FuncOne
    unchecked: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct FuncTwo<'info> {
    unchecked: UncheckedAccount<'info>,
}
