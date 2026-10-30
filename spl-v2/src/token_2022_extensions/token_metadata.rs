use {
    super::common::validate_token_2022_program,
    alloc::borrow::ToOwned,
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
    spl_pod::optional_keys::OptionalNonZeroPubkey,
};

/// A metadata field that can be updated without requiring an owned key.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Field<'a> {
    Name,
    Symbol,
    Uri,
    Key(&'a str),
}

#[derive(ToCpiAccounts)]
pub struct TokenMetadataInitialize<'a> {
    pub metadata: CpiHandleMut<'a>,
    pub update_authority: CpiHandle<'a>,
    pub mint: CpiHandle<'a>,
    #[signer]
    pub mint_authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct TokenMetadataUpdateAuthority<'a> {
    pub metadata: CpiHandleMut<'a>,
    #[signer]
    pub current_authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct TokenMetadataUpdateField<'a> {
    pub metadata: CpiHandleMut<'a>,
    #[signer]
    pub update_authority: CpiHandle<'a>,
}

#[derive(ToCpiAccounts)]
pub struct TokenMetadataRemoveKey<'a> {
    pub metadata: CpiHandleMut<'a>,
    #[signer]
    pub update_authority: CpiHandle<'a>,
}

pub fn token_metadata_initialize<'a, const N: usize>(
    ctx: CpiContext<'a, TokenMetadataInitialize<'a>, N>,
    name: &str,
    symbol: &str,
    uri: &str,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_metadata_interface::instruction::initialize(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.update_authority.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.mint_authority.address(),
        name.to_owned(),
        symbol.to_owned(),
        uri.to_owned(),
    );
    ctx.invoke_ix(ix)
}

pub fn token_metadata_update_authority<'a, const N: usize>(
    ctx: CpiContext<'a, TokenMetadataUpdateAuthority<'a>, N>,
    new_authority: Option<&Address>,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let new_authority = OptionalNonZeroPubkey::try_from(new_authority.copied())
        .map_err(|_| ProgramError::InvalidArgument)?;
    let ix = spl_token_metadata_interface::instruction::update_authority(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.current_authority.address(),
        new_authority,
    );
    ctx.invoke_ix(ix)
}

pub fn token_metadata_update_field<'a, const N: usize>(
    ctx: CpiContext<'a, TokenMetadataUpdateField<'a>, N>,
    field: Field<'_>,
    value: &str,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let field = match field {
        Field::Name => spl_token_metadata_interface::state::Field::Name,
        Field::Symbol => spl_token_metadata_interface::state::Field::Symbol,
        Field::Uri => spl_token_metadata_interface::state::Field::Uri,
        Field::Key(key) => spl_token_metadata_interface::state::Field::Key(key.to_owned()),
    };
    let ix = spl_token_metadata_interface::instruction::update_field(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.update_authority.address(),
        field,
        value.to_owned(),
    );
    ctx.invoke_ix(ix)
}

pub fn token_metadata_remove_key<'a, const N: usize>(
    ctx: CpiContext<'a, TokenMetadataRemoveKey<'a>, N>,
    key: &str,
    idempotent: bool,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_metadata_interface::instruction::remove_key(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.update_authority.address(),
        key.to_owned(),
        idempotent,
    );
    ctx.invoke_ix(ix)
}
