use crate::{Discriminator, InstructionDataBuffer};

// Sha256(anchor:event)[..8], matching Anchor v1's self-CPI event instruction tag.
pub const EVENT_IX_TAG: u64 = 0x1d9acb512ea545e4;
pub const EVENT_IX_TAG_LE: &[u8] = &EVENT_IX_TAG.to_le_bytes();

/// Trait for event structs. Implemented by the `#[event]` attribute macro.
///
/// The optional `#[event(name = "...")]` argument overrides the event's
/// wire and IDL name, which can be used to disambiguate same-named events in
/// different modules.
///
/// Two serialization modes are emitted by the macro, both exposed via the
/// single `data()` entry point:
/// - default (`#[event]`) — wincode with a borsh-compatible wire format
///   (`BORSH_CONFIG`: u8 enum tags + fixed `u32` LE length prefixes), so
///   off-chain consumers decoding as borsh see the same bytes. Supports
///   `Vec`/`String`/`Option`/enums and is materially cheaper than borsh on
///   SBF (3–10× fewer CUs). The macro also derives `AnchorSerialize` and
///   `AnchorDeserialize`.
/// - opt-in (`#[event(bytemuck)]`) — zero-copy `copy_nonoverlapping` of a
///   `repr(C)` struct with a compile-time no-padding assertion on every
///   target. Cheapest on fixed-size shapes, but the struct must contain
///   only fixed-size, non-fat-pointer fields.
pub trait Event: Discriminator {
    /// Serialize the event: discriminator bytes followed by event data.
    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    fn data(&self) -> InstructionDataBuffer;

    /// Serialize the event into a caller-provided bounded buffer.
    ///
    /// Generated events override this to avoid creating the full instruction
    /// data buffer on the SBF stack when used with `emit_cpi!`.
    fn try_data_into<const N: usize>(
        &self,
        buffer: &mut crate::FixedVec<u8, N>,
    ) -> Result<(), solana_program_error::ProgramError> {
        let data = self.try_data()?;
        buffer
            .try_extend_from_slice(data.as_slice())
            .map_err(|_| solana_program_error::ProgramError::InvalidInstructionData)
    }

    /// Fallible event serialization for allocator-free runtime paths.
    /// On-chain implementations must provide this method directly. Host
    /// implementations retain the infallible `data()` compatibility method.
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    fn try_data(
        &self,
    ) -> Result<InstructionDataBuffer, solana_program_error::ProgramError>;

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    fn try_data(&self) -> Result<InstructionDataBuffer, solana_program_error::ProgramError> {
        Ok(self.data())
    }
}

/// Log event data via the `sol_log_data` syscall.
///
/// On-chain (`any(target_os = "solana", target_arch = "bpf")`), this calls the `sol_log_data` syscall
/// which emits a `Program data: <base64>` log entry that clients can subscribe to.
///
/// Off-chain (tests / non-Solana), this is a no-op.
pub fn sol_log_data(data: &[&[u8]]) {
    #[cfg(any(target_os = "solana", target_arch = "bpf"))]
    // SAFETY: data is a valid slice-of-slices; the syscall reads but does not write.
    unsafe {
        pinocchio::syscalls::sol_log_data(data as *const _ as *const u8, data.len() as u64)
    };

    #[cfg(not(any(target_os = "solana", target_arch = "bpf")))]
    core::hint::black_box(data);
}
