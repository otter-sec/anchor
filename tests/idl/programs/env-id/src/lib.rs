use anchor_lang::prelude::*;

declare_id!(Pubkey::from_str_const(env!("ENV_ID_ADDRESS")));

#[program]
pub mod env_id {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
