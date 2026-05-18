#![allow(unused_variables)]

use anchor_lang::prelude::*;

declare_id!("Externa111111111111111111111111111111111111");

#[constant]
pub const BOOL: bool = false;
#[constant]
pub const U8: u8 = 1;
#[constant]
pub const U16: u16 = 2;
#[constant]
pub const U32: u32 = 4;
#[constant]
pub const U64: u64 = 8;
#[constant]
pub const U128: u128 = 16;
#[constant]
pub const I8: i8 = 1;
#[constant]
pub const I16: i16 = 2;
#[constant]
pub const I32: i32 = 4;
#[constant]
pub const I64: i64 = 8;
#[constant]
pub const I128: i128 = 16;
#[constant]
pub const BYTES: &[u8] = b"abc";
#[constant]
pub const STRING: &str = "abc";
#[constant]
pub const PUBKEY: Pubkey = Pubkey::from_str_const("SomeAdress111111111111111111111111111111111");
#[constant]
pub const ARRAY: [u8; 4] = [1, 2, 3, 4];
#[constant]
pub const OPTION_BOOL: Option<bool> = Some(BOOL);
#[constant]
pub const NESTED_OPTION_BOOL: Option<Option<bool>> = Some(OPTION_BOOL);
#[constant]
pub const OPTION_STRING: Option<&str> = Some(STRING);
#[constant]
pub const NESTED_OPTION_STRING: Option<Option<&str>> = Some(OPTION_STRING);
#[constant]
pub const OPTION_ARRAY: Option<[u8; 4]> = Some(ARRAY);
#[constant]
pub const NESTED_OPTION_ARRAY: Option<Option<[u8; 4]>> = Some(OPTION_ARRAY);

#[program]
pub mod external {
    use super::*;

    pub fn init(_ctx: Context<Init>) -> Result<()> {
        Ok(())
    }

    pub fn update(ctx: Context<Update>, value: u32) -> Result<()> {
        ctx.accounts.my_account.field = value;
        Ok(())
    }

    pub fn update_composite(ctx: Context<UpdateComposite>, value: u32) -> Result<()> {
        ctx.accounts.update.my_account.field = value;
        Ok(())
    }

    // Test the issue described in https://github.com/otter-sec/anchor/issues/3274
    pub fn update_non_instruction_composite(
        ctx: Context<UpdateNonInstructionComposite>,
        value: u32,
    ) -> Result<()> {
        ctx.accounts.non_instruction_update.my_account.field = value;
        Ok(())
    }

    // Test the issue described in https://github.com/otter-sec/anchor/issues/3349
    pub fn update_non_instruction_composite2(
        ctx: Context<UpdateNonInstructionComposite2>,
        value: u32,
    ) -> Result<()> {
        ctx.accounts
            .non_instruction_update_with_different_ident
            .my_account
            .field = value;
        Ok(())
    }

    // Compilation test for whether a defined type (an account in this case) can be used in `cpi` client.
    pub fn test_compilation_defined_type_param(
        _ctx: Context<TestCompilation>,
        _my_account: MyAccount,
    ) -> Result<()> {
        Ok(())
    }

    // Compilation test for whether a custom return type can be specified in `cpi` client
    pub fn test_compilation_return_type(_ctx: Context<TestCompilation>) -> Result<bool> {
        Ok(true)
    }

    // Compilation test for whether `data` can be used as an instruction parameter name
    pub fn test_compilation_data_as_parameter_name(
        _ctx: Context<TestCompilation>,
        data: Vec<u8>,
    ) -> Result<()> {
        Ok(())
    }

    // Compilation test for unsafe zero-copy accounts generated via `declare_program!`
    pub fn test_compilation_packed_account(
        _ctx: Context<TestCompilationPackedAccount>,
    ) -> Result<()> {
        Ok(())
    }

    // Compilation test for an instruction with no accounts
    pub fn test_compilation_no_accounts(_ctx: Context<TestCompilationNoAccounts>) -> Result<()> {
        Ok(())
    }

    // Test optional accounts parsing
    pub fn update_with_optional(ctx: Context<UpdateWithOptional>, value: u32) -> Result<()> {
        ctx.accounts.my_account.field = value;
        Ok(())
    }
}

#[error_code]
pub enum ExternalProgramError {
    // Should have offset 6000
    MyNormalError,
    // Should have offset 6500
    MyErrorWithSpecialOffset = 500,
}

#[derive(Accounts)]
pub struct TestCompilation<'info> {
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct TestCompilationPackedAccount<'info> {
    pub packed_account: AccountLoader<'info, PackedAccount>,
}

#[derive(Accounts)]
pub struct TestCompilationNoAccounts {}

#[derive(Accounts)]
pub struct Init<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,
    #[account(
        init,
        payer = authority,
        space = 8 + 4,
        seeds = [authority.key.as_ref()],
        bump
    )]
    pub my_account: Account<'info, MyAccount>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct Update<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [authority.key.as_ref()], bump)]
    pub my_account: Account<'info, MyAccount>,
}

#[derive(Accounts)]
pub struct UpdateComposite<'info> {
    pub update: Update<'info>,
}

#[derive(Accounts)]
pub struct UpdateNonInstructionComposite<'info> {
    pub non_instruction_update: NonInstructionUpdate<'info>,
}

#[derive(Accounts)]
pub struct UpdateNonInstructionComposite2<'info> {
    // Intenionally using different composite account with the same identifier
    // https://github.com/otter-sec/anchor/pull/3350#pullrequestreview-2425405970
    pub non_instruction_update: NonInstructionUpdate2<'info>,
    pub non_instruction_update_with_different_ident: NonInstructionUpdate<'info>,
}

#[derive(Accounts)]
pub struct NonInstructionUpdate<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [authority.key.as_ref()], bump)]
    pub my_account: Account<'info, MyAccount>,
    pub program: Program<'info, program::External>,
}

#[derive(Accounts)]
pub struct NonInstructionUpdate2<'info> {
    pub program: Program<'info, program::External>,
}

#[derive(Accounts)]
pub struct UpdateWithOptional<'info> {
    pub authority: Signer<'info>,
    #[account(mut, seeds = [authority.key.as_ref()], bump)]
    pub my_account: Account<'info, MyAccount>,
    /// CHECK: Optional account for testing
    #[account(mut)]
    pub optional_account: Option<AccountInfo<'info>>,
}

#[account]
pub struct MyAccount {
    pub field: u32,
}

// Regression test for `declare_program!` codegen on `#[account(zero_copy(unsafe))]`
// https://github.com/otter-sec/anchor/issues/4072
#[account(zero_copy(unsafe))]
pub struct PackedAccount {
    pub a: [u8; 8],
    pub b: [u16; 8],
}

#[event]
pub struct MyEvent {
    pub value: u32,
}
