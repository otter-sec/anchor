//! `Option<T>` type for optional accounts.
//!
//! # Example
//! ```ignore
//! #[derive(Accounts)]
//! pub struct Example<'info> {
//!     pub my_acc: Option<Account<'info, MyData>>
//! }
//! ```

use {
    crate::{
        error::ErrorCode,
        solana_program::{account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey},
        Accounts, AccountsClose, AccountsExit, Result, ToAccountInfos, ToAccountMetas,
    },
    std::collections::BTreeSet,
};

impl<'info, B, T: Accounts<'info, B>> Accounts<'info, B> for Option<T> {
    fn try_accounts(
        program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        ix_data: &[u8],
        bumps: &mut B,
        reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        if accounts.is_empty() {
            return if cfg!(feature = "allow-missing-optionals") {
                // We don't care if accounts is empty (when this feature is active),
                // so if that's the case we return None. This allows adding optional
                // accounts at the end of the Accounts struct without causing a breaking
                // change. This is safe and will error out if a required account is then
                // added after the optional account and the accounts aren't passed in.
                Ok(None)
            } else {
                // If the feature is inactive (it is off by default), then we error out
                // like every other Account.
                Err(ErrorCode::AccountNotEnoughKeys.into())
            };
        }

        // If there are enough accounts, it will check the program_id and return
        // None if it matches, popping the first account off the accounts vec.
        if accounts[0].key == program_id {
            *accounts = &accounts[1..];
            Ok(None)
        } else {
            // If the program_id doesn't equal the account key, we default to
            // the try_accounts implementation for the inner type and then wrap that with
            // Some. This should handle all possible valid cases.
            T::try_accounts(program_id, accounts, ix_data, bumps, reallocs).map(Some)
        }
    }
}

impl<'info, T: ToAccountInfos<'info>> ToAccountInfos<'info> for Option<T> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        self.as_ref()
            .map_or_else(Vec::new, |account| account.to_account_infos())
    }
}

impl<T: ToAccountMetas> ToAccountMetas for Option<T> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        self.as_ref()
            .expect("Cannot run `to_account_metas` on None")
            .to_account_metas(is_signer)
    }
}

impl<'info, T: AccountsClose<'info>> AccountsClose<'info> for Option<T> {
    fn close(&self, sol_destination: AccountInfo<'info>) -> Result<()> {
        self.as_ref()
            .map_or(Ok(()), |t| T::close(t, sol_destination))
    }
}

impl<'info, T: AccountsExit<'info>> AccountsExit<'info> for Option<T> {
    fn exit(&self, program_id: &Pubkey) -> Result<()> {
        self.as_ref().map_or(Ok(()), |t| t.exit(program_id))
    }
}
