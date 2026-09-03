use {
    anchor_lang::{bytemuck::bytes_of, prelude::*, programs::Token2022, AnchorAccount, Id},
    anchor_spl::{
        extensions::{
            CpiGuard as SplCpiGuard, ExtensionType as SplExtensionType,
            GroupMemberPointer as SplGroupMemberPointer, GroupPointer as SplGroupPointer,
            MetadataPointer as SplMetadataPointer, MintCloseAuthority as SplMintCloseAuthority,
            PausableAccount as SplPausableAccount, PausableConfig as SplPausableConfig,
            PermanentDelegate as SplPermanentDelegate, TransferFeeAmount as SplTransferFeeAmount,
            TransferHook as SplTransferHook, TransferHookAccount as SplTransferHookAccount,
        },
        token_2022::spl_token_2022::extension::transfer_fee::TransferFeeConfig as SplTransferFeeConfig,
        token_interface::{self, TokenInterfaceAccountExtensions},
        Mint, TokenAccount,
    },
};

declare_id!("5FGXfwXAgDDy76hUXWQEYdBF8ztPezhq7AwibdDtFWvs");

const TAG_STRICT_MINT: u8 = 1;
const TAG_STRICT_TOKEN: u8 = 2;
const TAG_INTERFACE_MINT: u8 = 3;
const TAG_INTERFACE_TOKEN: u8 = 4;
const TAG_INTERFACE_MINT_EXTENSION: u8 = 5;
const TAG_INTERFACE_TOKEN_EXTENSION: u8 = 6;
const EXTENSION_STATUS_FOUND: u8 = 1;
const EXTENSION_STATUS_ILLEGAL_OWNER: u8 = 2;
const EXTENSION_STATUS_ACCESS_ERROR: u8 = 3;

#[program]
pub mod equivalence_spl_v2 {
    use super::*;

    #[discrim = 0]
    pub fn check_strict_mint(ctx: &mut Context<CheckStrictMint>) -> Result<()> {
        let mint = &*ctx.accounts.mint;
        write_mint_observation(
            &ctx.accounts.out,
            TAG_STRICT_MINT,
            mint.supply(),
            mint.decimals(),
            mint.is_initialized(),
            mint.mint_authority().map(|key| key.to_bytes()),
            mint.freeze_authority().map(|key| key.to_bytes()),
        )
    }

    #[discrim = 1]
    pub fn check_strict_token_account(ctx: &mut Context<CheckStrictTokenAccount>) -> Result<()> {
        let token_account = &*ctx.accounts.token_account;
        write_token_account_observation(
            &ctx.accounts.out,
            TAG_STRICT_TOKEN,
            token_account.mint().to_bytes(),
            token_account.owner().to_bytes(),
            token_account.amount(),
            token_account.delegate().map(|key| key.to_bytes()),
            token_account.state(),
            token_account.native_amount(),
            token_account.delegated_amount(),
            token_account.close_authority().map(|key| key.to_bytes()),
        )
    }

    #[discrim = 2]
    pub fn check_interface_mint(ctx: &mut Context<CheckInterfaceMint>) -> Result<()> {
        let mint = &*ctx.accounts.mint;
        write_mint_observation(
            &ctx.accounts.out,
            TAG_INTERFACE_MINT,
            mint.supply(),
            mint.decimals(),
            mint.is_initialized(),
            mint.mint_authority().map(|key| key.to_bytes()),
            mint.freeze_authority().map(|key| key.to_bytes()),
        )
    }

    #[discrim = 3]
    pub fn check_interface_token_account(
        ctx: &mut Context<CheckInterfaceTokenAccount>,
    ) -> Result<()> {
        let token_account = &*ctx.accounts.token_account;
        write_token_account_observation(
            &ctx.accounts.out,
            TAG_INTERFACE_TOKEN,
            token_account.mint().to_bytes(),
            token_account.owner().to_bytes(),
            token_account.amount(),
            token_account.delegate().map(|key| key.to_bytes()),
            token_account.state(),
            token_account.native_amount(),
            token_account.delegated_amount(),
            token_account.close_authority().map(|key| key.to_bytes()),
        )
    }

