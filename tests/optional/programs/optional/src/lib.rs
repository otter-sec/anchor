//! This example demonstrates the ability to use optional accounts in
//! structs deriving `Accounts`.

use anchor_lang::prelude::*;
pub use context::*;

pub mod account;
pub mod context;
declare_id!("FNqz6pqLAwvMSds2FYjR4nKV3moVpPNtvkfGFrqLKrgG");

#[program]
mod optional {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>, value: u64, key: Pubkey) -> Result<()> {
        let optional_pda = &mut ctx.accounts.optional_pda;
        let optional_account = &mut ctx.accounts.optional_account;
        let required = &mut ctx.accounts.required;

        required.data = 0;

        if let Some(data_account) = optional_account {
            if let Some(data_pda) = optional_pda {
                data_pda.data_account = key;
                data_account.data = value;
            } else {
                data_account.data = value * 2;
            }
        }

        Ok(())
    }

    pub fn update(ctx: Context<Update>, value: u64, key: Pubkey, _pda_bump: u8) -> Result<()> {
        if let Some(data_account) = &mut ctx.accounts.optional_account {
            data_account.data = value;
        }
        if let Some(data_account) = &mut ctx.accounts.optional_pda {
            data_account.data_account = key;
        }
        Ok(())
    }

    pub fn realloc(ctx: Context<Realloc>, new_size: u64) -> Result<()> {
        let optional_pda = &ctx.accounts.optional_pda;
        let optional_account = &ctx.accounts.optional_account;
        if let Some(data_pda) = optional_pda {
            let len = data_pda.to_account_info().data_len();
            if len != new_size as usize {
                return err!(OptionalErrors::ReallocFailed);
            }
        }
        if let Some(data_account) = optional_account {
            let len = data_account.to_account_info().data_len();
            if len != new_size as usize {
                return err!(OptionalErrors::ReallocFailed);
            }
        }
        Ok(())
    }

    pub fn close(_ctx: Context<Close>) -> Result<()> {
        Ok(())
    }
}

#[error_code]
pub enum OptionalErrors {
    #[msg("Failed realloc")]
    ReallocFailed,
}
