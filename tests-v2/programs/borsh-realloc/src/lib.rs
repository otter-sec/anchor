extern crate alloc;

use anchor_lang::prelude::*;

declare_id!("D51usz545PmMTSqE18F1YSj1RXqvpPhKUUxB6wHPNewT");

#[program]
pub mod borsh_realloc {
    use super::*;

    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.data.items = alloc::vec![1, 2, 3];
        Ok(())
    }

    pub fn grow(ctx: &mut Context<Grow>, new_items: alloc::vec::Vec<u8>) -> Result<()> {
        ctx.accounts.data.items = new_items;
        Ok(())
    }

    pub fn shrink(ctx: &mut Context<Shrink>, new_items: alloc::vec::Vec<u8>) -> Result<()> {
        ctx.accounts.data.items = new_items;
        Ok(())
    }

    pub fn shrink_below_discriminator(_ctx: &mut Context<ShrinkBelowDiscriminator>) -> Result<()> {
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        // disc(8) + borsh Vec len(4) + 3 bytes data = 15
        space = 15,
        seeds = [b"data"],
        bump,
    )]
    pub data: BorshAccount<DynData>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
#[instruction(new_items: alloc::vec::Vec<u8>)]
pub struct Grow {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        mut,
        seeds = [b"data"],
        bump,
        // disc(8) + borsh Vec len(4) + new_items.len()
        realloc = 8 + 4 + new_items.len(),
        realloc_payer = payer,
        realloc_zero = false,
    )]
    pub data: BorshAccount<DynData>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
#[instruction(new_items: alloc::vec::Vec<u8>)]
pub struct Shrink {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        mut,
        seeds = [b"data"],
        bump,
        realloc = 8 + 4 + new_items.len(),
        realloc_payer = payer,
        realloc_zero = false,
    )]
    pub data: BorshAccount<DynData>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct ShrinkBelowDiscriminator {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        mut,
        seeds = [b"data"],
        bump,
        realloc = 4,
        realloc_payer = payer,
        realloc_zero = false,
    )]
    pub data: BorshAccount<DynData>,
    pub system_program: Program<System>,
}

#[account(borsh)]
pub struct DynData {
    pub items: alloc::vec::Vec<u8>,
}
