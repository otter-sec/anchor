//! Type validating that the account is a sysvar and deserializing it

use {
    crate::{
        compat::solana_sysvar::Sysvar as SolanaSysvar,
        error::ErrorCode,
        solana_program::{account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey},
        Accounts, AccountsExit, Key, Result, ToAccountInfos, ToAccountMetas,
    },
    std::{
        collections::BTreeSet,
        fmt,
        ops::{Deref, DerefMut},
    },
};

/// The per-cohort capability needed to decode a sysvar from its account data.
///
/// Blanket-implemented, so callers never name it — it exists only so the impls
/// below can state one bound that means different things under `solana-v3` and `solana-v4`.
/// `solana-v3` decodes via `solana_sysvar::SysvarSerialize`; `solana-v4` removed that trait,
/// so the equivalent is assembled from `SysvarId` plus a serde/bincode decode.
#[cfg(feature = "solana-v3")]
pub trait DecodeSysvar: solana_sysvar_v3::SysvarSerialize {}
#[cfg(feature = "solana-v3")]
impl<T: solana_sysvar_v3::SysvarSerialize> DecodeSysvar for T {}

#[cfg(feature = "solana-v4")]
pub trait DecodeSysvar: solana_sysvar_id::SysvarId + serde::de::DeserializeOwned {}
#[cfg(feature = "solana-v4")]
impl<T: solana_sysvar_id::SysvarId + serde::de::DeserializeOwned> DecodeSysvar for T {}

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

impl<T: SolanaSysvar + fmt::Debug> fmt::Debug for Sysvar<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Sysvar")
            .field("info", &self.info)
            .field("account", &self.account)
            .finish()
    }
}

/// Deserializes a sysvar from its account data.
#[cfg(feature = "solana-v3")]
fn deserialize_sysvar<T: DecodeSysvar>(acc_info: &AccountInfo) -> Result<T> {
    T::from_account_info(acc_info).map_err(|_| ErrorCode::AccountSysvarMismatch.into())
}

/// Deserializes a sysvar from its account data.
#[cfg(feature = "solana-v4")]
fn deserialize_sysvar<T: DecodeSysvar>(acc_info: &AccountInfo) -> Result<T> {
    if !T::check_id(acc_info.key) {
        return Err(ErrorCode::AccountSysvarMismatch.into());
    }
    bincode::deserialize(&acc_info.data.borrow())
        .map_err(|_| ErrorCode::AccountSysvarMismatch.into())
}

impl<'info, T: SolanaSysvar + DecodeSysvar> Sysvar<'info, T> {
    pub fn from_account_info(acc_info: &'info AccountInfo<'info>) -> Result<Sysvar<'info, T>> {
        Ok(Sysvar {
            info: acc_info,
            account: deserialize_sysvar(acc_info)?,
        })
    }
}

impl<T: SolanaSysvar + DecodeSysvar> Clone for Sysvar<'_, T> {
    fn clone(&self) -> Self {
        Self {
            info: self.info,
            account: deserialize_sysvar(self.info).unwrap(),
        }
    }
}

impl<'info, B, T: SolanaSysvar + DecodeSysvar> Accounts<'info, B> for Sysvar<'info, T> {
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

impl<T: SolanaSysvar> ToAccountMetas for Sysvar<'_, T> {
    fn to_account_metas(&self, _is_signer: Option<bool>) -> Vec<AccountMeta> {
        vec![AccountMeta::new_readonly(*self.info.key, false)]
    }
}

impl<'info, T: SolanaSysvar> ToAccountInfos<'info> for Sysvar<'info, T> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.info.clone()]
    }
}

impl<'info, T: SolanaSysvar> AsRef<AccountInfo<'info>> for Sysvar<'info, T> {
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.info
    }
}

impl<T: SolanaSysvar> Deref for Sysvar<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &self.account
    }
}

impl<T: SolanaSysvar> DerefMut for Sysvar<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.account
    }
}

impl<'info, T: SolanaSysvar> AccountsExit<'info> for Sysvar<'info, T> {}

impl<T: SolanaSysvar> Key for Sysvar<'_, T> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}
