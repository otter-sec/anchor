//! `Box<T>` type to save stack space.
//!
//! Sometimes accounts are too large for the stack,
//! leading to stack violations.
//!
//! Boxing the account can help.
//!
//! # Example
//! ```ignore
//! #[derive(Accounts)]
//! pub struct Example<'info> {
//!     pub my_acc: Box<Account<'info, MyData>>
//! }
//! ```

use {
    crate::{
        solana_program::{account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey},
        Accounts, AccountsClose, AccountsExit, Result, ToAccountInfos, ToAccountMetas,
    },
    std::{collections::BTreeSet, ops::Deref},
};

impl<'info, B, T: Accounts<'info, B>> Accounts<'info, B> for Box<T> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut B,
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        let layout = std::alloc::Layout::new::<T>();
        let raw_ptr = unsafe { std::alloc::alloc(layout) as *mut T };
        if raw_ptr.is_null() {
            return Err(crate::error::ErrorCode::AccountDidNotDeserialize.into());
        }
        struct AllocGuard<T>(*mut T);
        impl<T> Drop for AllocGuard<T> {
            fn drop(&mut self) {
                if !self.0.is_null() {
                    unsafe {
                        std::alloc::dealloc(self.0 as *mut u8, std::alloc::Layout::new::<T>());
                    }
                }
            }
        }
        let mut guard = AllocGuard(raw_ptr);
        unsafe {
            let val = T::try_accounts(program_id, accounts, ix_data, bumps, reallocs)?;
            std::ptr::write(guard.0, val);
        }
        let ptr = guard.0;
        guard.0 = std::ptr::null_mut();
        Ok(unsafe { Box::from_raw(ptr) })
    }
}

impl<'info, T: AccountsExit<'info>> AccountsExit<'info> for Box<T> {
    fn exit(&self, program_id: &Pubkey) -> Result<()> {
        T::exit(Deref::deref(self), program_id)
    }
}

impl<'info, T: ToAccountInfos<'info>> ToAccountInfos<'info> for Box<T> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        T::to_account_infos(self)
    }
}

impl<T: ToAccountMetas> ToAccountMetas for Box<T> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        T::to_account_metas(self, is_signer)
    }
}

impl<'info, T: AccountsClose<'info>> AccountsClose<'info> for Box<T> {
    fn close(&self, sol_destination: AccountInfo<'info>) -> Result<()> {
        T::close(self, sol_destination)
    }
}
