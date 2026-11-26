use {
    super::serialized_account::{AnchorAccountSerialize, SerializedAccount},
    crate::{AnchorDeserialize, AnchorSerialize, BorshConfig, BORSH_CONFIG},
    solana_program_error::ProgramError,
    solana_program_memory::sol_memcpy,
    wincode::{io, SchemaRead},
};

struct BorshAccountWriter<'a>(&'a mut [u8]);

impl<'a> BorshAccountWriter<'a> {
    #[inline(always)]
    fn take(&mut self, len: usize) -> io::WriteResult<&'a mut [u8]> {
        if len > self.0.len() {
            return Err(io::WriteError::WriteSizeLimit(len));
        }
        let (dst, rest) = core::mem::take(&mut self.0).split_at_mut(len);
        self.0 = rest;
        Ok(dst)
    }
}

impl io::Writer for BorshAccountWriter<'_> {
    #[inline(always)]
    fn write(&mut self, src: &[u8]) -> io::WriteResult<()> {
        let dst = self.take(src.len())?;
        // The VM rejects even same-byte stores to foreign-owned account data.
        if dst != src {
            // SAFETY: `take` returns a disjoint destination of exactly `src.len()` bytes.
            unsafe { sol_memcpy(dst, src, src.len()) };
        }
        Ok(())
    }

    #[inline(always)]
    unsafe fn as_trusted_for(&mut self, len: usize) -> io::WriteResult<impl io::Writer> {
        // Keep the same comparison for writes within wincode's reserved windows.
        Ok(Self(self.take(len)?))
    }
}

/// Borsh-wire codec tag for [`SerializedAccount`]. Zero-sized; selected at the
/// use site via the [`BorshAccount`] alias. The on-chain wire format matches
/// borsh exactly (via [`crate::BORSH_CONFIG`]); the encode/decode path is
/// wincode under the hood.
pub struct BorshSerializer;

impl<T> AnchorAccountSerialize<T> for BorshSerializer
where
    T: AnchorSerialize + AnchorDeserialize,
{
    #[inline(always)]
    fn serialize(value: &T, buf: &mut &mut [u8]) -> Result<(), ProgramError> {
        let mut writer = BorshAccountWriter(core::mem::take(buf));
        wincode::config::serialize_into(&mut writer, value, BORSH_CONFIG)
            .map_err(|_| ProgramError::InvalidAccountData)?;
        *buf = writer.0;
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
/// Serialization skips equal chunks so writable foreign accounts can be used
/// without changing their data. The runtime rejects writes to foreign data.
/// After an ownership change, `exit()` and `release_borrow()` compare the
/// encoded bytes without writing: unchanged bytes succeed and changed bytes
/// return `ProgramError::IllegalOwner`. See [`SerializedAccount`] for details.
///
/// Type alias over [`SerializedAccount<T, BorshSerializer>`]; all inherent
/// methods (`address`, `release_borrow`, `reacquire_borrow_mut`,
/// `reacquire_guard_only`) and trait impls live on `SerializedAccount`.
///
/// ## `#[account(owner = X @ MyErr)]` does NOT surface `MyErr`
///
/// Owner/discriminator validation runs inside `load`/`load_mut`, before
/// derive-level constraints. A mismatch is `ProgramError::IllegalOwner`,
/// not the user's `@ MyErr`. For a custom error, use `UncheckedAccount`
/// with derive-level `owner = X @ MyErr` (you lose the built-in disc/borsh
/// checks).
pub type BorshAccount<T> = SerializedAccount<T, BorshSerializer>;
