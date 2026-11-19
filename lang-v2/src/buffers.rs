use crate::{CpiHandle, FixedVec};

pub type CpiHandleBuffer<'a, const N: usize> = FixedVec<CpiHandle<'a>, N>;

// Agave's sol_invoke_signed syscall checks this independently of the account count.
const MAX_CPI_SIGNERS: usize = 16;

pub(crate) type SignerBuffer<'a> = FixedVec<pinocchio::cpi::Signer<'a, 'a>, MAX_CPI_SIGNERS>;

pub(crate) type CpiAccountBuffer<'a, const N: usize> = FixedVec<
    core::mem::MaybeUninit<pinocchio::cpi::CpiAccount<'a>>,
    N,
>;
