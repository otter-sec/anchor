//! Type validating that the account is one of a set of given Programs

use {
    crate::{
        accounts::program::Program,
        error::{Error, ErrorCode},
        solana_program::{account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey},
        AccountDeserialize, Accounts, AccountsExit, CheckId, Key, Result, ToAccountInfos,
        ToAccountMetas,
    },
    std::{collections::BTreeSet, ops::Deref},
};

/// Type validating that the account is one of a set of given Programs
///
/// The `Interface` wraps over [`Program`], allowing for
/// multiple possible program ids. Useful for any program that implements an
/// instruction interface. For example, spl-token and spl-token-2022 both implement
/// the spl-token interface.
///
/// # Table of Contents
/// - [Basic Functionality](#basic-functionality)
/// - [Out of the Box Types](#out-of-the-box-types)
///
/// # Basic Functionality
///
/// Checks:
///
/// - `expected_programs.contains(account_info.key)`
/// - `account_info.executable == true`
///
/// # Example
/// ```ignore
/// #[program]
/// mod my_program {
///     fn set_admin_settings(...){...}
/// }
///
/// #[account]
/// #[derive(Default)]
/// pub struct AdminSettings {
///     ...
/// }
///
/// #[derive(Accounts)]
/// pub struct SetAdminSettings<'info> {
///     #[account(mut, seeds = [b"admin"], bump)]
///     pub admin_settings: Account<'info, AdminSettings>,
///     #[account(constraint = program.programdata_address()? == Some(program_data.key()))]
///     pub program: Interface<'info, MyProgram>,
///     #[account(constraint = program_data.upgrade_authority_address == Some(authority.key()))]
///     pub program_data: Account<'info, ProgramData>,
///     pub authority: Signer<'info>,
/// }
/// ```
/// The given program has a function with which the upgrade authority can set admin settings.
///
/// The required constraints are as follows:
///
/// - `program` is the account of the program itself.
///   Its constraint checks that `program_data` is the account that contains the program's upgrade authority.
///   Implicitly, this checks that `program` is a BPFUpgradeable program (`program.programdata_address()?`
///   will be `None` if it's not).
/// - `program_data`'s constraint checks that its upgrade authority is the `authority` account.
/// - Finally, `authority` needs to sign the transaction.
///
/// # Out of the Box Types
///
/// Between the [`anchor_lang`](https://docs.rs/anchor-lang/latest/anchor_lang) and [`anchor_spl`](https://docs.rs/anchor_spl/latest/anchor_spl) crates,
/// the following `Interface` types are provided out of the box:
///
/// - [`TokenInterface`](https://docs.rs/anchor-spl/latest/anchor_spl/token_interface/struct.TokenInterface.html)
///
#[derive(Clone)]
pub struct Interface<'info, T>(Program<'info, T>);
impl<'a, T> Interface<'a, T> {
    pub(crate) fn new(info: &'a AccountInfo<'a>) -> Self {
        Self(Program::new(info))
    }
    pub fn programdata_address(&self) -> Result<Option<Pubkey>> {
        self.0.programdata_address()
    }
}
impl<'a, T: CheckId> TryFrom<&'a AccountInfo<'a>> for Interface<'a, T> {
    type Error = Error;
    /// Deserializes the given `info` into a `Program`.
    fn try_from(info: &'a AccountInfo<'a>) -> Result<Self> {
        T::check_id(info.key)?;
        if !info.executable {
            return Err(ErrorCode::InvalidProgramExecutable.into());
        }
        Ok(Self::new(info))
    }
}
impl<'info, T> Deref for Interface<'info, T> {
    type Target = AccountInfo<'info>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
impl<'info, T> AsRef<AccountInfo<'info>> for Interface<'info, T> {
    fn as_ref(&self) -> &AccountInfo<'info> {
        &self.0
    }
}

impl<'info, B, T: CheckId> Accounts<'info, B> for Interface<'info, T> {
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
        Self::try_from(account)
    }
}

impl<T> ToAccountMetas for Interface<'_, T> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        self.0.to_account_metas(is_signer)
    }
}

impl<'info, T> ToAccountInfos<'info> for Interface<'info, T> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        self.0.to_account_infos()
    }
}

impl<'info, T: AccountDeserialize> AccountsExit<'info> for Interface<'info, T> {}

impl<T: AccountDeserialize> Key for Interface<'_, T> {
    fn key(&self) -> Pubkey {
        self.0.key()
    }
}
