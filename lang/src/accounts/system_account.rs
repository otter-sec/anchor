//! Type validating that the account is owned by the system program

use {
    crate::{error::ErrorCode, solana_program::system_program, *},
    std::ops::Deref,
};

/// Type validating that the account is owned by the system program
///
/// Checks:
///
/// - `SystemAccount.info.owner == SystemProgram`
#[derive(Debug, Clone)]
pub struct SystemAccount<'info> {
    info: &'info AccountInfo<'info>,
}

impl<'info> SystemAccount<'info> {
    fn new(info: &'info AccountInfo<'info>) -> SystemAccount<'info> {
        Self { info }
    }

    #[inline(never)]
    pub fn try_from(info: &'info AccountInfo<'info>) -> Result<SystemAccount<'info>> {
        if *info.owner != system_program::ID {
            return Err(ErrorCode::AccountNotSystemOwned.into());
        }
        Ok(SystemAccount::new(info))
    }
}

impl<'info, B> Accounts<'info, B> for SystemAccount<'info> {
    #[inline(never)]
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
        SystemAccount::try_from(account)
    }
}

impl<'info> AccountsExit<'info> for SystemAccount<'info> {}

impl ToAccountMetas for SystemAccount<'_> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let is_signer = is_signer.unwrap_or(self.info.is_signer);
        let meta = match self.info.is_writable {
            false => AccountMeta::new_readonly(*self.info.key, is_signer),
            true => AccountMeta::new(*self.info.key, is_signer),
        };
        vec![meta]
    }
}

impl<'info> ToAccountInfos<'info> for SystemAccount<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.info.clone()]
    }
}

impl<'info> AsRef<AccountInfo<'info>> for SystemAccount<'info> {
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.info
    }
}

impl<'info> Deref for SystemAccount<'info> {
    type Target = AccountInfo<'info>;

    fn deref(&self) -> &Self::Target {
        self.info
    }
}

impl Key for SystemAccount<'_> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}
