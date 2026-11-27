//! SPL Memo CPI helpers.
//!
//! Memo v3 owns no accounts: the instruction data is the memo bytes and the
//! account list is the signer set. Unlike v1, the payload is `&str` rather
//! than `&[u8]`, matching `pinocchio-memo`.
//!
//! `pinocchio_memo::instructions::Memo::invoke` is deliberately not used:
//! both it and the `invoke_signed_with_bounds` it calls are
//! `#[inline(always)]`, and each allocates a 64 entry CPI buffer on the
//! stack. Inlined together they need a 4688 byte frame, which overflows the
//! 4096 byte SBF stack frame and traps at runtime. The slice variant used
//! here heap allocates its CPI buffer instead.

pub use pinocchio_memo::ID;
use {
    alloc::vec::Vec,
    pinocchio::{
        account::AccountView,
        cpi::{invoke_signed_with_slice, Seed, Signer},
        instruction::{InstructionAccount, InstructionView},
    },
    solana_program_error::ProgramResult,
};

/// Attach `memo` to the transaction, signed by `signers`.
///
/// `signers` are forwarded as read-only signers. Each one must be either a
/// transaction signer or a PDA of the calling program covered by
/// `signer_seeds`.
///
/// # Example
///
/// ```ignore
/// let signers = ctx.remaining_accounts();
/// anchor_spl::memo::build_memo(&signers, &[], "hello")?;
/// ```
///
/// A PDA of the calling program can sign through `signer_seeds`:
///
/// ```ignore
/// let seeds: &[&[u8]] = &[b"memo", &[bump]];
/// anchor_spl::memo::build_memo(&[pda], &[seeds], "hello")?;
/// ```
#[inline(always)]
pub fn build_memo(signers: &[AccountView], signer_seeds: &[&[&[u8]]], memo: &str) -> ProgramResult {
    let accounts: Vec<InstructionAccount> = signers
        .iter()
        .map(|signer| InstructionAccount::readonly_signer(signer.address()))
        .collect();

    // `invoke_signed_with_slice` takes `&[Signer]`, while callers pass the
    // same nested seed slices as `CpiContext`. `Seed` and `Signer` are
    // repr(C) and layout-compatible with `&[u8]`, so this mirrors the
    // conversion in `lang-v2/src/context_cpi.rs`.
    let signers_seeds: Vec<Signer> = signer_seeds
        .iter()
        .map(|pda_seeds| {
            // SAFETY: pinocchio::cpi::Seed is repr(C) { *const u8, u64,
            // PhantomData }, which has the same layout as &[u8] on SBF. This
            // is verified by the static assertion in lang-v2/src/cpi.rs.
            let cpi_seeds: &[Seed] = unsafe {
                core::slice::from_raw_parts(pda_seeds.as_ptr() as *const Seed, pda_seeds.len())
            };
            Signer::from(cpi_seeds)
        })
        .collect();

    invoke_signed_with_slice(
        &InstructionView {
            program_id: &ID,
            accounts: &accounts,
            data: memo.as_bytes(),
        },
        signers,
        &signers_seeds,
    )
}

#[cfg(test)]
mod tests {
    use {
        super::ID,
        anchor_lang::{programs, Id},
    };

    #[test]
    fn memo_program_id_matches_marker_type() {
        assert_eq!(ID, programs::Memo::id());
    }
}
