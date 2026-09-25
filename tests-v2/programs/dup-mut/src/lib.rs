//! Test program for aliased account inputs.
//!
//! Data-carrying wrappers (`Account<T>`, `BorshAccount<T>`, `Box<_>`) hold a
//! borrow on the account's runtime borrow state, so loading a conflicting
//! alias fails. Wrappers that expose no typed data (`Signer`,
//! `SystemAccount`, `UncheckedAccount`) hold no borrow and accept aliases;
//! raw borrows and CPI handles through them still honor the data wrappers'
//! borrows.

use anchor_lang::prelude::*;

declare_id!("2TxMd2YAMi9Sk4xxiJBNkYQNuxK9FwvwwiujuEbKoanz");

#[program]
pub mod dup_mut {
    use super::*;

    pub fn initialize(_ctx: &mut Context<Initialize>, seed: u8) -> Result<()> {
        let _ = seed;
        Ok(())
    }

    pub fn initialize_borsh(_ctx: &mut Context<InitializeBorsh>, seed: u8) -> Result<()> {
        let _ = seed;
        Ok(())
    }

    pub fn touch_two_mut(ctx: &mut Context<TouchTwoMut>, value: u64) -> Result<()> {
        ctx.accounts.data_a.value = value;
        ctx.accounts.data_b.value = value.wrapping_add(1);
        Ok(())
    }

    pub fn touch_three_mut(ctx: &mut Context<TouchThreeMut>, value: u64) -> Result<()> {
        ctx.accounts.data_a.value = value;
        ctx.accounts.data_b.value = value.wrapping_add(1);
        ctx.accounts.data_c.value = value.wrapping_add(2);
        Ok(())
    }

    pub fn touch_mut_and_readonly(
        ctx: &mut Context<TouchMutAndReadonly>,
        value: u64,
    ) -> Result<()> {
        ctx.accounts.data_a.value = value.wrapping_add(ctx.accounts.data_b.value);
        Ok(())
    }

    pub fn touch_readonly_and_mut(
        ctx: &mut Context<TouchReadonlyAndMut>,
        value: u64,
    ) -> Result<()> {
        ctx.accounts.data_b.value = value.wrapping_add(ctx.accounts.data_a.value);
        Ok(())
    }

    pub fn read_two(ctx: &mut Context<ReadTwo>, expected: u64) -> Result<()> {
        require_eq!(ctx.accounts.data_a.value, expected, ProgramError::InvalidAccountData);
        require_eq!(ctx.accounts.data_b.value, expected, ProgramError::InvalidAccountData);
        Ok(())
    }

    pub fn touch_borsh_mut_and_readonly(
        ctx: &mut Context<TouchBorshMutAndReadonly>,
        value: u64,
    ) -> Result<()> {
        ctx.accounts.data_a.value = value.wrapping_add(ctx.accounts.data_b.value);
        Ok(())
    }

    pub fn touch_boxed_mut_and_readonly(
        ctx: &mut Context<TouchBoxedMutAndReadonly>,
        value: u64,
    ) -> Result<()> {
        ctx.accounts.data_a.value = value.wrapping_add(ctx.accounts.data_b.value);
        Ok(())
    }

    pub fn touch_optional_mut_and_mut(
        ctx: &mut Context<TouchOptionalMutAndMut>,
        value: u64,
    ) -> Result<()> {
        if let Some(data_a) = ctx.accounts.data_a.as_mut() {
            data_a.value = value;
        }
        ctx.accounts.data_b.value = value.wrapping_add(1);
        Ok(())
    }

    pub fn touch_nested_two_mut(ctx: &mut Context<TouchNestedTwoMut>, value: u64) -> Result<()> {
        ctx.accounts.pair.data_a.value = value;
        ctx.accounts.pair.data_b.value = value.wrapping_add(1);
        Ok(())
    }

    pub fn touch_nested_mut_readonly(
        ctx: &mut Context<TouchNestedMutReadonly>,
        value: u64,
    ) -> Result<()> {
        ctx.accounts.pair.data_a.value = value.wrapping_add(ctx.accounts.pair.data_b.value);
        Ok(())
    }

    pub fn touch_outer_mut_plus_nested(
        ctx: &mut Context<TouchOuterMutPlusNested>,
        value: u64,
    ) -> Result<()> {
        ctx.accounts.outer.value = value;
        ctx.accounts.pair.data_a.value = value.wrapping_add(1);
        ctx.accounts.pair.data_b.value = value.wrapping_add(2);
        Ok(())
    }