    #[discrim = 4]
    pub fn check_interface_mint_extension(
        ctx: &mut Context<CheckInterfaceMintExtension>,
        operation: u8,
    ) -> Result<()> {
        match operation {
            0 => write_mint_extension_observation::<SplMetadataPointer>(
                &ctx.accounts.mint,
                &ctx.accounts.out,
                operation,
            ),
            1 => write_mint_extension_observation::<SplGroupPointer>(
                &ctx.accounts.mint,
                &ctx.accounts.out,
                operation,
            ),
            2 => write_mint_extension_observation::<SplGroupMemberPointer>(
                &ctx.accounts.mint,
                &ctx.accounts.out,
                operation,
            ),
            3 => write_mint_extension_observation::<SplTransferHook>(
                &ctx.accounts.mint,
                &ctx.accounts.out,
                operation,
            ),
            4 => write_mint_extension_observation::<SplMintCloseAuthority>(
                &ctx.accounts.mint,
                &ctx.accounts.out,
                operation,
            ),
            5 => write_mint_extension_observation::<SplPermanentDelegate>(
                &ctx.accounts.mint,
                &ctx.accounts.out,
                operation,
            ),
            6 => write_mint_extension_observation::<SplTransferFeeConfig>(
                &ctx.accounts.mint,
                &ctx.accounts.out,
                operation,
            ),
            7 => write_mint_extension_observation::<SplPausableConfig>(
                &ctx.accounts.mint,
                &ctx.accounts.out,
                operation,
            ),
            _ => write_extension_observation(
                &ctx.accounts.out,
                TAG_INTERFACE_MINT_EXTENSION,
                operation,
                EXTENSION_STATUS_ACCESS_ERROR,
                &[],
            ),
        }
    }

    #[discrim = 5]
    pub fn check_interface_token_account_extension(
        ctx: &mut Context<CheckInterfaceTokenAccountExtension>,
        operation: u8,
    ) -> Result<()> {
        match operation {
            0 => write_token_account_extension_observation::<SplTransferFeeAmount>(
                &ctx.accounts.token_account,
                &ctx.accounts.out,
                operation,
            ),
            1 => write_token_account_extension_observation::<SplCpiGuard>(
                &ctx.accounts.token_account,
                &ctx.accounts.out,
                operation,
            ),
            2 => write_token_account_extension_observation::<SplTransferHookAccount>(
                &ctx.accounts.token_account,
                &ctx.accounts.out,
                operation,
            ),
            3 => write_token_account_extension_observation::<SplPausableAccount>(
                &ctx.accounts.token_account,
                &ctx.accounts.out,
                operation,
            ),
            _ => write_extension_observation(
                &ctx.accounts.out,
                TAG_INTERFACE_TOKEN_EXTENSION,
                operation,
                EXTENSION_STATUS_ACCESS_ERROR,
                &[],
            ),
        }
    }
}

