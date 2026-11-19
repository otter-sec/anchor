use {
    super::serialized_account::{AnchorAccountSerialize, SerializedAccount},
    crate::{AnchorDeserialize, AnchorSerialize, BorshConfig, BORSH_CONFIG},
    solana_program_error::ProgramError,
    wincode::{
        io::{WriteResult, Writer},
        SchemaRead,
    },
};

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
    fn serialize_and_compare(
        value: &T,
        payload: &[u8],
        previous_serialized_len: usize,
    ) -> Result<(usize, bool), ProgramError> {
        let mut writer = CompareWriter {
            expected: payload,
            offset: 0,
            matches: true,
        };
        wincode::config::serialize_into(&mut writer, value, BORSH_CONFIG)
            .map_err(|_| ProgramError::InvalidAccountData)?;

        let serialized_len = writer.offset;
        let zero_len = previous_serialized_len
            .saturating_sub(serialized_len)
            .min(payload.len().saturating_sub(serialized_len));
        let stale_bytes_are_zero = payload
            .get(serialized_len..serialized_len + zero_len)
            .is_some_and(|bytes| bytes.iter().all(|byte| *byte == 0));

        Ok((serialized_len, writer.matches && stale_bytes_are_zero))
    }

    #[inline(always)]
    fn deserialize(buf: &mut &[u8]) -> Result<T, ProgramError> {
        // `wincode::config::deserialize` takes the input by value and would
        // leave `*buf` unchanged. Use `SchemaRead::get` directly so the
        // `&mut &[u8]` reader advances in place per the trait contract.
        <T as SchemaRead<'_, BorshConfig>>::get(buf).map_err(|_| ProgramError::InvalidAccountData)
    }
}

struct CompareWriter<'a> {
    expected: &'a [u8],
    offset: usize,
    matches: bool,
}

impl Writer for CompareWriter<'_> {
    #[inline(always)]
    fn write(&mut self, bytes: &[u8]) -> WriteResult<()> {
        let expected = self.expected[self.offset..]
            .get(..bytes.len())
            .ok_or(wincode::io::WriteError::WriteSizeLimit(bytes.len()))?;
        if expected != bytes {
            self.matches = false;
        }
        self.offset += bytes.len();
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::{AnchorAccountSerialize, BorshSerializer};

    #[test]
    fn streaming_comparison_accepts_payload_larger_than_instruction_buffer() {
        let value = [9u8; 2048];
        assert_eq!(
            BorshSerializer::serialize_and_compare(&value, &value, value.len()).unwrap(),
            (value.len(), true)
        );
    }

    #[test]
    fn streaming_comparison_rejects_serialization_past_account_capacity() {
        let payload = [42, 0, 0];
        assert_eq!(
            BorshSerializer::serialize_and_compare(&42u32, &payload, payload.len()),
            Err(solana_program_error::ProgramError::InvalidAccountData)
        );
        assert_eq!(payload, [42, 0, 0]);
    }

    #[test]
    fn borsh_streaming_comparison_checks_payload_and_stale_zeroing() {
        let payload = [42, 0, 0, 0, 0, 7];
        assert_eq!(
            BorshSerializer::serialize_and_compare(&42u32, &payload, 5).unwrap(),
            (4, true)
        );

        let stale_byte_not_cleared = [42, 0, 0, 0, 1, 7];
        assert_eq!(
            BorshSerializer::serialize_and_compare(&42u32, &stale_byte_not_cleared, 5).unwrap(),
            (4, false)
        );

        let changed_value = [41, 0, 0, 0, 0, 7];
        assert_eq!(
            BorshSerializer::serialize_and_compare(&42u32, &changed_value, 5).unwrap(),
            (4, false)
        );
    }
}

/// Borsh-wire-compatible account type.
///
/// Validates owner, checks discriminator, then encodes/decodes the payload via
/// wincode using [`crate::BORSH_CONFIG`]. The on-disk and CPI wire format is
/// byte-identical to borsh, so off-chain clients that decode with a borsh
/// library still work. Holds a pinocchio borrow guard (`Ref` for `load`,
/// `RefMut` for `load_mut`); `exit()` serializes through the held `RefMut`.
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
