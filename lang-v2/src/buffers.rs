use crate::{CpiHandle, FixedVec};

pub type CpiHandleBuffer<'a, const N: usize> = FixedVec<CpiHandle<'a>, N>;

pub(crate) type SignerBuffer<'a, const N: usize> =
    FixedVec<pinocchio::cpi::Signer<'a, 'a>, N>;

pub(crate) type CpiAccountBuffer<'a, const N: usize> = FixedVec<
    core::mem::MaybeUninit<pinocchio::cpi::CpiAccount<'a>>,
    N,
>;
