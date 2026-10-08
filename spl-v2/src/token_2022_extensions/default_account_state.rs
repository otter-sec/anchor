use {
    super::common::validate_token_2022_program,
    crate::{token_2022::spl_token_2022, token_shared::add_signers},
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct DefaultAccountStateInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct DefaultAccountStateUpdate<'a> {
    pub mint: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub freeze_authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

pub fn default_account_state_initialize<'a>(
    ctx: CpiContext<'a, DefaultAccountStateInitialize<'a>>,
    state: &spl_token_2022::state::AccountState,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix =
        spl_token_2022::extension::default_account_state::instruction::initialize_default_account_state(
            &program,
            ctx.accounts.mint.address(),
            state,
        )?;
    ctx.invoke_ix(ix)
}

pub fn default_account_state_update<'a>(
    ctx: CpiContext<'a, DefaultAccountStateUpdate<'a>>,
    state: &spl_token_2022::state::AccountState,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let mut ix =
        spl_token_2022::extension::default_account_state::instruction::update_default_account_state(
            &program,
            ctx.accounts.mint.address(),
            ctx.accounts.freeze_authority.address(),
            &[],
            state,
        )?;
    add_signers(&mut ix, 1, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}
