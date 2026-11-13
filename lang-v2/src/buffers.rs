use crate::{CpiHandle, FixedVec};

pub type CpiHandleBuffer<'a, const N: usize> = FixedVec<CpiHandle<'a>, N>;
