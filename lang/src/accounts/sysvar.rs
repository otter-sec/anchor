//! Type validating that the account is a sysvar and deserializing it

use {
    crate::{
        error::ErrorCode,
        solana_program::{account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey},
        Accounts, AccountsExit, Key, Result, ToAccountInfos, ToAccountMetas,
    },
    solana_sysvar::Sysvar as SolanaSysvar,
    solana_sysvar_id::SysvarId,
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

impl<T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned + fmt::Debug> fmt::Debug
    for Sysvar<'_, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sysvar")
            .field("info", &self.info)
            .field("account", &self.account)
            .finish()
    }
}

/// Deserializes a sysvar from its account data.
///
/// This performs the same steps as the deprecated `solana_sysvar::SysvarSerialize`:
/// it checks the account address against the sysvar's ID and then decodes the
/// account data with bincode, so the accepted wire format is unchanged.
fn deserialize_sysvar<T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned>(
    acc_info: &AccountInfo,
) -> Result<T> {
    if !T::check_id(acc_info.key) {
        return Err(ErrorCode::AccountSysvarMismatch.into());
    }
    bincode::deserialize(&acc_info.data.borrow())
        .map_err(|_| ErrorCode::AccountSysvarMismatch.into())
}

impl<'info, T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned> Sysvar<'info, T> {
    pub fn from_account_info(acc_info: &'info AccountInfo<'info>) -> Result<Sysvar<'info, T>> {
        Ok(Sysvar {
            info: acc_info,
            account: deserialize_sysvar(acc_info)?,
        })
    }
}

impl<T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned> Clone for Sysvar<'_, T> {
    fn clone(&self) -> Self {
        Self {
            info: self.info,
            account: deserialize_sysvar(self.info).unwrap(),
        }
    }
}

impl<'info, B, T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned> Accounts<'info, B>
    for Sysvar<'info, T>
{
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

impl<T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned> ToAccountMetas for Sysvar<'_, T> {
    fn to_account_metas(&self, _is_signer: Option<bool>) -> Vec<AccountMeta> {
        vec![AccountMeta::new_readonly(*self.info.key, false)]
    }
}

impl<'info, T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned> ToAccountInfos<'info>
    for Sysvar<'info, T>
{
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.info.clone()]
    }
}

impl<'info, T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned> AsRef<AccountInfo<'info>>
    for Sysvar<'info, T>
{
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.info
    }
}

impl<T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned> Deref for Sysvar<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.account
    }
}

impl<T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned> DerefMut for Sysvar<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.account
    }
}

impl<'info, T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned> AccountsExit<'info>
    for Sysvar<'info, T>
{
}

impl<T: SolanaSysvar + SysvarId + serde::de::DeserializeOwned> Key for Sysvar<'_, T> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}
