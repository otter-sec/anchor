use {
    super::common::validate_token_2022_program,
    crate::{token_2022::spl_token_2022, token_shared::multisig_signer_addresses},
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct TransferHookInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct TransferHookUpdate<'a> {
    pub mint: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

pub fn transfer_hook_initialize<'a>(
    ctx: CpiContext<'a, TransferHookInitialize<'a>>,
    authority: Option<&Address>,
    transfer_hook_program_id: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::extension::transfer_hook::instruction::initialize(
        &program,
        ctx.accounts.mint.address(),
        authority.copied(),
        transfer_hook_program_id.copied(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn transfer_hook_update<'a>(
    ctx: CpiContext<'a, TransferHookUpdate<'a>>,
    transfer_hook_program_id: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::extension::transfer_hook::instruction::update(
        &program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        transfer_hook_program_id.copied(),
    )?;
    ctx.invoke_ix(ix)
}
