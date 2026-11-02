use {
    super::common::validate_token_2022_program,
    crate::{token_2022::spl_token_2022, token_shared::add_signers},
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct MetadataPointerInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct MetadataPointerUpdate<'a> {
    pub mint: CpiHandleMut<'a>,
    #[signer(self.signers.is_empty())]
    pub authority: CpiHandle<'a>,
    #[signer]
    pub signers: &'a [CpiHandle<'a>],
}

pub fn metadata_pointer_initialize<'a>(
    ctx: CpiContext<'a, MetadataPointerInitialize<'a>>,
    authority: Option<&Address>,
    metadata_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_2022::extension::metadata_pointer::instruction::initialize(
        &program,
        ctx.accounts.mint.address(),
        authority.copied(),
        metadata_address.copied(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn metadata_pointer_update<'a>(
    ctx: CpiContext<'a, MetadataPointerUpdate<'a>>,
    metadata_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let mut ix = spl_token_2022::extension::metadata_pointer::instruction::update(
        &program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &[],
        metadata_address.copied(),
    )?;
    add_signers(&mut ix, 1, ctx.accounts.signers);
    ctx.invoke_ix(ix)
}
