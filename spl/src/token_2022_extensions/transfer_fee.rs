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

pub fn transfer_fee_initialize<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, TransferFeeInitialize<'info>>,
    transfer_fee_config_authority: Option<&Pubkey>,
    withdraw_withheld_authority: Option<&Pubkey>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {
    let ix = spl_token_2022::extension::transfer_fee::instruction::initialize_transfer_fee_config(
        ctx.accounts.token_program_id.key,
        ctx.accounts.mint.key,
        transfer_fee_config_authority,
        withdraw_withheld_authority,
        transfer_fee_basis_points,
        maximum_fee,
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.token_program_id, ctx.accounts.mint],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct TransferFeeInitialize<'info> {
    pub token_program_id: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
}

pub fn transfer_fee_set<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, TransferFeeSetTransferFee<'info>>,
    transfer_fee_basis_points: u16,
    maximum_fee: u64,
) -> Result<()> {
    let ix = spl_token_2022::extension::transfer_fee::instruction::set_transfer_fee(
        ctx.accounts.token_program_id.key,
        ctx.accounts.mint.key,
        ctx.accounts.authority.key,
        &[],
        transfer_fee_basis_points,
        maximum_fee,
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.token_program_id,
            ctx.accounts.mint,
            ctx.accounts.authority,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct TransferFeeSetTransferFee<'info> {
    pub token_program_id: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

pub fn transfer_checked_with_fee<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, TransferCheckedWithFee<'info>>,
    amount: u64,
    decimals: u8,
    fee: u64,
) -> Result<()> {
    let ix = spl_token_2022::extension::transfer_fee::instruction::transfer_checked_with_fee(
        ctx.accounts.token_program_id.key,
        ctx.accounts.source.key,
        ctx.accounts.mint.key,
        ctx.accounts.destination.key,
        ctx.accounts.authority.key,
        &[],
        amount,
        decimals,
        fee,
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.token_program_id,
            ctx.accounts.source,
            ctx.accounts.mint,
            ctx.accounts.destination,
            ctx.accounts.authority,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct TransferCheckedWithFee<'info> {
    pub token_program_id: AccountInfo<'info>,
    pub source: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub destination: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

pub fn harvest_withheld_tokens_to_mint<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, HarvestWithheldTokensToMint<'info>>,
    sources: Vec<AccountInfo<'info>>,
) -> Result<()> {
    let ix = spl_token_2022::extension::transfer_fee::instruction::harvest_withheld_tokens_to_mint(
        ctx.accounts.token_program_id.key,
        ctx.accounts.mint.key,
        sources.iter().map(|a| a.key).collect::<Vec<_>>().as_slice(),
    )?;

    let mut account_infos = vec![ctx.accounts.token_program_id, ctx.accounts.mint];
    account_infos.extend_from_slice(&sources);

    anchor_lang::solana_program::program::invoke_signed(&ix, &account_infos, ctx.signer_seeds)
        .map_err(Into::into)
}

#[derive(Accounts)]
pub struct HarvestWithheldTokensToMint<'info> {
    pub token_program_id: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
}

pub fn withdraw_withheld_tokens_from_mint<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, WithdrawWithheldTokensFromMint<'info>>,
) -> Result<()> {
    let ix =
        spl_token_2022::extension::transfer_fee::instruction::withdraw_withheld_tokens_from_mint(
            ctx.accounts.token_program_id.key,
            ctx.accounts.mint.key,
            ctx.accounts.destination.key,
            ctx.accounts.authority.key,
            &[],
        )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.token_program_id,
            ctx.accounts.mint,
            ctx.accounts.destination,
            ctx.accounts.authority,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct WithdrawWithheldTokensFromMint<'info> {
    pub token_program_id: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub destination: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

pub fn withdraw_withheld_tokens_from_accounts<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, WithdrawWithheldTokensFromAccounts<'info>>,
    sources: Vec<AccountInfo<'info>>,
) -> Result<()> {
    let ix = spl_token_2022::extension::transfer_fee::instruction::withdraw_withheld_tokens_from_accounts(
        ctx.accounts.token_program_id.key,
        ctx.accounts.mint.key,
        ctx.accounts.destination.key,
        ctx.accounts.authority.key,
        &[],
        sources.iter().map(|a| a.key).collect::<Vec<_>>().as_slice(),
    )?;

    let mut account_infos = vec![
        ctx.accounts.token_program_id,
        ctx.accounts.mint,
        ctx.accounts.destination,
        ctx.accounts.authority,
    ];
    account_infos.extend_from_slice(&sources);

    anchor_lang::solana_program::program::invoke_signed(&ix, &account_infos, ctx.signer_seeds)
        .map_err(Into::into)
}

#[derive(Accounts)]
pub struct WithdrawWithheldTokensFromAccounts<'info> {
    pub token_program_id: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub destination: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}
