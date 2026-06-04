// Avoiding AccountInfo deprecated msg in anchor context
#![allow(deprecated)]
pub use ::spl_associated_token_account_interface::{
    self as spl_associated_token_account,
    address::{get_associated_token_address, get_associated_token_address_with_program_id},
    program::ID,
};
use anchor_lang::{
    context::CpiContext,
    solana_program::{account_info::AccountInfo, pubkey::Pubkey},
    Accounts, Result,
};

pub fn create<'info>(ctx: CpiContext<'_, '_, '_, 'info, Create<'info>>) -> Result<()> {
    let ix = spl_associated_token_account::instruction::create_associated_token_account(
        ctx.accounts.payer.key,
        ctx.accounts.authority.key,
        ctx.accounts.mint.key,
        ctx.accounts.token_program.key,
    );
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.payer,
            ctx.accounts.associated_token,
            ctx.accounts.authority,
            ctx.accounts.mint,
            ctx.accounts.system_program,
            ctx.accounts.token_program,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn create_idempotent<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, CreateIdempotent<'info>>,
) -> Result<()> {
    let ix = spl_associated_token_account::instruction::create_associated_token_account_idempotent(
        ctx.accounts.payer.key,
        ctx.accounts.authority.key,
        ctx.accounts.mint.key,
        ctx.accounts.token_program.key,
    );
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.payer,
            ctx.accounts.associated_token,
            ctx.accounts.authority,
            ctx.accounts.mint,
            ctx.accounts.system_program,
            ctx.accounts.token_program,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct Create<'info> {
    pub payer: AccountInfo<'info>,
    pub associated_token: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub system_program: AccountInfo<'info>,
    pub token_program: AccountInfo<'info>,
}

type CreateIdempotent<'info> = Create<'info>;

#[derive(Clone)]
pub struct AssociatedToken;

impl anchor_lang::Id for AssociatedToken {
    fn id() -> Pubkey {
        ID
    }
}
