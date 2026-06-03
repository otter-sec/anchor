#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod test_instruction_validation {
    use super::*;

    pub fn missing_instruction_attr(
        _ctx: Context<MissingInstructionAttr>,
        data: u64, // Handler has u64
        _ehe: u64,
    ) -> Result<()> {
        msg!("Data: {}, Ehe: ", data);
        Ok(())
    }

    pub fn no_params(_ctx: Context<NoParams>) -> Result<()> {
        msg!("No params needed");
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(data: u64, ehe: u64)]
pub struct MissingInstructionAttr<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
pub struct NoParams<'info> {
    pub user: Signer<'info>,
}
