#![cfg_attr(target_arch = "bpf", no_std)]

use anchor_lang::prelude::*;

declare_id!("He11oWor1d111111111111111111111111111111111");

#[program]
pub mod hello_world {
    use super::*;

    pub fn hello(_ctx: &mut Context<Hello>) -> Result<()> {
        msg!("Hello, world!");
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Hello {}
