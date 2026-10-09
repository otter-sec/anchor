//! Spy registered at the Memo program id.
//!
//! Memo v3 has no accounts, no arguments, and no discriminator: the instruction
//! data *is* the memo bytes, and the account list is the signer set. So this
//! spy uses a raw `pinocchio` entrypoint rather than `#[program]`, whose
//! discriminator dispatch cannot decode an opaque payload.
//!
//! It mirrors the properties `anchor_spl::memo` is responsible for:
//!
//! - the callee is invoked with zero accounts of its own
//! - every account arrives marked as a signer
//! - the raw memo bytes reach the callee unmodified
//!
//! The observed signer count and memo bytes are returned as instruction return
//! data so the test can assert on them, rather than only on success.
//!
//! Real Memo v3 behaviour beyond that is owned by `spl-memo-interface`.

use {
    pinocchio::{cpi::set_return_data, entrypoint, AccountView, Address, ProgramResult},
    solana_msg::msg,
    solana_program_error::ProgramError,
};

entrypoint!(process_instruction);

/// Fails if any account was not marked a signer, then returns the signer count
/// and memo bytes so the caller can assert both were forwarded.
pub fn process_instruction(
    _program_id: &Address,
    accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    for account in accounts.iter() {
        if !account.is_signer() {
            msg!("memo signer missing");
            return Err(ProgramError::MissingRequiredSignature);
        }
    }

    msg!("memo:{}", String::from_utf8_lossy(instruction_data));

    // `<signer count as u32 LE><memo bytes>`
    let signer_count = u32::try_from(accounts.len()).unwrap_or(u32::MAX);
    let mut out = signer_count.to_le_bytes().to_vec();
    out.extend_from_slice(instruction_data);
    set_return_data(&out);
    Ok(())
}
