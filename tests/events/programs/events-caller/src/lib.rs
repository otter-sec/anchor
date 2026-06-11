//! This program CPIs into the `events` program so that events are
//! emitted from an inner instruction (invoke depth >= 2). Clients
//! subscribed to the `events` program must still detect them, which
//! is the scenario from #4450 fixed in #4451.

use anchor_lang::{
    prelude::*,
    solana_program::{instruction::Instruction, program::invoke},
    InstructionData,
};
use events::program::Events;

declare_id!("9Cjn1bYn2naaf4JCHSSEfMGcnUsLGSdKHpYX3wc6NvwU");

#[program]
pub mod events_caller {
    use super::*;

    pub fn cpi_event(ctx: Context<CpiEvent>) -> Result<()> {
        let ix = Instruction {
            program_id: ctx.accounts.events_program.key(),
            accounts: vec![],
            data: events::instruction::TestEvent.data(),
        };
        invoke(&ix, &[ctx.accounts.events_program.to_account_info()])?;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct CpiEvent<'info> {
    pub events_program: Program<'info, Events>,
}
