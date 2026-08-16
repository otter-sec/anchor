//! This example demonstrates how custom errors and associated error messages
//! can be defined and transparently propagated to clients.

use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
mod errors {
    use super::*;

    pub fn hello(_ctx: Context<Hello>) -> Result<()> {
        err!(MyError::Hello)
    }

    pub fn hello_no_msg(_ctx: Context<Hello>) -> Result<()> {
        err!(MyError::HelloNoMsg)
    }

    pub fn hello_next(_ctx: Context<Hello>) -> Result<()> {
        err!(MyError::HelloNext)
    }

    pub fn test_require(_ctx: Context<Hello>) -> Result<()> {
        require!(false, MyError::Hello);
        Ok(())
    }

    pub fn test_err(_ctx: Context<Hello>) -> Result<()> {
        err!(MyError::Hello)
    }

    pub fn test_program_error(_ctx: Context<Hello>) -> Result<()> {
        Err(ProgramError::InvalidAccountData.into())
    }

    pub fn test_program_error_with_source(_ctx: Context<Hello>) -> Result<()> {
        Err(Error::from(ProgramError::InvalidAccountData).with_source(source!()))
    }

    pub fn mut_error(_ctx: Context<MutError>) -> Result<()> {
        Ok(())
    }

    pub fn has_one_error(_ctx: Context<HasOneError>) -> Result<()> {
        Ok(())
    }

    pub fn signer_error(_ctx: Context<SignerError>) -> Result<()> {
        Ok(())
    }

    pub fn raw_custom_error(_ctx: Context<RawCustomError>) -> Result<()> {
        Ok(())
    }

    pub fn account_not_initialized_error(_ctx: Context<AccountNotInitializedError>) -> Result<()> {
        Ok(())
    }

    pub fn account_owned_by_wrong_program_error(
        _ctx: Context<AccountOwnedByWrongProgramError>,
    ) -> Result<()> {
        Ok(())
    }

    pub fn require_eq(_ctx: Context<RequireEq>) -> Result<()> {
        require_eq!(5241, 124124124, MyError::ValueMismatch);
        Ok(())
    }

    pub fn require_eq_default_error(_ctx: Context<RequireEq>) -> Result<()> {
        require_eq!(5241, 124124124);
        Ok(())
    }

    pub fn require_neq(_ctx: Context<RequireNeq>) -> Result<()> {
        require_neq!(500, 500, MyError::ValueMatch);
        Ok(())
    }

    pub fn require_neq_default_error(_ctx: Context<RequireNeq>) -> Result<()> {
        require_neq!(500, 500);
        Ok(())
    }

    pub fn require_keys_eq(ctx: Context<RequireKeysEq>) -> Result<()> {
        require_keys_eq!(
            ctx.accounts.some_account.key(),
            *ctx.program_id,
            MyError::ValueMismatch
        );
        Ok(())
    }

    pub fn require_keys_eq_default_error(ctx: Context<RequireKeysEq>) -> Result<()> {
        require_keys_eq!(ctx.accounts.some_account.key(), *ctx.program_id);
        Ok(())
    }

    pub fn require_keys_neq(ctx: Context<RequireKeysNeq>) -> Result<()> {
        require_keys_neq!(
            ctx.accounts.some_account.key(),
            *ctx.program_id,
            MyError::ValueMatch
        );
        Ok(())
    }

    pub fn require_keys_neq_default_error(ctx: Context<RequireKeysNeq>) -> Result<()> {
        require_keys_neq!(ctx.accounts.some_account.key(), *ctx.program_id);
        Ok(())
    }

    pub fn require_gt(_ctx: Context<RequireGt>) -> Result<()> {
        require_gt!(5, 10, MyError::ValueLessOrEqual);
        Ok(())
    }

    pub fn require_gt_default_error(_ctx: Context<RequireGt>) -> Result<()> {
        require_gt!(10, 10);
        Ok(())
    }

    pub fn require_gte(_ctx: Context<RequireGt>) -> Result<()> {
        require_gte!(5, 10, MyError::ValueLess);
        Ok(())
    }

    pub fn require_gte_default_error(_ctx: Context<RequireGt>) -> Result<()> {
        require_gte!(5, 10);
        Ok(())
    }

    pub fn try_into_integer(_ctx: Context<Hello>) -> Result<i64> {
        Ok(u64::MAX.try_into()?)
    }
}

#[derive(Accounts)]
pub struct Hello {}

#[derive(Accounts)]
pub struct MutError<'info> {
    #[account(mut)]
    my_account: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct HasOneError<'info> {
    #[account(zero, has_one = owner)]
    my_account: Account<'info, HasOneAccount>,
    owner: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct SignerError<'info> {
    my_account: Signer<'info>,
}

#[account]
pub struct HasOneAccount {
    owner: Pubkey,
}

#[derive(Accounts)]
pub struct RawCustomError<'info> {
    #[account(constraint = *my_account.key == ID @ MyError::HelloCustom)]
    my_account: UncheckedAccount<'info>,
}

#[account]
pub struct AnyAccount {}

#[derive(Accounts)]
pub struct AccountNotInitializedError<'info> {
    not_initialized_account: Account<'info, AnyAccount>,
}

#[derive(Accounts)]
pub struct AccountOwnedByWrongProgramError<'info> {
    pub wrong_account: Account<'info, AnyAccount>,
}

#[derive(Accounts)]
pub struct RequireEq {}

#[derive(Accounts)]
pub struct RequireNeq {}

#[derive(Accounts)]
pub struct RequireGt {}

#[derive(Accounts)]
pub struct RequireGte {}

#[derive(Accounts)]
pub struct RequireKeysEq<'info> {
    pub some_account: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct RequireKeysNeq<'info> {
    pub some_account: UncheckedAccount<'info>,
}

#[error_code]
pub enum MyError {
    #[msg("This is an error message clients will automatically display")]
    Hello,
    HelloNoMsg = 123,
    HelloNext,
    HelloCustom,
    ValueMismatch,
    ValueMatch,
    ValueLess,
    ValueLessOrEqual,
}
