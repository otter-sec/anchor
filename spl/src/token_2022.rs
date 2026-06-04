// Avoiding AccountInfo deprecated msg in anchor context
#![allow(deprecated)]
use anchor_lang::{
    context::CpiContext,
    solana_program::{account_info::AccountInfo, pubkey::Pubkey},
    Accounts, Result,
};
pub use {spl_token_2022::ID, spl_token_2022_interface as spl_token_2022};

#[deprecated(
    since = "0.28.0",
    note = "please use `transfer_checked` or `transfer_checked_with_fee` instead"
)]
pub fn transfer<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, Transfer<'info>>,
    amount: u64,
) -> Result<()> {
    #[allow(deprecated)]
    let ix = spl_token_2022::instruction::transfer(
        &ctx.program_id,
        ctx.accounts.from.key,
        ctx.accounts.to.key,
        ctx.accounts.authority.key,
        &[],
        amount,
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.from, ctx.accounts.to, ctx.accounts.authority],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn transfer_checked<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, TransferChecked<'info>>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let ix = spl_token_2022::instruction::transfer_checked(
        &ctx.program_id,
        ctx.accounts.from.key,
        ctx.accounts.mint.key,
        ctx.accounts.to.key,
        ctx.accounts.authority.key,
        &[],
        amount,
        decimals,
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.from,
            ctx.accounts.mint,
            ctx.accounts.to,
            ctx.accounts.authority,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn mint_to<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, MintTo<'info>>,
    amount: u64,
) -> Result<()> {
    let ix = spl_token_2022::instruction::mint_to(
        &ctx.program_id,
        ctx.accounts.mint.key,
        ctx.accounts.to.key,
        ctx.accounts.authority.key,
        &[],
        amount,
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.to, ctx.accounts.mint, ctx.accounts.authority],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn mint_to_checked<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, MintToChecked<'info>>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let ix = spl_token_2022::instruction::mint_to_checked(
        &ctx.program_id,
        ctx.accounts.mint.key,
        ctx.accounts.to.key,
        ctx.accounts.authority.key,
        &[],
        amount,
        decimals,
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.to, ctx.accounts.mint, ctx.accounts.authority],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn burn<'info>(ctx: CpiContext<'_, '_, '_, 'info, Burn<'info>>, amount: u64) -> Result<()> {
    let ix = spl_token_2022::instruction::burn(
        &ctx.program_id,
        ctx.accounts.from.key,
        ctx.accounts.mint.key,
        ctx.accounts.authority.key,
        &[],
        amount,
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.from, ctx.accounts.mint, ctx.accounts.authority],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn burn_checked<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, BurnChecked<'info>>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let ix = spl_token_2022::instruction::burn_checked(
        &ctx.program_id,
        ctx.accounts.from.key,
        ctx.accounts.mint.key,
        ctx.accounts.authority.key,
        &[],
        amount,
        decimals,
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.from, ctx.accounts.mint, ctx.accounts.authority],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn approve<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, Approve<'info>>,
    amount: u64,
) -> Result<()> {
    let ix = spl_token_2022::instruction::approve(
        &ctx.program_id,
        ctx.accounts.to.key,
        ctx.accounts.delegate.key,
        ctx.accounts.authority.key,
        &[],
        amount,
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.to,
            ctx.accounts.delegate,
            ctx.accounts.authority,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn approve_checked<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, ApproveChecked<'info>>,
    amount: u64,
    decimals: u8,
) -> Result<()> {
    let ix = spl_token_2022::instruction::approve_checked(
        &ctx.program_id,
        ctx.accounts.to.key,
        ctx.accounts.mint.key,
        ctx.accounts.delegate.key,
        ctx.accounts.authority.key,
        &[],
        amount,
        decimals,
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.to,
            ctx.accounts.mint,
            ctx.accounts.delegate,
            ctx.accounts.authority,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn revoke<'info>(ctx: CpiContext<'_, '_, '_, 'info, Revoke<'info>>) -> Result<()> {
    let ix = spl_token_2022::instruction::revoke(
        &ctx.program_id,
        ctx.accounts.source.key,
        ctx.accounts.authority.key,
        &[],
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.source, ctx.accounts.authority],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn initialize_account<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, InitializeAccount<'info>>,
) -> Result<()> {
    let ix = spl_token_2022::instruction::initialize_account(
        &ctx.program_id,
        ctx.accounts.account.key,
        ctx.accounts.mint.key,
        ctx.accounts.authority.key,
    )?;
    anchor_lang::solana_program::program::invoke(
        &ix,
        &[
            ctx.accounts.account,
            ctx.accounts.mint,
            ctx.accounts.authority,
            ctx.accounts.rent,
        ],
    )
    .map_err(Into::into)
}

pub fn initialize_account3<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, InitializeAccount3<'info>>,
) -> Result<()> {
    let ix = spl_token_2022::instruction::initialize_account3(
        &ctx.program_id,
        ctx.accounts.account.key,
        ctx.accounts.mint.key,
        ctx.accounts.authority.key,
    )?;
    anchor_lang::solana_program::program::invoke(&ix, &[ctx.accounts.account, ctx.accounts.mint])
        .map_err(Into::into)
}

pub fn close_account<'info>(ctx: CpiContext<'_, '_, '_, 'info, CloseAccount<'info>>) -> Result<()> {
    let ix = spl_token_2022::instruction::close_account(
        &ctx.program_id,
        ctx.accounts.account.key,
        ctx.accounts.destination.key,
        ctx.accounts.authority.key,
        &[], // TODO: support multisig
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.account,
            ctx.accounts.destination,
            ctx.accounts.authority,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn freeze_account<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, FreezeAccount<'info>>,
) -> Result<()> {
    let ix = spl_token_2022::instruction::freeze_account(
        &ctx.program_id,
        ctx.accounts.account.key,
        ctx.accounts.mint.key,
        ctx.accounts.authority.key,
        &[], // TODO: Support multisig signers.
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.account,
            ctx.accounts.mint,
            ctx.accounts.authority,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn thaw_account<'info>(ctx: CpiContext<'_, '_, '_, 'info, ThawAccount<'info>>) -> Result<()> {
    let ix = spl_token_2022::instruction::thaw_account(
        &ctx.program_id,
        ctx.accounts.account.key,
        ctx.accounts.mint.key,
        ctx.accounts.authority.key,
        &[], // TODO: Support multisig signers.
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.account,
            ctx.accounts.mint,
            ctx.accounts.authority,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn initialize_mint<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, InitializeMint<'info>>,
    decimals: u8,
    authority: &Pubkey,
    freeze_authority: Option<&Pubkey>,
) -> Result<()> {
    let ix = spl_token_2022::instruction::initialize_mint(
        &ctx.program_id,
        ctx.accounts.mint.key,
        authority,
        freeze_authority,
        decimals,
    )?;
    anchor_lang::solana_program::program::invoke(&ix, &[ctx.accounts.mint, ctx.accounts.rent])
        .map_err(Into::into)
}

pub fn initialize_mint2<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, InitializeMint2<'info>>,
    decimals: u8,
    authority: &Pubkey,
    freeze_authority: Option<&Pubkey>,
) -> Result<()> {
    let ix = spl_token_2022::instruction::initialize_mint2(
        &ctx.program_id,
        ctx.accounts.mint.key,
        authority,
        freeze_authority,
        decimals,
    )?;
    anchor_lang::solana_program::program::invoke(&ix, &[ctx.accounts.mint]).map_err(Into::into)
}

pub fn set_authority<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, SetAuthority<'info>>,
    authority_type: spl_token_2022::instruction::AuthorityType,
    new_authority: Option<Pubkey>,
) -> Result<()> {
    let ix = spl_token_2022::instruction::set_authority(
        &ctx.program_id,
        ctx.accounts.account_or_mint.key,
        new_authority.as_ref(),
        authority_type,
        ctx.accounts.current_authority.key,
        &[], // TODO: Support multisig signers.
    )?;
    anchor_lang::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.account_or_mint, ctx.accounts.current_authority],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

pub fn sync_native<'info>(ctx: CpiContext<'_, '_, '_, 'info, SyncNative<'info>>) -> Result<()> {
    let ix = spl_token_2022::instruction::sync_native(&ctx.program_id, ctx.accounts.account.key)?;
    anchor_lang::solana_program::program::invoke(&ix, &[ctx.accounts.account]).map_err(Into::into)
}

