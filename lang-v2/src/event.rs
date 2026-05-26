use {crate::Discriminator, alloc::vec::Vec};

// Sha256(anchor:event)[..8], matching Anchor v1's self-CPI event instruction tag.
pub const EVENT_IX_TAG: u64 = 0x1d9acb512ea545e4;
pub const EVENT_IX_TAG_LE: &[u8] = &EVENT_IX_TAG.to_le_bytes();

/// Trait for event structs. Implemented by the `#[event]` attribute macro.
///
/// Two serialization modes are emitted by the macro, both exposed via the
/// single `data()` entry point:
/// - default (`#[event]`) — wincode with a borsh-compatible wire format
///   (`BORSH_CONFIG`: u8 enum tags + fixed `u32` LE length prefixes), so
///   off-chain consumers decoding as borsh see the same bytes. Supports
///   `Vec`/`String`/`Option`/enums and is materially cheaper than borsh on
///   SBF (3–10× fewer CUs).
/// - opt-in (`#[event(bytemuck)]`) — zero-copy `copy_nonoverlapping` of a
///   `repr(C)` struct with a compile-time padding assertion. Cheapest on
///   fixed-size shapes, but the struct must contain only fixed-size,
///   non-fat-pointer fields.
pub trait Event: Discriminator {
    /// Serialize the event: discriminator bytes followed by event data.
    fn data(&self) -> Vec<u8>;
}

/// Log event data via the `sol_log_data` syscall.
///
/// On-chain (`target_os = "solana"`), this calls the `sol_log_data` syscall
/// which emits a `Program data: <base64>` log entry that clients can subscribe to.
///
/// Off-chain (tests / non-Solana), this is a no-op.
pub fn sol_log_data(data: &[&[u8]]) {
    #[cfg(target_os = "solana")]
    // SAFETY: data is a valid slice-of-slices; the syscall reads but does not write.
    unsafe {
        pinocchio::syscalls::sol_log_data(data as *const _ as *const u8, data.len() as u64)
    };

    #[cfg(not(target_os = "solana"))]
    core::hint::black_box(data);
}
