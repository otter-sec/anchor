use anchor_lang::prelude::*;

declare_id!("overf1owChecks11111111111111111111111111111");

#[program]
pub mod overflow_checks {
    use super::*;

    pub fn initialize(ctx: Context<Initialize>) -> Result<()> {
        *ctx.accounts.account = OverflowAccount {
            i8_left: i8::MIN,
            i8_right: i8::MAX,
            i16_left: i16::MIN,
            i16_right: i16::MAX,
            i32_left: i32::MIN,
            i32_right: i32::MAX,
            i64_left: i64::MIN,
            i64_right: i64::MAX,
            i128_left: i128::MIN,
            i128_right: i128::MAX,
            u8_left: u8::MIN,
            u8_right: u8::MAX,
            u16_left: u16::MIN,
            u16_right: u16::MAX,
            u32_left: u32::MIN,
            u32_right: u32::MAX,
            u64_left: u64::MIN,
            u64_right: u64::MAX,
            u128_left: u128::MIN,
            u128_right: u128::MAX,
        };

        Ok(())
    }

    pub fn test_overflow_add(ctx: Context<TestOverflowAdd>) -> Result<()> {
        let account = &mut ctx.accounts.account;
        account.i8_right += 1;
        account.i16_right += 1;
        account.i32_right += 1;
        account.i64_right += 1;
        account.i128_right += 1;

        account.u8_right += 1;
        account.u16_right += 1;
        account.u32_right += 1;
        account.u64_right += 1;
        account.u128_right += 1;

        Ok(())
    }

    pub fn test_overflow_sub(ctx: Context<TestOverflowSub>) -> Result<()> {
        let account = &mut ctx.accounts.account;
        account.i8_left -= 1;
        account.i16_left -= 1;
        account.i32_left -= 1;
        account.i64_left -= 1;
        account.i128_left -= 1;

        account.u8_left -= 1;
        account.u16_left -= 1;
        account.u32_left -= 1;
        account.u64_left -= 1;
        account.u128_left -= 1;

        Ok(())
    }

    pub fn test_overflow_mul(ctx: Context<TestOverflowMul>) -> Result<()> {
        let account = &mut ctx.accounts.account;
        account.i8_right *= 2;
        account.i16_right *= 2;
        account.i32_right *= 2;
        account.i64_right *= 2;
        account.i128_right *= 2;

        account.u8_right *= 2;
        account.u16_right *= 2;
        account.u32_right *= 2;
        account.u64_right *= 2;
        account.u128_right *= 2;

        Ok(())
    }
}

#[derive(Accounts)]
pub struct Initialize<'info> {
    #[account(init, payer = payer, space = 8 + OverflowAccount::INIT_SPACE)]
    pub account: Account<'info, OverflowAccount>,
    #[account(mut)]
    pub payer: Signer<'info>,
    pub system_program: Program<'info, System>,
}

#[derive(Accounts)]
pub struct TestOverflowAdd<'info> {
    #[account(mut)]
    pub account: Account<'info, OverflowAccount>,
}

#[derive(Accounts)]
pub struct TestOverflowSub<'info> {
    #[account(mut)]
    pub account: Account<'info, OverflowAccount>,
}

#[derive(Accounts)]
pub struct TestOverflowMul<'info> {
    #[account(mut)]
    pub account: Account<'info, OverflowAccount>,
}

#[account]
#[derive(InitSpace)]
pub struct OverflowAccount {
    pub i8_left: i8,
    pub i8_right: i8,
    pub i16_left: i16,
    pub i16_right: i16,
    pub i32_left: i32,
    pub i32_right: i32,
    pub i64_left: i64,
    pub i64_right: i64,
    pub i128_left: i128,
    pub i128_right: i128,
    pub u8_left: u8,
    pub u8_right: u8,
    pub u16_left: u16,
    pub u16_right: u16,
    pub u32_left: u32,
    pub u32_right: u32,
    pub u64_left: u64,
    pub u64_right: u64,
    pub u128_left: u128,
    pub u128_right: u128,
}
