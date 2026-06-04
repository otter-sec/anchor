// Avoiding AccountInfo deprecated msg in anchor context
#![allow(deprecated)]
use {
    anchor_lang::{
        context::CpiContext,
        solana_program::{account_info::AccountInfo, pubkey::Pubkey},
        Accounts, Result,
    },
    spl_token_2022_interface as spl_token_2022,
};

#[deprecated(
    note = "Token-2022 rejects CPI-initiated toggling of the CPI Guard with \
            CpiGuardSettingsLocked, so this wrapper is unreachable from any on-chain program. \
            Build and send the enable instruction client-side with \
            `spl_token_2022_interface::extension::cpi_guard::instruction::enable_cpi_guard`."
)]
pub fn cpi_guard_enable<'info>(ctx: CpiContext<'_, '_, '_, 'info, CpiGuard<'info>>) -> Result<()> {
    let ix = spl_token_2022::extension::cpi_guard::instruction::enable_cpi_guard(
        ctx.accounts.token_program_id.key,
        ctx.accounts.account.key,
        ctx.accounts.owner.key,
        &[],
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.token_program_id,
            ctx.accounts.account,
            ctx.accounts.owner,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[deprecated(
    note = "Token-2022 rejects CPI-initiated toggling of the CPI Guard with \
            CpiGuardSettingsLocked, so this wrapper is unreachable from any on-chain program. \
            Build and send the disable instruction client-side with \
            `spl_token_2022_interface::extension::cpi_guard::instruction::disable_cpi_guard`."
)]
pub fn cpi_guard_disable<'info>(ctx: CpiContext<'_, '_, '_, 'info, CpiGuard<'info>>) -> Result<()> {
    let ix = spl_token_2022::extension::cpi_guard::instruction::disable_cpi_guard(
        ctx.accounts.token_program_id.key,
        ctx.accounts.account.key,
        ctx.accounts.owner.key,
        &[],
    )?;

    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.token_program_id,
            ctx.accounts.account,
            ctx.accounts.owner,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[deprecated(
    note = "CPI Guard enable/disable cannot be invoked via CPI (Token-2022 returns \
            CpiGuardSettingsLocked). Kept only for the deprecated `cpi_guard_enable` / \
            `cpi_guard_disable` wrappers; do not use in new code."
)]
#[derive(Accounts)]
pub struct CpiGuard<'info> {
    pub token_program_id: AccountInfo<'info>,
    pub account: AccountInfo<'info>,
    pub owner: AccountInfo<'info>,
}
