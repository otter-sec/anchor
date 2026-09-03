use {
    super::common::validate_token_2022_program,
    alloc::string::String,
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
    spl_pod::optional_keys::OptionalNonZeroPubkey,
};

pub use spl_token_metadata_interface::state::Field;

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
    name: String,
    symbol: String,
    uri: String,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_metadata_interface::instruction::initialize(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.update_authority.address(),
        ctx.accounts.mint.address(),
        ctx.accounts.mint_authority.address(),
        name,
        symbol,
        uri,
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
    field: Field,
    value: String,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_metadata_interface::instruction::update_field(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.update_authority.address(),
        field,
        value,
    );
    ctx.invoke_ix(ix)
}

pub fn token_metadata_remove_key<'a>(
    ctx: CpiContext<'a, TokenMetadataRemoveKey<'a>>,
    key: String,
    idempotent: bool,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    let program = *ctx.program;
    let ix = spl_token_metadata_interface::instruction::remove_key(
        &program,
        ctx.accounts.metadata.address(),
        ctx.accounts.update_authority.address(),
        key,
        idempotent,
    );
    ctx.invoke_ix(ix)
}
