//! Explicit wrapper for AccountInfo types to emphasize
//! that no checks are performed

use {
    crate::{
        error::ErrorCode,
        solana_program::{account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey},
        Accounts, AccountsExit, Key, Result, ToAccountInfos, ToAccountMetas,
    },
    std::{collections::BTreeSet, ops::Deref},
};

/// Explicit wrapper for AccountInfo types to emphasize
/// that no checks are performed
#[derive(Debug, Clone)]
pub struct UncheckedAccount<'info>(&'info AccountInfo<'info>);

impl<'info> From<&'info AccountInfo<'info>> for UncheckedAccount<'info> {
    fn from(value: &'info AccountInfo<'info>) -> Self {
        Self(value)
    }
}

impl<'info> UncheckedAccount<'info> {
    #[deprecated(note = "Use `UncheckedAccount::from` instead; this constructor cannot fail")]
    pub fn try_from(acc_info: &'info AccountInfo<'info>) -> Self {
        Self(acc_info)
    }
}

impl<'info, B> Accounts<'info, B> for UncheckedAccount<'info> {
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
        Ok(UncheckedAccount(account))
    }
}

impl ToAccountMetas for UncheckedAccount<'_> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let is_signer = is_signer.unwrap_or(self.is_signer);
        let meta = match self.is_writable {
            false => AccountMeta::new_readonly(*self.key, is_signer),
            true => AccountMeta::new(*self.key, is_signer),
        };
        vec![meta]
    }
}

impl<'info> ToAccountInfos<'info> for UncheckedAccount<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.0.clone()]
    }
}

impl<'info> AccountsExit<'info> for UncheckedAccount<'info> {}

impl<'info> AsRef<AccountInfo<'info>> for UncheckedAccount<'info> {
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.0
    }
}

impl<'info> Deref for UncheckedAccount<'info> {
    type Target = AccountInfo<'info>;

    fn deref(&self) -> &Self::Target {
        self.0
    }
}

impl Key for UncheckedAccount<'_> {
    fn key(&self) -> Pubkey {
        *self.0.key
    }
}
