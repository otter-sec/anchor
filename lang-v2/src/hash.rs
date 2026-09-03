/// SHA-256 hash of `data`.
///
/// On-chain: `sol_sha256` syscall. Off-chain: `sha2` crate.
#[inline]
pub fn sha256(data: &[u8]) -> [u8; 32] {
    #[cfg(target_os = "solana")]
    {
        use solana_sha256_hasher::sol_sha256;
        let slices: [&[u8]; 1] = [data];
        let mut out = core::mem::MaybeUninit::<[u8; 32]>::uninit();
        // SAFETY: slices is a valid single-element array; sol_sha256 writes
        // exactly 32 bytes into out, fully initializing it.
        unsafe {
            sol_sha256(slices.as_ptr() as *const u8, 1, out.as_mut_ptr() as *mut u8);
            out.assume_init()
        }
    }
    #[cfg(not(target_os = "solana"))]
    {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(data);
        hasher.finalize().into()
    }
}
