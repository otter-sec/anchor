// Avoiding AccountInfo deprecated msg in anchor context
#![allow(deprecated)]
use anchor_lang::{
    context::CpiContext,
    solana_program::{account_info::AccountInfo, pubkey::Pubkey},
    Accounts, Result,
};

pub fn token_group_initialize<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, TokenGroupInitialize<'info>>,
    update_authority: Option<Pubkey>,
    max_size: u64,
) -> Result<()> {
    let ix = spl_token_group_interface::instruction::initialize_group(
        ctx.accounts.program_id.key,
        ctx.accounts.group.key,
        ctx.accounts.mint.key,
        ctx.accounts.mint_authority.key,
        update_authority,
        max_size,
    );
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.program_id,
            ctx.accounts.group,
            ctx.accounts.mint,
            ctx.accounts.mint_authority,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct TokenGroupInitialize<'info> {
    pub program_id: AccountInfo<'info>,
    pub group: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub mint_authority: AccountInfo<'info>,
}

pub fn token_member_initialize<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, TokenMemberInitialize<'info>>,
) -> Result<()> {
    let ix = spl_token_group_interface::instruction::initialize_member(
        ctx.accounts.program_id.key,
        ctx.accounts.member.key,
        ctx.accounts.member_mint.key,
        ctx.accounts.member_mint_authority.key,
        ctx.accounts.group.key,
        ctx.accounts.group_update_authority.key,
    );
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.program_id,
            ctx.accounts.member,
            ctx.accounts.member_mint,
            ctx.accounts.member_mint_authority,
            ctx.accounts.group,
            ctx.accounts.group_update_authority,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct TokenMemberInitialize<'info> {
    pub program_id: AccountInfo<'info>,
    pub member: AccountInfo<'info>,
    pub member_mint: AccountInfo<'info>,
    pub member_mint_authority: AccountInfo<'info>,
    pub group: AccountInfo<'info>,
    pub group_update_authority: AccountInfo<'info>,
}
