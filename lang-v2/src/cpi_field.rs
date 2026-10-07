//! Compositional CPI fields. Derives dispatch through these traits after Rust
//! resolves aliases, imports and associated types.

use {
    crate::{CpiHandle, CpiHandleMut},
    alloc::vec::Vec,
    pinocchio::{address::Address, instruction::InstructionAccount},
};

/// Appends one field's contribution to a CPI account list.
#[doc(hidden)]
pub trait CpiField<'a> {
    fn append_instruction_accounts(
        &self,
        program_id: &'a Address,
        signer: bool,
        out: &mut Vec<InstructionAccount<'a>>,
    );
    fn append_cpi_handles(&self, out: &mut Vec<CpiHandle<'a>>);
    fn append_sentinel_flags(&self, out: &mut Vec<bool>);
}

/// A field contributing exactly one account meta. Only these fields may be
/// optional: absence contributes one program-id sentinel and no handle.
#[doc(hidden)]
pub trait SingleCpiField<'a>: CpiField<'a> {}

/// A direct handle that may also contribute a duplicate readonly account.
#[doc(hidden)]
pub trait CpiReadonlyField<'a>: SingleCpiField<'a> {
    fn readonly_handle(&self) -> CpiHandle<'a>;
}

macro_rules! impl_handle {
    ($ty:ident, $writable:literal, $handle:expr) => {
        impl<'a> CpiField<'a> for $ty<'a> {
            #[inline(always)]
            fn append_instruction_accounts(
                &self,
                _: &'a Address,
                signer: bool,
                out: &mut Vec<InstructionAccount<'a>>,
            ) {
                out.push(InstructionAccount::new(self.address(), $writable, signer));
            }
            #[inline(always)]
            fn append_cpi_handles(&self, out: &mut Vec<CpiHandle<'a>>) {
                out.push(($handle)(*self));
            }
            #[inline(always)]
            fn append_sentinel_flags(&self, out: &mut Vec<bool>) {
                out.push(false);
            }
        }
        impl<'a> SingleCpiField<'a> for $ty<'a> {}
        impl<'a> CpiReadonlyField<'a> for $ty<'a> {
            #[inline(always)]
            fn readonly_handle(&self) -> CpiHandle<'a> {
                self.into_readonly()
            }
        }
    };
}

impl_handle!(CpiHandle, false, CpiHandle::into_readonly);
impl_handle!(CpiHandleMut, true, CpiHandle::from);

impl<'a, T: SingleCpiField<'a>> CpiField<'a> for Option<T> {
    #[inline(always)]
    fn append_instruction_accounts(
        &self,
        program_id: &'a Address,
        signer: bool,
        out: &mut Vec<InstructionAccount<'a>>,
    ) {
        match self {
            Some(field) => field.append_instruction_accounts(program_id, signer, out),
            None => out.push(InstructionAccount::readonly(program_id)),
        }
    }
    #[inline(always)]
    fn append_cpi_handles(&self, out: &mut Vec<CpiHandle<'a>>) {
        if let Some(field) = self {
            field.append_cpi_handles(out);
        }
    }
    #[inline(always)]
    fn append_sentinel_flags(&self, out: &mut Vec<bool>) {
        out.push(self.is_none());
    }
}

impl<'a, T> CpiField<'a> for core::marker::PhantomData<T> {
    fn append_instruction_accounts(
        &self,
        _: &'a Address,
        _: bool,
        _: &mut Vec<InstructionAccount<'a>>,
    ) {
    }
    fn append_cpi_handles(&self, _: &mut Vec<CpiHandle<'a>>) {}
    fn append_sentinel_flags(&self, _: &mut Vec<bool>) {}
}
