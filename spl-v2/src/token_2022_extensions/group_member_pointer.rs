use {
    super::common::validate_token_2022_program,
    crate::{token_2022::spl_token_2022, token_shared::multisig_signer_addresses},
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct GroupMemberPointerInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
}

#[derive(ToCpiAccounts)]
pub struct GroupMemberPointerUpdate<'a> {
    pub mint: CpiHandleMut<'a>,
    #[signer]
    pub authority: CpiHandle<'a>,
}

pub fn group_member_pointer_initialize<'a>(
    ctx: CpiContext<'a, GroupMemberPointerInitialize<'a>>,
    authority: Option<&Address>,
    member_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let ix = spl_token_2022::extension::group_member_pointer::instruction::initialize(
        ctx.program,
        ctx.accounts.mint.address(),
        authority.copied(),
        member_address.copied(),
    )?;
    ctx.invoke_ix(ix)
}

pub fn group_member_pointer_update<'a>(
    ctx: CpiContext<'a, GroupMemberPointerUpdate<'a>>,
    member_address: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let signer_addresses = multisig_signer_addresses(&ctx.remaining_accounts);
    let ix = spl_token_2022::extension::group_member_pointer::instruction::update(
        ctx.program,
        ctx.accounts.mint.address(),
        ctx.accounts.authority.address(),
        &signer_addresses,
        member_address.copied(),
    )?;
    ctx.invoke_ix(ix)
}
