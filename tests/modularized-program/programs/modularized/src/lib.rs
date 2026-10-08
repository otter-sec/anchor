//! This program's `Accounts` structs live in nested modules and are referenced
//! with module-qualified paths in `Context<...>`, without any glob re-exports
//! at the crate root.

use anchor_lang::prelude::*;

pub mod instructions;
pub mod state;

declare_id!("Modu1arized11111111111111111111111111111111");

#[program]
pub mod modularized {
    use super::*;

    pub fn init(ctx: Context<instructions::init::Init>, data: u64) -> Result<()> {
        instructions::init::handler(ctx, data)
    }

    pub fn update(ctx: Context<crate::instructions::update::Update>, data: u64) -> Result<()> {
        instructions::update::handler(ctx, data)
    }

    pub fn ping(_ctx: Context<Ping>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Ping<'info> {
    pub payer: Signer<'info>,
}
