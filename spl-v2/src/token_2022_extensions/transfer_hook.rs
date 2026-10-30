use {
    super::common::validate_token_2022_program,
    crate::{token_2022::spl_token_2022, token_shared::add_signers},
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
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
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
    let mut ix = spl_token_2022::extension::transfer_hook::instruction::update(
        &program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        transfer_hook_program_id.copied(),
    )?;
    add_signers(&mut ix, 1, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}
