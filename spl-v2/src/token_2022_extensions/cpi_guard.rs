use {
    crate::token_2022::spl_token_2022::error::TokenError,
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
    // Always invoked via CPI from a caller program, so Token-2022 would reject
    // the toggle. Return the matching error instead of panicking through a
    // `Result` API.
    Err(TokenError::CpiGuardSettingsLocked.into())
}

#[deprecated(
    note = "Token-2022 rejects CPI-initiated toggling of CPI Guard with CpiGuardSettingsLocked."
)]
pub fn cpi_guard_disable<'a>(_ctx: CpiContext<'a, CpiGuard<'a>>) -> Result<(), ProgramError> {
    Err(TokenError::CpiGuardSettingsLocked.into())
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

    fn sample_ctx<'a>(
        program: &'a Address,
        account_view: &'a mut pinocchio::account::AccountView,
        owner_view: &'a pinocchio::account::AccountView,
    ) -> CpiContext<'a, CpiGuard<'a>> {
        let accounts = CpiGuard {
            account: CpiHandleMut::writable(account_view),
            owner: CpiHandle::readonly(owner_view),
        };
        CpiContext::new(program, accounts)
    }

    #[test]
    #[allow(deprecated)]
    fn cpi_guard_enable_returns_settings_locked() {
        let program = Address::new_from_array([7; 32]);
        let account_buffer = account([1; 32], false, true);
        let owner_buffer = account([2; 32], true, false);
        let mut account_view = unsafe { account_buffer.view() };
        let owner_view = unsafe { owner_buffer.view() };

        let err = cpi_guard_enable(sample_ctx(&program, &mut account_view, &owner_view)).unwrap_err();
        assert_eq!(err, TokenError::CpiGuardSettingsLocked.into());
    }

    #[test]
    #[allow(deprecated)]
    fn cpi_guard_disable_returns_settings_locked() {
        let program = Address::new_from_array([7; 32]);
        let account_buffer = account([1; 32], false, true);
        let owner_buffer = account([2; 32], true, false);
        let mut account_view = unsafe { account_buffer.view() };
        let owner_view = unsafe { owner_buffer.view() };

        let err =
            cpi_guard_disable(sample_ctx(&program, &mut account_view, &owner_view)).unwrap_err();
        assert_eq!(err, TokenError::CpiGuardSettingsLocked.into());
    }
}
