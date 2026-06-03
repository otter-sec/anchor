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
        T::try_accounts(program_id, accounts, ix_data, bumps, reallocs).map(Box::new)
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
