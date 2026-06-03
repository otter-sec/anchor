use anchor_lang::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod realloc {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        ctx.accounts.sample.data = vec![0];
        ctx.accounts.sample.bump = ctx.bumps.sample;
        Ok(())
    }

    pub fn realloc(ctx: Context<Realloc>, len: u16) -> Result<()> {
        ctx.accounts
            .sample
            .data
            .resize_with(len as usize, Default::default);
        Ok(())
    }

    pub fn realloc2(ctx: Context<Realloc2>, len: u16) -> Result<()> {
        ctx.accounts
            .sample1
            .data
            .resize_with(len as usize, Default::default);

        ctx.accounts
            .sample2
            .data
            .resize_with(len as usize, Default::default);
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        init,
        payer = authority,
        seeds = [b"sample"],
        bump,
        space = Sample::space(1),
    )]
    pub sample: Account<'info, Sample>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(len: u16)]
pub struct Realloc<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sample"],
        bump = sample.bump,
        realloc = Sample::space(len as usize),
        realloc::payer = authority,
        realloc::zero = false,
    )]
    pub sample: Account<'info, Sample>,

    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
#[instruction(len: u16)]
pub struct Realloc2<'info> {
    #[account(mut)]
    pub authority: Signer<'info>,

    #[account(
        mut,
        seeds = [b"sample"],
        bump = sample1.bump,
        realloc = Sample::space(len as usize),
        realloc::payer = authority,
        realloc::zero = false,
    )]
    pub sample1: Account<'info, Sample>,

    #[account(
        mut,
        seeds = [b"sample"],
        bump = sample2.bump,
        realloc = Sample::space((len + 10) as usize),
        realloc::payer = authority,
        realloc::zero = false,
        dup, // Allow duplicate accounts
    )]
    pub sample2: Account<'info, Sample>,

    pub system_program: Program<'info, System>,
}

#[account]
pub struct Sample {
    pub data: Vec<u8>,
    pub bump: u8,
}

impl Sample {
    pub fn space(len: usize) -> usize {
        8 + (4 + len) + 1
    }
}
