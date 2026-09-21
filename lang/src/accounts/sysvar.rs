//! Type validating that the account is a sysvar and deserializing it

use {
    crate::{
        error::ErrorCode,
        solana_program::{account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey},
        Accounts, AccountsExit, Key, Result, ToAccountInfos, ToAccountMetas,
    },
    solana_sysvar::{Sysvar as SolanaSysvar, SysvarSerialize as SolanaSysvarSerialize},
    std::{
        collections::BTreeSet,
        fmt,
        ops::{Deref, DerefMut},
    },
};

/// Type validating that the account is a sysvar and deserializing it.
///
/// If possible, sysvars should not be used via accounts
/// but by using the [`get`](https://docs.rs/solana-program/latest/solana_program/sysvar/trait.Sysvar.html#method.get)
/// function on the desired sysvar. This is because using `get`
/// does not run the risk of Anchor having a bug in its `Sysvar` type
/// and using `get` also decreases tx size, making space for other
/// accounts that cannot be requested via syscall.
///
/// # Example
/// ```ignore
/// // OK - via account in the account validation struct
/// #[derive(Accounts)]
/// pub struct Example<'info> {
///     pub clock: Sysvar<'info, Clock>
/// }
/// // BETTER - via syscall in the instruction function
/// fn better(ctx: Context<Better>) -> Result<()> {
///     let clock = Clock::get()?;
/// }
/// ```
pub struct Sysvar<'info, T: SolanaSysvar> {
    info: &'info AccountInfo<'info>,
    account: T,
}

impl<T: SolanaSysvarSerialize + fmt::Debug> fmt::Debug for Sysvar<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sysvar")
            .field("info", &self.info)
            .field("account", &self.account)
            .finish()
    }
}

impl<'info, T: SolanaSysvarSerialize> Sysvar<'info, T> {
    pub fn from_account_info(acc_info: &'info AccountInfo<'info>) -> Result<Sysvar<'info, T>> {
        match T::from_account_info(acc_info) {
            Ok(val) => Ok(Sysvar {
                info: acc_info,
                account: val,
            }),
            Err(_) => Err(ErrorCode::AccountSysvarMismatch.into()),
        }
    }
}

impl<T: SolanaSysvarSerialize> Clone for Sysvar<'_, T> {
    fn clone(&self) -> Self {
        Self {
            info: self.info,
            account: T::from_account_info(self.info).unwrap(),
        }
    }
}

impl<'info, B, T: SolanaSysvarSerialize> Accounts<'info, B> for Sysvar<'info, T> {
    fn try_accounts(
        _program_id: &Pubkey,
        accounts: &mut &'info [AccountInfo<'info>],
        _ix_data: &[u8],
        _bumps: &mut B,
        _reallocs: &mut BTreeSet<Pubkey>,
    ) -> Result<Self> {
        if accounts.is_empty() {
            return Err(ErrorCode::AccountNotEnoughKeys.into());
        }
        let account = &accounts[0];
        *accounts = &accounts[1..];
        Sysvar::from_account_info(account)
    }
}

impl<T: SolanaSysvarSerialize> ToAccountMetas for Sysvar<'_, T> {
    fn to_account_metas(&self, _is_signer: Option<bool>) -> Vec<AccountMeta> {
        vec![AccountMeta::new_readonly(*self.info.key, false)]
    }
}

impl<'info, T: SolanaSysvarSerialize> ToAccountInfos<'info> for Sysvar<'info, T> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.info.clone()]
    }
}

impl<'info, T: SolanaSysvarSerialize> AsRef<AccountInfo<'info>> for Sysvar<'info, T> {
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.info
    }
}

impl<T: SolanaSysvarSerialize> Deref for Sysvar<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.account
    }
}

impl<T: SolanaSysvarSerialize> DerefMut for Sysvar<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.account
    }
}

impl<'info, T: SolanaSysvarSerialize> AccountsExit<'info> for Sysvar<'info, T> {}

impl<T: SolanaSysvarSerialize> Key for Sysvar<'_, T> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}
