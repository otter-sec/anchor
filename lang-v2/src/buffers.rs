use crate::{CpiHandle, FixedVec};

/// Solana's maximum serialized instruction-data budget used by the current
/// runtime compatibility paths. Operation-specific stack sizing remains a
/// follow-up concern; this is not an account-count limit.
#[doc(hidden)]
pub const MAX_INSTRUCTION_DATA: usize = 1232;

pub type CpiHandleBuffer<'a, const N: usize> = FixedVec<CpiHandle<'a>, N>;

#[doc(hidden)]
pub type InstructionDataBuffer = FixedVec<u8, MAX_INSTRUCTION_DATA>;

#[doc(hidden)]
pub type SignerBuffer<'a, const N: usize> =
    FixedVec<pinocchio::cpi::Signer<'a, 'a>, N>;

#[doc(hidden)]
pub type CpiAccountBuffer<'a, const N: usize> = FixedVec<
    core::mem::MaybeUninit<pinocchio::cpi::CpiAccount<'a>>,
    N,
>;
