//! An example of a transfer hook program.
//!
//! This program is intended to implement the SPL Transfer Hook interface,
//! thus allowing Token2022 to call into this program when a transfer occurs.
//!
//! <https://spl.solana.com/token-2022/extensions#transfer-hook>

use {
    anchor_lang::prelude::*,
    anchor_spl::{
        token_2022::{
            spl_token_2022::{
                extension::{
                    transfer_hook::TransferHookAccount, BaseStateWithExtensions,
                    StateWithExtensions,
                },
                state::Account as Token2022Account,
            },
            ID as TOKEN_2022_PROGRAM_ID,
        },
        token_interface::{Mint, TokenAccount},
    },
    spl_discriminator::SplDiscriminate,
    spl_tlv_account_resolution::{account::ExtraAccountMeta, state::ExtraAccountMetaList},
    spl_transfer_hook_interface::{
        error::TransferHookError,
        instruction::{
            ExecuteInstruction, InitializeExtraAccountMetaListInstruction, TransferHookInstruction,
        },
    },
};

declare_id!("9vaEfNU4HquQJuNQ6HYrpJW518a3n4wNUt5mAMY2UUHW");

fn check_token_account_is_transferring(account_data: &[u8]) -> Result<()> {
    let token_account = StateWithExtensions::<Token2022Account>::unpack(account_data)?;
    let extension = token_account.get_extension::<TransferHookAccount>()?;
    if bool::from(extension.transferring) {
        Ok(())
    } else {
        Err(Into::<ProgramError>::into(
            TransferHookError::ProgramCalledOutsideOfTransfer,
        ))?
    }
}

#[program]
pub mod transfer_hook {
    use super::*;

    #[instruction(discriminator = InitializeExtraAccountMetaListInstruction::SPL_DISCRIMINATOR_SLICE)]
    pub fn initialize(ctx: Context<Initialize>, metas: Vec<AnchorExtraAccountMeta>) -> Result<()> {
        let extra_metas_account = &ctx.accounts.extra_metas_account;
        let mint = &ctx.accounts.mint;
        let mint_authority = &ctx.accounts.mint_authority;

        if mint_authority.key()
            != mint.mint_authority.ok_or(Into::<ProgramError>::into(
                TransferHookError::MintHasNoMintAuthority,
            ))?
        {
            Err(Into::<ProgramError>::into(
                TransferHookError::IncorrectMintAuthority,
            ))?;
        }

        let metas: Vec<ExtraAccountMeta> = metas.into_iter().map(|meta| meta.into()).collect();
        let mut data = extra_metas_account.try_borrow_mut_data()?;
        ExtraAccountMetaList::init::<ExecuteInstruction>(&mut data, &metas)?;

        Ok(())
    }

    #[instruction(discriminator = ExecuteInstruction::SPL_DISCRIMINATOR_SLICE)]
    pub fn execute(ctx: Context<Execute>, amount: u64) -> Result<()> {
        let source_account = &ctx.accounts.source_account;
        let destination_account = &ctx.accounts.destination_account;

        check_token_account_is_transferring(&source_account.to_account_info().try_borrow_data()?)?;
        check_token_account_is_transferring(
            &destination_account.to_account_info().try_borrow_data()?,
        )?;

        let data = ctx.accounts.extra_metas_account.try_borrow_data()?;
        ExtraAccountMetaList::check_account_infos::<ExecuteInstruction>(
            &ctx.accounts.to_account_infos(),
            &TransferHookInstruction::Execute { amount }.pack(),
            &ctx.program_id,
            &data,
        )?;

        Ok(())
    }
}

#[derive(Accounts)]
#[instruction(metas: Vec<AnchorExtraAccountMeta>)]
pub struct Initialize<'info> {
    /// CHECK: This account's data is a buffer of TLV data
    #[account(
        init,
        space = ExtraAccountMetaList::size_of(metas.len()).unwrap(),
        // space = 8 + 4 + 2 * 35,
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
        payer = payer,
    )]
    pub extra_metas_account: UncheckedAccount<'info>,

    #[account(
        mint::token_program = TOKEN_2022_PROGRAM_ID,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(mut)]
    pub mint_authority: Signer<'info>,

    pub system_program: Program<'info, System>,

    #[account(mut)]
    pub payer: Signer<'info>,
}

#[derive(Accounts)]
pub struct Execute<'info> {
    #[account(
        token::mint = mint,
        token::authority = owner_delegate,
        token::token_program = TOKEN_2022_PROGRAM_ID,
    )]
    pub source_account: Box<InterfaceAccount<'info, TokenAccount>>,

    #[account(
        mint::token_program = TOKEN_2022_PROGRAM_ID,
    )]
    pub mint: Box<InterfaceAccount<'info, Mint>>,

    #[account(
        token::mint = mint,
        token::token_program = TOKEN_2022_PROGRAM_ID,
    )]
    pub destination_account: Box<InterfaceAccount<'info, TokenAccount>>,

    pub owner_delegate: SystemAccount<'info>,

    /// CHECK: This account's data is a buffer of TLV data
    #[account(
        seeds = [b"extra-account-metas", mint.key().as_ref()],
        bump,
    )]
    pub extra_metas_account: UncheckedAccount<'info>,

    /// CHECK: Example extra PDA for transfer #1
    pub secondary_authority_1: UncheckedAccount<'info>,

    /// CHECK: Example extra PDA for transfer #2
    pub secondary_authority_2: UncheckedAccount<'info>,
}

#[derive(AnchorSerialize, AnchorDeserialize)]
pub struct AnchorExtraAccountMeta {
    pub discriminator: u8,
    pub address_config: [u8; 32],
    pub is_signer: bool,
    pub is_writable: bool,
}
impl From<AnchorExtraAccountMeta> for ExtraAccountMeta {
    fn from(meta: AnchorExtraAccountMeta) -> Self {
        Self {
            discriminator: meta.discriminator,
            address_config: meta.address_config,
            is_signer: meta.is_signer.into(),
            is_writable: meta.is_writable.into(),
        }
    }
}
