use {
    anchor_lang::prelude::*,
    core::ops::Deref,
    pinocchio::account::AccountView,
    solana_program_error::ProgramError,
};

declare_id!("EDhJyPDycxByBe3wTsN2zppGcRYgM2WR5LQw9f8SFxMF");

#[account]
pub struct Data {
    pub value: u64,
    pub bump: u8,
    pub _pad: [u8; 7],
}

pub struct SpyAccount {
    view: AccountView,
}

impl Deref for SpyAccount {
    type Target = AccountView;

    fn deref(&self) -> &Self::Target {
        &self.view
    }
}

impl AnchorAccount for SpyAccount {
    type Data = AccountView;

    fn load(view: AccountView) -> Result<Self> {
        Ok(Self { view })
    }

    unsafe fn load_mut(view: AccountView) -> Result<Self> {
        anchor_lang::msg!("spy_load_mut");
        if !view.is_writable() {
            return Err(ErrorCode::ConstraintMut.into());
        }
        Ok(Self { view })
    }

    fn account(&self) -> &AccountView {
        &self.view
    }
}

#[program]
pub mod optional_accounts {
    use super::*;

    #[discrim = 0]
    pub fn init_required(ctx: &mut Context<InitRequired>) -> Result<()> {
        ctx.accounts.data.value = 7;
        ctx.accounts.data.bump = ctx.bumps.data;
        ctx.accounts.data._pad = [0; 7];
        Ok(())
    }

    #[discrim = 1]
    pub fn read_optional(ctx: &mut Context<ReadOptional>) -> Result<()> {
        if let Some(data) = ctx.accounts.data.as_ref() {
            if data.value != 7 {
                return Err(ProgramError::InvalidAccountData.into());
            }
        }
        Ok(())
    }

    #[discrim = 2]
    pub fn mut_optional(ctx: &mut Context<MutOptional>) -> Result<()> {
        ctx.accounts.required.value = ctx.accounts.required.value.saturating_add(1);
        if let Some(data) = ctx.accounts.data.as_mut() {
            data.value = data.value.saturating_add(10);
        }
        Ok(())
    }

    #[discrim = 3]
    pub fn optional_with_seeds_bump(ctx: &mut Context<OptionalSeeds>) -> Result<()> {
        if ctx.accounts.data.is_some() != ctx.bumps.data.is_some() {
            return Err(ProgramError::InvalidSeeds.into());
        }
        Ok(())
    }

    #[discrim = 4]
    pub fn optional_with_explicit_bump(_ctx: &mut Context<OptionalExplicitBump>) -> Result<()> {
        Ok(())
    }

    #[discrim = 5]
    pub fn corrupt_bump(ctx: &mut Context<CorruptBump>) -> Result<()> {
        ctx.accounts.data.bump = 0;
        Ok(())
    }

    #[discrim = 6]
    pub fn optional_init_if_needed(ctx: &mut Context<OptionalInitIfNeeded>) -> Result<()> {
        if let Some(data) = ctx.accounts.data.as_mut() {
            data.value = data.value.saturating_add(1);
            data.bump = ctx.bumps.data.unwrap();
            data._pad = [0; 7];
        }
        Ok(())
    }

    #[discrim = 7]
    pub fn optional_zeroed(ctx: &mut Context<OptionalZeroed>) -> Result<()> {
        if let Some(data) = ctx.accounts.data.as_mut() {
            data.value = 11;
            data.bump = 1;
            data._pad = [0; 7];
        }
        Ok(())
    }

    #[discrim = 8]
    pub fn optional_close(_ctx: &mut Context<OptionalClose>) -> Result<()> {
        Ok(())
    }

    #[discrim = 9]
    pub fn optional_constraint(_ctx: &mut Context<OptionalConstraint>) -> Result<()> {
        Ok(())
    }

    #[discrim = 10]
    pub fn optional_spy_mut(_ctx: &mut Context<OptionalSpyMut>) -> Result<()> {
        Ok(())
    }

    #[discrim = 11]
    pub fn double_mut_optional(ctx: &mut Context<DoubleMutOptional>) -> Result<()> {
        if let Some(a) = ctx.accounts.a.as_mut() {
            a.value = a.value.saturating_add(1);
        }
        if let Some(b) = ctx.accounts.b.as_mut() {
            b.value = b.value.saturating_add(1);
        }
        Ok(())
    }
}

#[derive(Accounts)]
pub struct InitRequired {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"data"],
        bump,
    )]
    pub data: Account<Data>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct ReadOptional {
    pub data: Option<Account<Data>>,
}

#[derive(Accounts)]
pub struct MutOptional {
    #[account(mut)]
    pub required: Account<Data>,
    #[account(mut)]
    pub data: Option<Account<Data>>,
}

#[derive(Accounts)]
pub struct OptionalSeeds {
    #[account(seeds = [b"data"], bump)]
    pub data: Option<Account<Data>>,
}

#[derive(Accounts)]
pub struct OptionalExplicitBump {
    #[account(seeds = [b"data"], bump = data.bump)]
    pub data: Option<Account<Data>>,
}

#[derive(Accounts)]
pub struct CorruptBump {
    #[account(mut)]
    pub data: Account<Data>,
}

#[derive(Accounts)]
pub struct OptionalInitIfNeeded {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init_if_needed,
        payer = payer,
        seeds = [b"maybe"],
        bump,
    )]
    pub data: Option<Account<Data>>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct OptionalZeroed {
    #[account(zeroed)]
    pub data: Option<Account<Data>>,
}

#[derive(Accounts)]
pub struct OptionalClose {
    #[account(mut, close = receiver)]
    pub data: Option<Account<Data>>,
    #[account(mut)]
    pub receiver: SystemAccount,
}

#[derive(Accounts)]
pub struct OptionalConstraint {
    #[account(constraint = data.value == 7)]
    pub data: Option<Account<Data>>,
}

#[derive(Accounts)]
pub struct OptionalSpyMut {
    #[account(mut)]
    pub a: Option<SpyAccount>,
    #[account(mut)]
    pub b: Option<SpyAccount>,
}

#[derive(Accounts)]
pub struct DoubleMutOptional {
    #[account(mut)]
    pub a: Option<Account<Data>>,
    #[account(mut)]
    pub b: Option<Account<Data>>,
}