pub fn get_account_data_size<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, GetAccountDataSize<'info>>,
    extension_types: &[spl_token_2022::extension::ExtensionType],
) -> Result<u64> {
    let ix = spl_token_2022::instruction::get_account_data_size(
        &ctx.program_id,
        ctx.accounts.mint.key,
        extension_types,
    )?;
    anchor_lang::solana_program::program::invoke(&ix, &[ctx.accounts.mint])?;
    anchor_lang::solana_program::program::get_return_data()
        .ok_or(anchor_lang::solana_program::program_error::ProgramError::InvalidInstructionData)
        .and_then(|(key, data)| {
            if key != ctx.program_id {
                Err(anchor_lang::solana_program::program_error::ProgramError::IncorrectProgramId)
            } else {
                data.try_into().map(u64::from_le_bytes).map_err(|_| {
                    anchor_lang::solana_program::program_error::ProgramError::InvalidInstructionData
                })
            }
        })
        .map_err(Into::into)
}

pub fn initialize_mint_close_authority<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, InitializeMintCloseAuthority<'info>>,
    close_authority: Option<&Pubkey>,
) -> Result<()> {
    let ix = spl_token_2022::instruction::initialize_mint_close_authority(
        &ctx.program_id,
        ctx.accounts.mint.key,
        close_authority,
    )?;
    anchor_lang::solana_program::program::invoke(&ix, &[ctx.accounts.mint]).map_err(Into::into)
}

