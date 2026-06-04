// Avoiding AccountInfo deprecated msg in anchor context
#![allow(deprecated)]
pub use crate::solana_program::system_program::ID;
use crate::{prelude::*, solana_program::pubkey::Pubkey};

#[derive(Debug, Clone)]
pub struct System;

impl anchor_lang::Id for System {
    fn id() -> Pubkey {
        ID
    }
}

pub fn advance_nonce_account<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, AdvanceNonceAccount<'info>>,
) -> Result<()> {
    let ix = crate::solana_program::system_instruction::advance_nonce_account(
        ctx.accounts.nonce.key,
        ctx.accounts.authorized.key,
    );
    crate::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.nonce,
            ctx.accounts.recent_blockhashes,
            ctx.accounts.authorized,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct AdvanceNonceAccount<'info> {
    pub nonce: AccountInfo<'info>,
    pub authorized: AccountInfo<'info>,
    pub recent_blockhashes: AccountInfo<'info>,
}

pub fn allocate<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, Allocate<'info>>,
    space: u64,
) -> Result<()> {
    let ix = crate::solana_program::system_instruction::allocate(
        ctx.accounts.account_to_allocate.key,
        space,
    );
    crate::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.account_to_allocate],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct Allocate<'info> {
    pub account_to_allocate: AccountInfo<'info>,
}

pub fn allocate_with_seed<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, AllocateWithSeed<'info>>,
    seed: &str,
    space: u64,
    owner: &Pubkey,
) -> Result<()> {
    let ix = crate::solana_program::system_instruction::allocate_with_seed(
        ctx.accounts.account_to_allocate.key,
        ctx.accounts.base.key,
        seed,
        space,
        owner,
    );
    crate::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.account_to_allocate, ctx.accounts.base],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct AllocateWithSeed<'info> {
    pub account_to_allocate: AccountInfo<'info>,
    pub base: AccountInfo<'info>,
}

pub fn assign<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, Assign<'info>>,
    owner: &Pubkey,
) -> Result<()> {
    let ix = crate::solana_program::system_instruction::assign(
        ctx.accounts.account_to_assign.key,
        owner,
    );
    crate::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.account_to_assign],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct Assign<'info> {
    pub account_to_assign: AccountInfo<'info>,
}

pub fn assign_with_seed<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, AssignWithSeed<'info>>,
    seed: &str,
    owner: &Pubkey,
) -> Result<()> {
    let ix = crate::solana_program::system_instruction::assign_with_seed(
        ctx.accounts.account_to_assign.key,
        ctx.accounts.base.key,
        seed,
        owner,
    );
    crate::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.account_to_assign, ctx.accounts.base],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct AssignWithSeed<'info> {
    pub account_to_assign: AccountInfo<'info>,
    pub base: AccountInfo<'info>,
}

pub fn authorize_nonce_account<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, AuthorizeNonceAccount<'info>>,
    new_authority: &Pubkey,
) -> Result<()> {
    let ix = crate::solana_program::system_instruction::authorize_nonce_account(
        ctx.accounts.nonce.key,
        ctx.accounts.authorized.key,
        new_authority,
    );
    crate::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.nonce, ctx.accounts.authorized],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct AuthorizeNonceAccount<'info> {
    pub nonce: AccountInfo<'info>,
    pub authorized: AccountInfo<'info>,
}

pub fn create_account<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, CreateAccount<'info>>,
    lamports: u64,
    space: u64,
    owner: &Pubkey,
) -> Result<()> {
    let ix = crate::solana_program::system_instruction::create_account(
        ctx.accounts.from.key,
        ctx.accounts.to.key,
        lamports,
        space,
        owner,
    );
    crate::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.from, ctx.accounts.to],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct CreateAccount<'info> {
    pub from: AccountInfo<'info>,
    pub to: AccountInfo<'info>,
}

pub fn create_account_with_seed<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, CreateAccountWithSeed<'info>>,
    seed: &str,
    lamports: u64,
    space: u64,
    owner: &Pubkey,
) -> Result<()> {
    let ix = crate::solana_program::system_instruction::create_account_with_seed(
        ctx.accounts.from.key,
        ctx.accounts.to.key,
        ctx.accounts.base.key,
        seed,
        lamports,
        space,
        owner,
    );
    crate::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.from, ctx.accounts.to, ctx.accounts.base],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct CreateAccountWithSeed<'info> {
    pub from: AccountInfo<'info>,
    pub to: AccountInfo<'info>,
    pub base: AccountInfo<'info>,
}

