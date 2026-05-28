//! Fixture program for `anchor debugger` symbol-resolution tests.
//!
//! Each `marker_*` function has exactly one expression body on the line
//! tagged `// MARKER: <name>` in the source below. The test scrapes those
//! tags to discover the expected DWARF line numbers without hard-coding
//! them, so moving code around in this file doesn't break the test.

#![no_std]

use pinocchio::{
    account::AccountView, address::Address, no_allocator, program_entrypoint, ProgramResult,
};
use solana_program_error::ProgramError;

#[cfg(all(not(test), target_os = "solana"))]
pinocchio::nostd_panic_handler!();

program_entrypoint!(process_instruction);
no_allocator!();

#[inline(never)]
pub fn marker_alpha(x: u64) -> u64 {
    core::hint::black_box(x.wrapping_add(0xa1a1_a1a1)) // MARKER: alpha
}

#[inline(never)]
pub fn marker_beta(x: u64) -> u64 {
    core::hint::black_box(x.wrapping_mul(0xb2b2_b2b2)) // MARKER: beta
}

#[inline(never)]
pub fn marker_gamma(x: u64) -> u64 {
    core::hint::black_box(x ^ 0xc3c3_c3c3_c3c3_c3c3) // MARKER: gamma
}

pub fn process_instruction(
    _program_id: &Address,
    _accounts: &mut [AccountView],
    instruction_data: &[u8],
) -> ProgramResult {
    let seed = instruction_data.first().copied().unwrap_or(0) as u64;
    let a = marker_alpha(seed);
    let b = marker_beta(a);
    let c = marker_gamma(b);
    if c == 0 {
        return Err(ProgramError::Custom(1));
    }
    Ok(())
}
