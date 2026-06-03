#![allow(unexpected_cfgs)]

use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod test_instruction_validation {
    use super::*;

    // Test 1: Missing parameter - handler only has 1 arg but #[instruction] expects 2
    pub fn missing_instruction_attr(
        _ctx: Context<MissingInstructionAttr>,
        data: u64, // Handler has only 1 parameter
    ) -> Result<()> {
        msg!("Data: {}", data);
        Ok(())
    }

    pub fn no_params(_ctx: Context<NoParams>) -> Result<()> {
        msg!("No params needed");
        Ok(())
    }

    // Test 2: Type mismatch - handler has u64 but #[instruction(...)] has u8
    pub fn type_mismatch(
        _ctx: Context<TypeMismatch>,
        data: u64, // Handler parameter is u64
    ) -> Result<()> {
        msg!("Data: {}", data);
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(data: u64)] // Expects 2 params but handler only has 1
pub struct MissingInstructionAttr<'info> {
    #[account(mut)]
    pub user: Signer<'info>,
}

#[derive(Accounts)]
// No #[instruction(...)] - correct for no params
pub struct NoParams<'info> {
    pub user: Signer<'info>,
}

#[derive(Accounts)]
#[instruction(data: u8)] // Attribute specifies u8 but handler has u64
pub struct TypeMismatch<'info> {
    pub user: Signer<'info>,
}
