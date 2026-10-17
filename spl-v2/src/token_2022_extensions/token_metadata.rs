use {
    super::{common::validate_token_2022_program, spl_token_metadata_interface},
    alloc::borrow::ToOwned,
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
    spl_pod::optional_keys::OptionalNonZeroPubkey,
};

/// Metadata field selector used by the V2 CPI helper.
///
/// The key variant borrows its key instead of storing the canonical SPL
/// `String`, so this public API does not require `alloc`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Field<'a> {
    /// Update the metadata name.
    Name,
    /// Update the metadata symbol.
    Symbol,
    /// Update the metadata URI.
    Uri,
    /// Update a user-defined metadata key.
    Key(&'a str),
}

fn into_spl_field(field: Field<'_>) -> spl_token_metadata_interface::state::Field {
    match field {
        Field::Name => spl_token_metadata_interface::state::Field::Name,
        Field::Symbol => spl_token_metadata_interface::state::Field::Symbol,
        Field::Uri => spl_token_metadata_interface::state::Field::Uri,
        Field::Key(key) => spl_token_metadata_interface::state::Field::Key(key.to_owned()),
    }
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

pub fn token_metadata_initialize<'a>(
    ctx: CpiContext<'a, TokenMetadataInitialize<'a>>,
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

pub fn token_metadata_update_authority<'a>(
    ctx: CpiContext<'a, TokenMetadataUpdateAuthority<'a>>,
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

pub fn token_metadata_update_field<'a>(
    ctx: CpiContext<'a, TokenMetadataUpdateField<'a>>,
    field: Field<'_>,
    value: &str,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let field = into_spl_field(field);
    let ix = spl_token_metadata_interface::instruction::update_field(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.update_authority.address(),
        field,
        value.to_owned(),
    );
    ctx.invoke_ix(ix)
}

pub fn token_metadata_remove_key<'a>(
    ctx: CpiContext<'a, TokenMetadataRemoveKey<'a>>,
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
