use anchor_lang::prelude::*;

declare_id!("CustomDiscriminator111111111111111111111111");

const CONST_DISC: &'static [u8] = &[55, 66, 77, 88];

const fn get_disc(input: &str) -> &'static [u8] {
    match input.as_bytes() {
        b"wow" => &[4 + 5, 55 / 5],
        _ => unimplemented!(),
    }
}

#[program]
pub mod custom_discriminator {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn int(_ctx: Context<DefaultIx>) -> Result<()> {
        Ok(())
    }

    #[instruction(discriminator = [1, 2, 3, 4])]
    pub fn array(_ctx: Context<DefaultIx>) -> Result<()> {
        Ok(())
    }

    #[instruction(discriminator = b"hi")]
    pub fn byte_str(_ctx: Context<DefaultIx>) -> Result<()> {
        Ok(())
    }

    #[instruction(discriminator = CONST_DISC)]
    pub fn constant(_ctx: Context<DefaultIx>) -> Result<()> {
        Ok(())
    }

    #[instruction(discriminator = get_disc("wow"))]
    pub fn const_fn(_ctx: Context<DefaultIx>) -> Result<()> {
        Ok(())
    }

    pub fn account(ctx: Context<CustomAccountIx>, field: u8) -> Result<()> {
        ctx.accounts.my_account.field = field;
        Ok(())
    }

    pub fn event(_ctx: Context<DefaultIx>, field: u8) -> Result<()> {
        emit!(MyEvent { field });
        Ok(())
    }
}

#[derive(Accounts)]
pub struct DefaultIx<'info> {
    pub signer: Signer<'info>,
}

#[derive(Accounts)]
pub struct CustomAccountIx<'info> {
    #[account(mut)]
    pub signer: Signer<'info>,
    #[account(
        init,
        payer = signer,
        space = MyAccount::DISCRIMINATOR.len() + core::mem::size_of::<MyAccount>(),
        seeds = [b"my_account"],
        bump
    )]
    pub my_account: Account<'info, MyAccount>,
    pub system_program: Program<'info, System>,
}

#[account(discriminator = 1)]
pub struct MyAccount {
    pub field: u8,
}

#[event(discriminator = 1)]
pub struct MyEvent {
    field: u8,
}
