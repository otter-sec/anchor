use {
    super::serialized_account::{AnchorAccountSerialize, SerializedAccount},
    crate::{BorshConfig, BORSH_CONFIG},
    solana_program_error::ProgramError,
    wincode::{SchemaRead, SchemaWrite},
};

/// Borsh-wire codec tag for [`SerializedAccount`]. Zero-sized; selected at the
/// use site via the [`BorshAccount`] alias. The on-chain wire format matches
/// borsh exactly (via [`crate::BORSH_CONFIG`]); the encode/decode path is
/// wincode under the hood.
pub struct BorshSerializer;

impl<T> AnchorAccountSerialize<T> for BorshSerializer
where
    T: SchemaWrite<BorshConfig, Src = T> + for<'de> SchemaRead<'de, BorshConfig, Dst = T>,
{
    #[inline(always)]
    fn serialize(value: &T, buf: &mut &mut [u8]) -> Result<(), ProgramError> {
        // `&mut [u8]` implements `wincode::io::Writer` by advancing in place
        // via `mem::take` + split. Take ownership of the inner slice so the
        // writer can consume it, then restore the (now-advanced) tail to the
        // caller's outer cursor.
        let mut writer: &mut [u8] = core::mem::take(buf);
        wincode::config::serialize_into(&mut writer, value, BORSH_CONFIG)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        *buf = writer;
        Ok(())
    }

    #[inline(always)]
    fn deserialize(buf: &mut &[u8]) -> Result<T, ProgramError> {
        // `wincode::config::deserialize` takes the input by value and would
        // leave `*buf` unchanged. Use `SchemaRead::get` directly so the
        // `&mut &[u8]` reader advances in place per the trait contract.
        <T as SchemaRead<'_, BorshConfig>>::get(buf).map_err(|_| ProgramError::InvalidAccountData)
    }
}

/// Borsh-wire-compatible account type.
///
/// Validates owner, checks discriminator, then encodes/decodes the payload via
/// wincode using [`crate::BORSH_CONFIG`]. The on-disk and CPI wire format is
/// byte-identical to borsh, so off-chain clients that decode with a borsh
/// library still work. Holds a pinocchio borrow guard (`Ref` for `load`,
/// `RefMut` for `load_mut`); `exit()` serializes through the held `RefMut`.
///
/// Type alias over [`SerializedAccount<T, BorshSerializer, O>`]; all inherent
/// methods (`address`, `release_borrow`, `reacquire_borrow_mut`,
/// `reacquire_guard_only`) and trait impls live on `SerializedAccount`.
///
/// The optional owner marker defaults to `T`, preserving the usual
/// `T: Owner` behavior. Use `BorshAccount<T, ForeignOwner>` when the account
/// data type is declared with `#[account(borsh)]` but owned by another program.
///
/// ## `#[account(owner = X @ MyErr)]` does NOT surface `MyErr`
///
/// Owner/discriminator validation runs inside `load`/`load_mut`, before
/// derive-level constraints. A mismatch is `ProgramError::IllegalOwner`,
/// not the user's `@ MyErr`. For a custom error, use `UncheckedAccount`
/// with derive-level `owner = X @ MyErr` (you lose the built-in disc/borsh
/// checks).
pub type BorshAccount<T, O = T> = SerializedAccount<T, BorshSerializer, O>;
