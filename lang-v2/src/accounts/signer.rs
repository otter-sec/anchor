use {
    crate::{accounts::view_wrapper_traits, AnchorAccount},
    pinocchio::{account::AccountView, address::Address},
    solana_program_error::ProgramError,
};

pub struct Signer {
    view: AccountView,
}

impl Signer {
    /// Returns the account's address.
    #[inline(always)]
    pub fn address(&self) -> &Address {
        self.view.address()
    }
}

impl AnchorAccount for Signer {
    type Data = AccountView;
    const IS_SIGNER: bool = true;

    #[inline(always)]
    fn load(view: AccountView, _program_id: &Address) -> Result<Self, ProgramError> {
        if !view.is_signer() {
            return Err(ProgramError::MissingRequiredSignature);
        }
        Ok(Self { view })
    }

    /// Fused `is_signer` + `is_writable` check via a single 2-byte load.
    ///
    /// # Safety
    ///
    /// See [`AnchorAccount::load_mut`]. Additionally, reads bytes at offsets
    /// `+1` (is_signer) and `+2` (is_writable) from the serialized account
    /// header as a single LE u16. This layout is defined by agave's
    /// `serialize_parameters_aligned` (see
    /// `agave/program-runtime/src/serialization.rs`) and is part of the
    /// stable SBF ABI: `[NON_DUP_MARKER, is_signer, is_writable, key...]`.
    ///
    /// Returns `ConstraintSigner` if either flag is unset.
    #[inline(always)]
    unsafe fn load_mut(view: AccountView, _program_id: &Address) -> Result<Self, ProgramError> {
        // SAFETY: view.account_ptr() points at a valid RuntimeAccount header.
        // Byte `+1` and `+2` are always in bounds within the 88-byte header.
        // The read is byte-aligned (offset 1 into a u16) but SBF allows
        // unaligned access.
        let flags_ptr = unsafe { (view.account_ptr() as *const u8).add(1) as *const u16 };
        let flags = unsafe { core::ptr::read_unaligned(flags_ptr) };
        // Little-endian: low byte = is_signer, high byte = is_writable.
        // Both must be 1 → u16 value 0x0101.
        if flags != 0x0101 {
            return Err(crate::ErrorCode::ConstraintSigner.into());
        }
        Ok(Self { view })
    }

    #[inline(always)]
    fn account(&self) -> &AccountView {
        &self.view
    }
}

view_wrapper_traits!(Signer);

#[doc(hidden)]
impl crate::IdlAccountType for Signer {
    const __IDL_IS_SIGNER: bool = true;
}
