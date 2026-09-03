use {
    anchor_lang::{CpiContext, CpiHandle, CpiHandleMut, ToCpiAccounts},
    solana_program_error::ProgramError,
};

#[derive(ToCpiAccounts)]
pub struct CpiGuard<'a> {
    pub account: CpiHandleMut<'a>,
    #[signer]
    pub owner: CpiHandle<'a>,
}

#[deprecated(
    note = "Token-2022 rejects CPI-initiated toggling of CPI Guard with CpiGuardSettingsLocked."
)]
pub fn cpi_guard_enable<'a>(_ctx: CpiContext<'a, CpiGuard<'a>>) -> Result<(), ProgramError> {
    panic!("Token-2022 rejects CPI-initiated toggling of CPI Guard with CpiGuardSettingsLocked")
}

#[deprecated(
    note = "Token-2022 rejects CPI-initiated toggling of CPI Guard with CpiGuardSettingsLocked."
)]
pub fn cpi_guard_disable<'a>(_ctx: CpiContext<'a, CpiGuard<'a>>) -> Result<(), ProgramError> {
    panic!("Token-2022 rejects CPI-initiated toggling of CPI Guard with CpiGuardSettingsLocked")
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        anchor_lang::{
            testing::{AccountBuffer, MIN_ACCOUNT_BUF},
            Address,
        },
    };

    fn account(
        address: [u8; 32],
        signer: bool,
        writable: bool,
    ) -> AccountBuffer<{ MIN_ACCOUNT_BUF + 8 }> {
        let buffer = AccountBuffer::new();
        buffer.init(address, [9; 32], 8, signer, writable, false);
        buffer
    }

    #[test]
    #[allow(deprecated)]
    #[should_panic(
        expected = "Token-2022 rejects CPI-initiated toggling of CPI Guard with CpiGuardSettingsLocked"
    )]
    fn cpi_guard_enable_panics_at_runtime() {
        let program = Address::new_from_array([7; 32]);
        let account_buffer = account([1; 32], false, true);
        let owner_buffer = account([2; 32], true, false);
        let mut account_view = unsafe { account_buffer.view() };
        let owner_view = unsafe { owner_buffer.view() };

        let accounts = CpiGuard {
            account: CpiHandleMut::writable(&mut account_view),
            owner: CpiHandle::readonly(&owner_view),
        };
        let ctx = CpiContext::new(&program, accounts);

        let _ = cpi_guard_enable(ctx);
    }

    #[test]
    #[allow(deprecated)]
    #[should_panic(
        expected = "Token-2022 rejects CPI-initiated toggling of CPI Guard with CpiGuardSettingsLocked"
    )]
    fn cpi_guard_disable_panics_at_runtime() {
        let program = Address::new_from_array([7; 32]);
        let account_buffer = account([1; 32], false, true);
        let owner_buffer = account([2; 32], true, false);
        let mut account_view = unsafe { account_buffer.view() };
        let owner_view = unsafe { owner_buffer.view() };

        let accounts = CpiGuard {
            account: CpiHandleMut::writable(&mut account_view),
            owner: CpiHandle::readonly(&owner_view),
        };
        let ctx = CpiContext::new(&program, accounts);

        let _ = cpi_guard_disable(ctx);
    }
}