pub fn create_nonce_account<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, CreateNonceAccount<'info>>,
    lamports: u64,
    authority: &Pubkey,
) -> Result<()> {
    let ixs = crate::solana_program::system_instruction::create_nonce_account(
        ctx.accounts.from.key,
        ctx.accounts.nonce.key,
        authority,
        lamports,
    );
    crate::solana_program::program::invoke_signed(
        &ixs[0],
        &[ctx.accounts.from, ctx.accounts.nonce.clone()],
        ctx.signer_seeds,
    )?;

    crate::solana_program::program::invoke_signed(
        &ixs[1],
        &[
            ctx.accounts.nonce,
            ctx.accounts.recent_blockhashes,
            ctx.accounts.rent,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct CreateNonceAccount<'info> {
    pub from: AccountInfo<'info>,
    pub nonce: AccountInfo<'info>,
    pub recent_blockhashes: AccountInfo<'info>,
    pub rent: AccountInfo<'info>,
}

pub fn create_nonce_account_with_seed<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, CreateNonceAccountWithSeed<'info>>,
    lamports: u64,
    seed: &str,
    authority: &Pubkey,
) -> Result<()> {
    let ixs = crate::solana_program::system_instruction::create_nonce_account_with_seed(
        ctx.accounts.from.key,
        ctx.accounts.nonce.key,
        ctx.accounts.base.key,
        seed,
        authority,
        lamports,
    );
    crate::solana_program::program::invoke_signed(
        &ixs[0],
        &[
            ctx.accounts.from,
            ctx.accounts.nonce.clone(),
            ctx.accounts.base,
        ],
        ctx.signer_seeds,
    )?;

    crate::solana_program::program::invoke_signed(
        &ixs[1],
        &[
            ctx.accounts.nonce,
            ctx.accounts.recent_blockhashes,
            ctx.accounts.rent,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct CreateNonceAccountWithSeed<'info> {
    pub from: AccountInfo<'info>,
    pub nonce: AccountInfo<'info>,
    pub base: AccountInfo<'info>,
    pub recent_blockhashes: AccountInfo<'info>,
    pub rent: AccountInfo<'info>,
}

pub fn transfer<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, Transfer<'info>>,
    lamports: u64,
) -> Result<()> {
    let ix = crate::solana_program::system_instruction::transfer(
        ctx.accounts.from.key,
        ctx.accounts.to.key,
        lamports,
    );
    crate::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.from, ctx.accounts.to],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct Transfer<'info> {
    pub from: AccountInfo<'info>,
    pub to: AccountInfo<'info>,
}

pub fn transfer_with_seed<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, TransferWithSeed<'info>>,
    from_seed: String,
    from_owner: &Pubkey,
    lamports: u64,
) -> Result<()> {
    let ix = crate::solana_program::system_instruction::transfer_with_seed(
        ctx.accounts.from.key,
        ctx.accounts.base.key,
        from_seed,
        from_owner,
        ctx.accounts.to.key,
        lamports,
    );
    crate::solana_program::program::invoke_signed(
        &ix,
        &[ctx.accounts.from, ctx.accounts.base, ctx.accounts.to],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct TransferWithSeed<'info> {
    pub from: AccountInfo<'info>,
    pub base: AccountInfo<'info>,
    pub to: AccountInfo<'info>,
}

pub fn withdraw_nonce_account<'info>(
    ctx: CpiContext<'_, '_, '_, 'info, WithdrawNonceAccount<'info>>,
    lamports: u64,
) -> Result<()> {
    let ix = crate::solana_program::system_instruction::withdraw_nonce_account(
        ctx.accounts.nonce.key,
        ctx.accounts.authorized.key,
        ctx.accounts.to.key,
        lamports,
    );
    crate::solana_program::program::invoke_signed(
        &ix,
        &[
            ctx.accounts.nonce,
            ctx.accounts.to,
            ctx.accounts.recent_blockhashes,
            ctx.accounts.rent,
            ctx.accounts.authorized,
        ],
        ctx.signer_seeds,
    )
    .map_err(Into::into)
}

#[derive(Accounts)]
pub struct WithdrawNonceAccount<'info> {
    pub nonce: AccountInfo<'info>,
    pub to: AccountInfo<'info>,
    pub recent_blockhashes: AccountInfo<'info>,
    pub rent: AccountInfo<'info>,
    pub authorized: AccountInfo<'info>,
}
