use {
    super::common::validate_token_2022_program,
    crate::{token_2022::spl_token_2022, token_shared::add_signers},
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct MemoTransfer<'a> {
    pub account: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub owner: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

pub fn memo_transfer_initialize<'a>(
    ctx: CpiContext<'a, MemoTransfer<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let mut ix =
        spl_token_2022::extension::memo_transfer::instruction::enable_required_transfer_memos(
            &program,
            ctx.accounts.account.address(),
            ctx.accounts.owner.address(),
            &[],
        )?;
    add_signers(&mut ix, 1, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}

pub fn memo_transfer_disable<'a>(
    ctx: CpiContext<'a, MemoTransfer<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let mut ix =
        spl_token_2022::extension::memo_transfer::instruction::disable_required_transfer_memos(
            &program,
            ctx.accounts.account.address(),
            ctx.accounts.owner.address(),
            &[],
        )?;
    add_signers(&mut ix, 1, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}
