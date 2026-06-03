use anchor_lang_v2::prelude::*;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[account]
#[repr(C)]
pub struct MyData {
    pub value: u64,
}

fn payer_seeds() -> [&'static [u8]; 1] {
    [b"payer"]
}

#[program]
pub mod pda_payer_test {
    use super::*;

    #[discrim = 0]
    pub fn init_with_fresh_target(ctx: &mut Context<InitWithFreshTarget>) -> Result<()> {
        ctx.accounts.new_account.value = 42;
        Ok(())
    }

    #[discrim = 1]
    pub fn init_with_pda_target(ctx: &mut Context<InitWithPdaTarget>) -> Result<()> {
        ctx.accounts.new_account.value = 7;
        Ok(())
    }

    #[discrim = 2]
    pub fn init_boxed_with_fresh_target(ctx: &mut Context<InitBoxedWithFreshTarget>) -> Result<()> {
        ctx.accounts.new_account.value = 99;
        Ok(())
    }

    #[discrim = 3]
    pub fn init_with_too_many_payer_seeds(
        ctx: &mut Context<InitWithTooManyPayerSeeds>,
    ) -> Result<()> {
        ctx.accounts.new_account.value = 11;
        Ok(())
    }

    #[discrim = 4]
    pub fn init_with_opaque_payer_seeds(ctx: &mut Context<OpaquePayerSeeds>) -> Result<()> {
        ctx.accounts.new_account.value = 123;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitWithFreshTarget {
    #[account(mut, seeds = [b"payer"], bump)]
    pub pda_payer: SystemAccount,
    #[account(init, payer = pda_payer, space = 8 + core::mem::size_of::<MyData>())]
    pub new_account: Account<MyData>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitWithPdaTarget {
    #[account(mut, seeds = [b"payer"], bump)]
    pub pda_payer: SystemAccount,
    #[account(
        init,
        payer = pda_payer,
        space = 8 + core::mem::size_of::<MyData>(),
        seeds = [b"target"],
        bump
    )]
    pub new_account: Account<MyData>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitBoxedWithFreshTarget {
    #[account(mut, seeds = [b"payer"], bump)]
    pub pda_payer: SystemAccount,
    #[account(init, payer = pda_payer, space = 8 + core::mem::size_of::<MyData>())]
    pub new_account: Box<Account<MyData>>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitWithTooManyPayerSeeds {
    #[account(
        mut,
        seeds = [
            b"0",
            b"1",
            b"2",
            b"3",
            b"4",
            b"5",
            b"6",
            b"7",
            b"8",
            b"9",
            b"a",
            b"b",
            b"c",
            b"d",
            b"e",
            b"f",
            b"g"
        ],
        bump
    )]
    pub pda_payer: SystemAccount,
    #[account(init, payer = pda_payer, space = 8 + core::mem::size_of::<MyData>())]
    pub new_account: Account<MyData>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct OpaquePayerSeeds {
    #[account(mut, seeds = payer_seeds(), bump)]
    pub pda_payer: SystemAccount,
    #[account(init, payer = pda_payer, space = 8 + core::mem::size_of::<MyData>())]
    pub new_account: Account<MyData>,
    pub system_program: Program<System>,
}
