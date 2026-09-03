use {
    super::common::validate_token_2022_program,
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct TokenGroupInitialize<'a> {
    pub group: CpiHandleMut<'a>,
    pub mint: CpiHandle<'a>,
    #[signer]
    pub mint_authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct TokenMemberInitialize<'a> {
    pub member: CpiHandleMut<'a>,
    pub member_mint: CpiHandle<'a>,
    #[signer]
    pub member_mint_authority: CpiHandle<'a>,
    pub group: CpiHandleMut<'a>,
    #[signer]
    pub group_update_authority: CpiHandle<'a>,
}

pub fn token_group_initialize<'a>(
    ctx: CpiContext<'a, TokenGroupInitialize<'a>>,
    update_authority: Option<&Address>,
    max_size: u64,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_group_interface::instruction::initialize_group(
        &program,
        ctx.accounts.group.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.mint_authority.address(),
        update_authority.copied(),
        max_size,
    );
    ctx.invoke_ix(ix)
}

pub fn token_member_initialize<'a>(
    ctx: CpiContext<'a, TokenMemberInitialize<'a>>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_group_interface::instruction::initialize_member(
        &program,
        ctx.accounts.member.address(),
        ctx.accounts.member_mint.address(),
        ctx.accounts.member_mint_authority.address(),
        ctx.accounts.group.address(),
        ctx.accounts.group_update_authority.address(),
    );
    ctx.invoke_ix(ix)
}
