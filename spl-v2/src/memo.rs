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

use {
    alloc::vec::Vec,
    pinocchio::{
        account::AccountView,
        cpi::invoke_signed_with_slice,
        instruction::{InstructionAccount, InstructionView},
    },
    solana_program_error::ProgramResult,
};

pub use pinocchio_memo::ID;

/// Attach `memo` to the transaction, signed by `signers`.
///
/// `signers` are forwarded as read-only signers, so each one must be a
/// transaction signer.
///
/// # Example
///
/// ```ignore
/// let signers = ctx.remaining_accounts();
/// anchor_spl::memo::build_memo(&signers, "hello")?;
/// ```
#[inline(always)]
pub fn build_memo(signers: &[AccountView], memo: &str) -> ProgramResult {
    let accounts: Vec<InstructionAccount> = signers
        .iter()
        .map(|signer| InstructionAccount::readonly_signer(signer.address()))
        .collect();
    invoke_signed_with_slice(
        &InstructionView {
            program_id: &ID,
            accounts: &accounts,
            data: memo.as_bytes(),
        },
        signers,
        &[],
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