    pub fn signer_roles(ctx: &mut Context<SignerRoles>) -> Result<()> {
        let _ = ctx.accounts.payer.address();
        let _ = ctx.accounts.authority.address();
        Ok(())
    }

    pub fn transfer_to_recipient(ctx: &mut Context<TransferToRecipient>, lamports: u64) -> Result<()> {
        let cpi_accounts = system_program::Transfer {
            from: ctx.accounts.payer.cpi_handle_mut(),
            to: ctx.accounts.recipient.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::transfer(cpi_ctx, lamports)?;
        Ok(())
    }

    pub fn touch_data_and_raw(
        ctx: &mut Context<TouchDataAndRaw>,
        value: u64,
        borrow_raw: bool,
    ) -> Result<()> {
        ctx.accounts.data.value = value;
        if borrow_raw {
            ctx.accounts.raw.account().try_borrow()?;
        }
        Ok(())
    }

    pub fn transfer_to_raw_alias(ctx: &mut Context<TransferToRawAlias>, value: u64) -> Result<()> {
        ctx.accounts.data.value = value;
        let cpi_accounts = system_program::Transfer {
            from: ctx.accounts.payer.cpi_handle_mut(),
            to: ctx.accounts.raw.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::transfer(cpi_ctx, 0)?;
        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(seed: u8)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        seeds = [b"d", &seed.to_le_bytes()],
        bump,
    )]
    pub data: Account<Data>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
#[instruction(seed: u8)]
pub struct InitializeBorsh {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 16,
        seeds = [b"b", &seed.to_le_bytes()],
        bump,
    )]
    pub data: BorshAccount<BorshData>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct TouchTwoMut {
    #[account(mut)]
    pub data_a: Account<Data>,
    #[account(mut)]
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct TouchThreeMut {
    #[account(mut)]
    pub data_a: Account<Data>,
    #[account(mut)]
    pub data_b: Account<Data>,
    #[account(mut)]
    pub data_c: Account<Data>,
}

#[derive(Accounts)]
pub struct TouchMutAndReadonly {
    #[account(mut)]
    pub data_a: Account<Data>,
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct TouchReadonlyAndMut {
    pub data_a: Account<Data>,
    #[account(mut)]
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct ReadTwo {
    pub data_a: Account<Data>,
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct TouchBorshMutAndReadonly {
    #[account(mut)]
    pub data_a: BorshAccount<BorshData>,
    pub data_b: BorshAccount<BorshData>,
}

#[derive(Accounts)]
pub struct TouchBoxedMutAndReadonly {
    #[account(mut)]
    pub data_a: Box<Account<Data>>,
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct TouchOptionalMutAndMut {
    #[account(mut)]
    pub data_a: Option<Account<Data>>,
    #[account(mut)]
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct InnerTwoMut {
    #[account(mut)]
    pub data_a: Account<Data>,
    #[account(mut)]
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct InnerMutReadonly {
    #[account(mut)]
    pub data_a: Account<Data>,
    pub data_b: Account<Data>,
}

#[derive(Accounts)]
pub struct TouchNestedTwoMut {
    pub pair: Nested<InnerTwoMut>,
}

#[derive(Accounts)]
pub struct TouchNestedMutReadonly {
    pub pair: Nested<InnerMutReadonly>,
}

#[derive(Accounts)]
pub struct TouchOuterMutPlusNested {
    #[account(mut)]
    pub outer: Account<Data>,
    pub pair: Nested<InnerTwoMut>,
}

#[derive(Accounts)]
pub struct SignerRoles {
    #[account(mut)]
    pub payer: Signer,
    pub authority: Signer,
}

#[derive(Accounts)]
pub struct TransferToRecipient {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut)]
    pub recipient: SystemAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct TouchDataAndRaw {
    #[account(mut)]
    pub data: Account<Data>,
    pub raw: UncheckedAccount,
}

#[derive(Accounts)]
pub struct TransferToRawAlias {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut)]
    pub data: Account<Data>,
    #[account(mut)]
    pub raw: UncheckedAccount,
    pub system_program: Program<System>,
}

#[account]
pub struct Data {
    pub value: u64,
}

#[account(borsh)]
pub struct BorshData {
    pub value: u64,
}