#[derive(Accounts)]
pub struct CheckStrictMint {
    pub mint: Account<Mint>,
    #[account(mut)]
    pub out: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CheckStrictTokenAccount {
    pub token_account: Account<TokenAccount>,
    #[account(mut)]
    pub out: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CheckInterfaceMint {
    pub mint: InterfaceAccount<token_interface::Mint>,
    #[account(mut)]
    pub out: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CheckInterfaceTokenAccount {
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
    #[account(mut)]
    pub out: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CheckInterfaceMintExtension {
    pub mint: InterfaceAccount<token_interface::Mint>,
    #[account(mut)]
    pub out: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CheckInterfaceTokenAccountExtension {
    pub token_account: InterfaceAccount<token_interface::TokenAccount>,
    #[account(mut)]
    pub out: UncheckedAccount,
}

fn write_mint_extension_observation<T>(
    mint: &InterfaceAccount<token_interface::Mint>,
    out: &UncheckedAccount,
    operation: u8,
) -> Result<()>
where
    T: SplExtensionType,
{
    if !mint.account().owned_by(&Token2022::id()) {
        return write_extension_observation(
            out,
            TAG_INTERFACE_MINT_EXTENSION,
            operation,
            EXTENSION_STATUS_ILLEGAL_OWNER,
            &[],
        );
    }

    match mint.get_extension::<T>() {
        Ok(extension) => write_extension_observation(
            out,
            TAG_INTERFACE_MINT_EXTENSION,
            operation,
            EXTENSION_STATUS_FOUND,
            bytes_of(extension),
        ),
        Err(_) => write_extension_observation(
            out,
            TAG_INTERFACE_MINT_EXTENSION,
            operation,
            EXTENSION_STATUS_ACCESS_ERROR,
            &[],
        ),
    }
}

fn write_token_account_extension_observation<T>(
    token_account: &InterfaceAccount<token_interface::TokenAccount>,
    out: &UncheckedAccount,
    operation: u8,
) -> Result<()>
where
    T: SplExtensionType,
{
    if !token_account.account().owned_by(&Token2022::id()) {
        return write_extension_observation(
            out,
            TAG_INTERFACE_TOKEN_EXTENSION,
            operation,
            EXTENSION_STATUS_ILLEGAL_OWNER,
            &[],
        );
    }

    match token_account.get_extension::<T>() {
        Ok(extension) => write_extension_observation(
            out,
            TAG_INTERFACE_TOKEN_EXTENSION,
            operation,
            EXTENSION_STATUS_FOUND,
            bytes_of(extension),
        ),
        Err(_) => write_extension_observation(
            out,
            TAG_INTERFACE_TOKEN_EXTENSION,
            operation,
            EXTENSION_STATUS_ACCESS_ERROR,
            &[],
        ),
    }
}

fn write_mint_observation(
    out: &UncheckedAccount,
    tag: u8,
    supply: u64,
    decimals: u8,
    is_initialized: bool,
    mint_authority: Option<[u8; 32]>,
    freeze_authority: Option<[u8; 32]>,
) -> Result<()> {
    let mut out_view = out.account().clone();
    let data = unsafe { out_view.borrow_unchecked_mut() };
    clear(data);
    data[0] = tag;
    data[1..9].copy_from_slice(&supply.to_le_bytes());
    data[9] = decimals;
    data[10] = is_initialized as u8;
    write_option_key(data, 11, mint_authority);
    write_option_key(data, 44, freeze_authority);
    Ok(())
}

fn write_token_account_observation(
    out: &UncheckedAccount,
    tag: u8,
    mint: [u8; 32],
    owner: [u8; 32],
    amount: u64,
    delegate: Option<[u8; 32]>,
    state: u8,
    is_native: Option<u64>,
    delegated_amount: u64,
    close_authority: Option<[u8; 32]>,
) -> Result<()> {
    let mut out_view = out.account().clone();
    let data = unsafe { out_view.borrow_unchecked_mut() };
    clear(data);
    data[0] = tag;
    data[1..33].copy_from_slice(&mint);
    data[33..65].copy_from_slice(&owner);
    data[65..73].copy_from_slice(&amount.to_le_bytes());
    write_option_key(data, 73, delegate);
    data[106] = state;
    write_option_u64(data, 107, is_native);
    data[116..124].copy_from_slice(&delegated_amount.to_le_bytes());
    write_option_key(data, 124, close_authority);
    Ok(())
}

fn write_extension_observation(
    out: &UncheckedAccount,
    tag: u8,
    operation: u8,
    status: u8,
    extension_data: &[u8],
) -> Result<()> {
    let mut out_view = out.account().clone();
    let data = unsafe { out_view.borrow_unchecked_mut() };
    clear(data);
    data[0] = tag;
    data[1] = operation;
    data[2] = status;
    data[3..5].copy_from_slice(&(extension_data.len() as u16).to_le_bytes());
    data[5..5 + extension_data.len()].copy_from_slice(extension_data);
    Ok(())
}

fn clear(data: &mut [u8]) {
    for byte in data.iter_mut() {
        *byte = 0;
    }
}

fn write_option_key(data: &mut [u8], offset: usize, value: Option<[u8; 32]>) {
    if let Some(key) = value {
        data[offset] = 1;
        data[offset + 1..offset + 33].copy_from_slice(&key);
    }
}

fn write_option_u64(data: &mut [u8], offset: usize, value: Option<u64>) {
    if let Some(value) = value {
        data[offset] = 1;
        data[offset + 1..offset + 9].copy_from_slice(&value.to_le_bytes());
    }
}
