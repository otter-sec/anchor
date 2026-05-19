use anchor_lang_v2::prelude::*;

declare_id!("6NxceYZNn23ERJ6rDPENG8iT5bz7osPqiQeWukHaYsRs");

#[account]
pub struct Counter {
    pub value: u64,
}

#[program]
pub mod dispatch_remaining {
    use super::*;

    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.counter.value = 1;
        Ok(())
    }

    #[discrim = 1]
    pub fn needs_two_accounts(ctx: &mut Context<NeedsTwoAccounts>) -> Result<()> {
        let _ = ctx.accounts.counter.value;
        let _ = ctx.accounts.marker.address();
        Ok(())
    }

    #[discrim = 2]
    pub fn read_remaining_once(ctx: &mut Context<ReadRemaining>, expected_count: u8) -> Result<()> {
        let remaining = ctx.remaining_accounts()?;
        if remaining.len() != expected_count as usize {
            return Err(ProgramError::InvalidArgument.into());
        }
        if expected_count == 2 && remaining[0].address() != ctx.accounts.counter.account().address()
        {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    #[discrim = 3]
    pub fn read_remaining_twice(ctx: &mut Context<ReadRemaining>) -> Result<()> {
        let first = ctx.remaining_accounts()?;
        let second = ctx.remaining_accounts()?;
        if first.len() != 2 || second.len() != 2 {
            return Err(ProgramError::InvalidArgument.into());
        }
        if first[0].address() != second[0].address() || first[1].address() != second[1].address() {
            return Err(ProgramError::InvalidAccountData.into());
        }
        Ok(())
    }

    #[discrim = 4]
    pub fn mutate_then_read_remaining(
        ctx: &mut Context<MutateThenReadRemaining>,
        value: u64,
    ) -> Result<()> {
        ctx.accounts.counter.value = value;
        let remaining = ctx.remaining_accounts()?;
        if remaining.len() != 1 {
            return Err(ProgramError::InvalidArgument.into());
        }
        Ok(())
    }

    #[discrim = 5]
    pub fn arg_echo(_ctx: &mut Context<ReadRemaining>, value: u64, tag: [u8; 4]) -> Result<()> {
        if value != 0x0102_0304_0506_0708 || tag != *b"echo" {
            return Err(ProgramError::InvalidInstructionData.into());
        }
        Ok(())
    }

    #[discrim = 6]
    pub fn optional_mut_then_read_remaining(
        ctx: &mut Context<OptionalMutThenReadRemaining>,
        expected_count: u8,
    ) -> Result<()> {
        if let Some(counter) = ctx.accounts.counter.as_mut() {
            counter.value = counter.value.saturating_add(1);
        }
        let remaining = ctx.remaining_accounts()?;
        if remaining.len() != expected_count as usize {
            return Err(ProgramError::InvalidArgument.into());
        }
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
        space = 8 + core::mem::size_of::<Counter>(),
        seeds = [b"counter"],
        bump,
    )]
    pub counter: Account<Counter>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct NeedsTwoAccounts {
    pub counter: Account<Counter>,
    pub marker: UncheckedAccount,
}

#[derive(Accounts)]
pub struct ReadRemaining {
    pub counter: Account<Counter>,
}

#[derive(Accounts)]
pub struct MutateThenReadRemaining {
    #[account(mut)]
    pub counter: Account<Counter>,
}

#[derive(Accounts)]
pub struct OptionalMutThenReadRemaining {
    #[account(mut)]
    pub counter: Option<Account<Counter>>,
}
