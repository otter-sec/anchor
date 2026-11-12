//! Token-2022 `PermissionedBurn` extension.
//!
//! A mint with this extension rejects the plain `Burn` and `BurnChecked`
//! instructions. Every burn must also be signed by the mint's permissioned
//! burn authority, through the extension's own burn instructions.

use {
    super::common::validate_token_2022_program,
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    pinocchio::{account::AccountView, address::Address},
    pinocchio_token_2022::instructions::{permissioned_burn, ExtensionDiscriminator},
    solana_program_error::ProgramError,
};

const EXTENSION: u8 = ExtensionDiscriminator::PermissionedBurn as u8;

#[derive(ToCpiAccounts)]
pub struct PermissionedBurnInitialize<'a> {
    pub mint: CpiHandleMut<'a>,
}

/// Accounts for [`permissioned_burn`] and [`permissioned_burn_checked`].
#[derive(ToCpiAccounts)]
pub struct PermissionedBurn<'a> {
    pub from: CpiHandleMut<'a>,
    pub mint: CpiHandleMut<'a>,
    /// The authority stored in the mint's `PermissionedBurn` extension.
    #[signer]
    pub permissioned_burn_authority: CpiHandle<'a>,
    /// The owner or delegate of `from`.
    #[signer]
    pub authority: CpiHandle<'a>,
}

pub fn permissioned_burn_initialize<'a>(
    ctx: CpiContext<'a, PermissionedBurnInitialize<'a>>,
    authority: &Address,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    ctx.invoke(&initialize_data(authority))
}

pub fn permissioned_burn<'a>(
    ctx: CpiContext<'a, PermissionedBurn<'a>>,
    amount: u64,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    ctx.invoke(&burn_data(amount))
}

pub fn permissioned_burn_checked<'a>(
    ctx: CpiContext<'a, PermissionedBurn<'a>>,
    amount: u64,
    decimals: u8,
) -> Result<(), ProgramError> {
    validate_token_2022_program(ctx.program)?;
    ctx.invoke(&burn_checked_data(amount, decimals))
}

fn initialize_data(authority: &Address) -> [u8; 34] {
    let mut data = [0; 34];
    data[0] = EXTENSION;
    data[1] = permissioned_burn::Initialize::DISCRIMINATOR;
    data[2..].copy_from_slice(authority.as_ref());
    data
}

fn burn_data(amount: u64) -> [u8; 10] {
    let mut data = [0; 10];
    data[0] = EXTENSION;
    data[1] = permissioned_burn::Burn::<&AccountView>::DISCRIMINATOR;
    data[2..].copy_from_slice(&amount.to_le_bytes());
    data
}

fn burn_checked_data(amount: u64, decimals: u8) -> [u8; 11] {
    let mut data = [0; 11];
    data[0] = EXTENSION;
    data[1] = permissioned_burn::BurnChecked::<&AccountView>::DISCRIMINATOR;
    data[2..10].copy_from_slice(&amount.to_le_bytes());
    data[10] = decimals;
    data
}

#[cfg(test)]
mod tests {
    use super::*;

    // Token-2022 `TokenInstruction::PermissionedBurnExtension` is tag 46. The
    // sub-instructions are Initialize = 0, Burn = 1, BurnChecked = 2.
    #[test]
    fn encodes_initialize() {
        let authority = Address::new_from_array([7; 32]);
        let data = initialize_data(&authority);
        assert_eq!(data[..2], [46, 0]);
        assert_eq!(data[2..], [7; 32]);
    }

    #[test]
    fn encodes_burn() {
        assert_eq!(burn_data(0x0102), [46, 1, 2, 1, 0, 0, 0, 0, 0, 0]);
    }

    #[test]
    fn encodes_burn_checked() {
        assert_eq!(
            burn_checked_data(0x0102, 6),
            [46, 2, 2, 1, 0, 0, 0, 0, 0, 0, 6]
        );
    }
}
