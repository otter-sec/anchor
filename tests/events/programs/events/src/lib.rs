//! This example demonstrates how to emit an event, which can be
//! subscribed to by a client.

use anchor_lang::prelude::*;

declare_id!("2dhGsWUzy5YKUsjZdLHLmkNpUDAXkNa9MYWsPc4Ziqzy");

#[program]
pub mod events {
    use super::*;
    pub fn initialize(_ctx: Context<Initialize>) -> Result<()> {
        emit!(MyEvent {
            data: 5,
            label: "hello".to_string(),
        });
        Ok(())
    }

    pub fn test_event(_ctx: Context<TestEvent>) -> Result<()> {
        emit!(MyOtherEvent {
            data: 6,
            label: "bye".to_string(),
        });
        Ok(())
    }

    pub fn test_event_cpi(ctx: Context<TestEventCpi>) -> Result<()> {
        emit_cpi!(MyOtherEvent {
            data: 7,
            label: "cpi".to_string(),
        });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {}

#[derive(Accounts)]
pub struct TestEvent {}

#[event_cpi]
#[derive(Accounts)]
pub struct TestEventCpi {}

#[event]
pub struct MyEvent {
    pub data: u64,
    pub label: String,
}

#[event]
pub struct MyOtherEvent {
    pub data: u64,
    pub label: String,
}
