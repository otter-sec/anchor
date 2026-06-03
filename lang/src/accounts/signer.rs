//! Type validating that the account signed the transaction
use {
    crate::{
        error::ErrorCode,
        solana_program::{account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey},
        Accounts, AccountsExit, Key, Result, ToAccountInfos, ToAccountMetas,
    },
    std::{collections::BTreeSet, ops::Deref},
};

/// Type validating that the account signed the transaction. No other ownership
/// or type checks are done. If this is used, one should not try to access the
/// underlying account data.
///
/// Checks:
///
/// - `Signer.info.is_signer == true`
///
/// # Example
/// ```ignore
/// #[account]
/// #[derive(Default)]
/// pub struct MyData {
///     pub data: u64
/// }
///
/// #[derive(Accounts)]
/// pub struct Example<'info> {
///     #[account(init, payer = payer, space = 8 + 8)]
///     pub my_acc: Account<'info, MyData>,
///     #[account(mut)]
///     pub payer: Signer<'info>,
///     pub system_program: Program<'info, System>
/// }
/// ```
///
/// When creating an account with `init`, the `payer` needs to sign the transaction.
#[derive(Debug, Clone)]
pub struct Signer<'info> {
    info: &'info AccountInfo<'info>,
}

impl<'info> Signer<'info> {
    fn new(info: &'info AccountInfo<'info>) -> Signer<'info> {
        Self { info }
    }

    /// Deserializes the given `info` into a `Signer`.
    #[inline(never)]
    pub fn try_from(info: &'info AccountInfo<'info>) -> Result<Signer<'info>> {
        if !info.is_signer {
            return Err(ErrorCode::AccountNotSigner.into());
        }
        Ok(Signer::new(info))
    }
}

impl<'info, B> Accounts<'info, B> for Signer<'info> {
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
        Signer::try_from(account)
    }
}

impl<'info> AccountsExit<'info> for Signer<'info> {}

impl ToAccountMetas for Signer<'_> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let is_signer = is_signer.unwrap_or(self.info.is_signer);
        let meta = match self.info.is_writable {
            false => AccountMeta::new_readonly(*self.info.key, is_signer),
            true => AccountMeta::new(*self.info.key, is_signer),
        };
        vec![meta]
    }
}

impl<'info> ToAccountInfos<'info> for Signer<'info> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.info.clone()]
    }
}

impl<'info> AsRef<AccountInfo<'info>> for Signer<'info> {
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.info
    }
}

impl<'info> Deref for Signer<'info> {
    type Target = AccountInfo<'info>;

    fn deref(&self) -> &Self::Target {
        self.info
    }
}

impl Key for Signer<'_> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}
