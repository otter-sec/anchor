use {
    super::common::validate_token_2022_program,
    crate::{token_2022::spl_token_2022, token_shared::multisig_signer_addresses},
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct MemoTransfer<'a> {
    pub account: CpiHandleMut<'a>,
    #[signer]
    pub owner: CpiHandle<'a>,
}

pub fn memo_transfer_initialize<'a>(
    ctx: CpiContext<'a, MemoTransfer<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::extension::memo_transfer::instruction::enable_required_transfer_memos(
        &program,
        ctx.accounts.account.address(),
        ctx.accounts.owner.address(),
        &signer_addresses,
    )?;
    ctx.invoke_ix(ix)
}

pub fn memo_transfer_disable<'a>(
    ctx: CpiContext<'a, MemoTransfer<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix =
        spl_token_2022::extension::memo_transfer::instruction::disable_required_transfer_memos(
            &program,
            ctx.accounts.account.address(),
            ctx.accounts.owner.address(),
            &signer_addresses,
        )?;
    ctx.invoke_ix(ix)
}
