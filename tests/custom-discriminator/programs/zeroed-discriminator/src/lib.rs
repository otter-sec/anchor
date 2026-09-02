use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWxTWqkZP8eM5uZuN4fKwvVZZzCk");

const fn zeroed_account_disc_inner() -> &'static [u8] {
    &[1 - 1, 2 - 2, 3 - 3, 4 - 4]
}

const ZEROED_ACCOUNT_DISC: &'static [u8] = zeroed_account_disc_inner();

macro_rules! zeroed_account_disc {
    () => {
        ZEROED_ACCOUNT_DISC
    };
}

#[program]
pub mod zeroed_discriminator {
    use super::*;

    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

#[account(discriminator = zeroed_account_disc!())]
pub struct ZeroedAccount {
    pub field: u8,
}
