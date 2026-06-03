use anchor_lang::prelude::*;

declare_id!("E9GKQ5qAkB6N4eGK1Bu3R1a6hNFW2W2Kz6fTn6zkRgZN");

#[program]
pub mod matching_solana_program {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}