pub fn initialize_immutable_owner<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, InitializeImmutableOwner<'info>>,
) -> Result<()> {
    let ix = spl_token_2022::instruction::initialize_immutable_owner(
        &ctx.program_id,
        ctx.accounts.account.key,
    )?;
    anchor_lang::solana_program::program::invoke(&ix, &[ctx.accounts.account]).map_err(Into::into)
}

pub fn amount_to_ui_amount<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, AmountToUiAmount<'info>>,
    amount: u64,
) -> Result<String> {
    let ix = spl_token_2022::instruction::amount_to_ui_amount(
        &ctx.program_id,
        ctx.accounts.account.key,
        amount,
    )?;
    anchor_lang::solana_program::program::invoke(&ix, &[ctx.accounts.account])?;
    anchor_lang::solana_program::program::get_return_data()
        .ok_or(anchor_lang::solana_program::program_error::ProgramError::InvalidInstructionData)
        .and_then(|(key, data)| {
            if key != ctx.program_id {
                Err(anchor_lang::solana_program::program_error::ProgramError::IncorrectProgramId)
            } else {
                String::from_utf8(data).map_err(|_| {
                    anchor_lang::solana_program::program_error::ProgramError::InvalidInstructionData
                })
            }
        })
        .map_err(Into::into)
}

pub fn ui_amount_to_amount<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, UiAmountToAmount<'info>>,
    ui_amount: &str,
) -> Result<u64> {
    let ix = spl_token_2022::instruction::ui_amount_to_amount(
        &ctx.program_id,
        ctx.accounts.account.key,
        ui_amount,
    )?;
    anchor_lang::solana_program::program::invoke(&ix, &[ctx.accounts.account])?;
    anchor_lang::solana_program::program::get_return_data()
        .ok_or(anchor_lang::solana_program::program_error::ProgramError::InvalidInstructionData)
        .and_then(|(key, data)| {
            if key != ctx.program_id {
                Err(anchor_lang::solana_program::program_error::ProgramError::IncorrectProgramId)
            } else {
                data.try_into().map(u64::from_le_bytes).map_err(|_| {
                    anchor_lang::solana_program::program_error::ProgramError::InvalidInstructionData
                })
            }
        })
        .map_err(Into::into)
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    pub from: AccountInfo<'info>,
    pub to: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct TransferChecked<'info> {
    pub from: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub to: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct MintTo<'info> {
    pub mint: AccountInfo<'info>,
    pub to: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct MintToChecked<'info> {
    pub mint: AccountInfo<'info>,
    pub to: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Burn<'info> {
    pub mint: AccountInfo<'info>,
    pub from: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct BurnChecked<'info> {
    pub mint: AccountInfo<'info>,
    pub from: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Approve<'info> {
    pub to: AccountInfo<'info>,
    pub delegate: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct ApproveChecked<'info> {
    pub to: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub delegate: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct Revoke<'info> {
    pub source: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct InitializeAccount<'info> {
    pub account: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
    pub rent: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct InitializeAccount3<'info> {
    pub account: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct CloseAccount<'info> {
    pub account: AccountInfo<'info>,
    pub destination: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct FreezeAccount<'info> {
    pub account: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct ThawAccount<'info> {
    pub account: AccountInfo<'info>,
    pub mint: AccountInfo<'info>,
    pub authority: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct InitializeMint<'info> {
    pub mint: AccountInfo<'info>,
    pub rent: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct InitializeMint2<'info> {
    pub mint: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct SetAuthority<'info> {
    pub current_authority: AccountInfo<'info>,
    pub account_or_mint: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct SyncNative<'info> {
    pub account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct GetAccountDataSize<'info> {
    pub mint: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct InitializeMintCloseAuthority<'info> {
    pub mint: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct InitializeImmutableOwner<'info> {
    pub account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct AmountToUiAmount<'info> {
    pub account: AccountInfo<'info>,
}

#[derive(Accounts)]
pub struct UiAmountToAmount<'info> {
    pub account: AccountInfo<'info>,
}

#[derive(Clone)]
pub struct Token2022;

impl anchor_lang::Id for Token2022 {
    fn id() -> Pubkey {
        ID
    }
}
