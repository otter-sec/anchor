use anchor_lang::prelude::*;

declare_id!("5vHp6xYFQ4pc6D95P7h3CngkzyP4iMfsMghvYDn8ApXK");

#[program]
pub mod mismatched_solana_program {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
