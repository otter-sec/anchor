//! Calls into the `events` program so that its events are emitted from an
//! inner instruction. Used to test client event parsing against the logs of
//! a real transaction rather than hand-written log lines.

use {
    anchor_lang::prelude::*,
    events::{cpi::accounts::TestEvent, program::Events, MyOtherEvent},
};

declare_id!("EventsCa11er1111111111111111111111111111111");

#[program]
pub mod events_caller {
    use super::*;

    /// Emits `MyOtherEvent` from the `events` program at invoke depth 2.
    ///
    /// This program also logs a `MyOtherEvent` of its own before and after
    /// the CPI. The bytes are a valid event, but they are not logged by the
    /// `events` program, so a client listening to `events` must skip them.
    pub fn emit_from_cpi(ctx: Context<EmitFromCpi>) -> Result<()> {
        emit!(MyOtherEvent {
            data: 0,
            label: "caller before".to_string(),
        });

        let cpi_accounts = TestEvent {
            ..Default::default()
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.events_program.key(), cpi_accounts);
        events::cpi::test_event(cpi_ctx)?;

        emit!(MyOtherEvent {
            data: 0,
            label: "caller after".to_string(),
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct EmitFromCpi<'info> {
    pub events_program: Program<'info, Events>,
}
