use anchor_lang::prelude::*;

declare_id!("BDGVEEyvkqzGJDP3pvA9LeLsPanrPP9pykctEAHpK96z");

#[program(legacy_idl)]
pub mod anchor_cli_legacy_idl {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.acc.data.borrow_mut()[8] = 123;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    /// CHECK: ...
    pub acc: UncheckedAccount<'info>,
}
