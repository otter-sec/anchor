use crate::{AccountMeta, CpiHandle, FixedVec};

pub const MAX_INSTRUCTION_DATA: usize = 1232;
pub const MAX_EVENT_CPI_DATA: usize = 256 + 8;

pub type AccountMetaBuffer<const N: usize> = FixedVec<AccountMeta, N>;
pub type InstructionAccountBuffer<'a, const N: usize> =
    FixedVec<pinocchio::instruction::InstructionAccount<'a>, N>;
pub type AccountViewBuffer<const N: usize> = FixedVec<pinocchio::account::AccountView, N>;
pub type CpiHandleBuffer<'a, const N: usize> = FixedVec<CpiHandle<'a>, N>;
pub type BoolBuffer<const N: usize> = FixedVec<bool, N>;
pub type InstructionDataBuffer = FixedVec<u8, MAX_INSTRUCTION_DATA>;
pub type SignerBuffer<'a, const N: usize> = FixedVec<pinocchio::cpi::Signer<'a, 'a>, N>;
pub type CpiAccountBuffer<'a, const N: usize> =
    FixedVec<core::mem::MaybeUninit<pinocchio::cpi::CpiAccount<'a>>, N>;
pub type AccountBindingBuffer<const N: usize> = FixedVec<Option<usize>, N>;

/// Read-only view of optional-account sentinel flags used by CPI validation.
/// The bounded boolean representation is used by generated V2 code, while
/// slices remain accepted by the lower-level public CPI helpers.
pub trait OptionalSentinelFlags {
    fn len(&self) -> usize;
    fn get(&self, index: usize) -> bool;
}

impl OptionalSentinelFlags for [bool] {
    #[inline]
    fn len(&self) -> usize {
        <[bool]>::len(self)
    }

    #[inline]
    fn get(&self, index: usize) -> bool {
        <[bool]>::get(self, index).copied().unwrap_or(false)
    }
}

impl<const N: usize> OptionalSentinelFlags for [bool; N] {
    #[inline]
    fn len(&self) -> usize {
        N
    }

    #[inline]
    fn get(&self, index: usize) -> bool {
        self.as_slice().get(index).copied().unwrap_or(false)
    }
}

impl<const N: usize> OptionalSentinelFlags for BoolBuffer<N> {
    #[inline]
    fn len(&self) -> usize {
        FixedVec::len(self)
    }

    #[inline]
    fn get(&self, index: usize) -> bool {
        self.as_slice().get(index).copied().unwrap_or(false)
    }
}
