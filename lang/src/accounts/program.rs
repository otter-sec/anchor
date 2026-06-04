//! Type validating that the account is the given Program

use {
    crate::{
        error::{Error, ErrorCode},
        solana_program::{
            account_info::AccountInfo,
            bpf_loader_upgradeable::{self, UpgradeableLoaderState},
            instruction::AccountMeta,
            pubkey::Pubkey,
        },
        AccountDeserialize, Accounts, AccountsExit, Id, Key, Result, ToAccountInfos,
        ToAccountMetas,
    },
    std::{collections::BTreeSet, fmt, marker::PhantomData, ops::Deref},
};

/// Type validating that the account is the given Program
///
/// The type has a `programdata_address` function that will return `Option::Some`
/// if the program is owned by the [`BPFUpgradeableLoader`](https://docs.rs/solana-program/latest/solana_program/bpf_loader_upgradeable/index.html)
/// which will contain the `programdata_address` property of the `Program` variant of the [`UpgradeableLoaderState`](https://docs.rs/solana-loader-v3-interface/6.1.0/solana_loader_v3_interface/state/enum.UpgradeableLoaderState.html) enum.
///
/// # Table of Contents
/// - [Basic Functionality](#basic-functionality)
/// - [Generic Program Validation](#generic-program-validation)
/// - [Out of the Box Types](#out-of-the-box-types)
///
/// # Basic Functionality
///
/// For `Program<'info, T>` where T implements Id:
/// Checks:
///
/// - `account_info.key == expected_program`
/// - `account_info.executable == true`
///
/// # Generic Program Validation
///
/// For `Program<'info>` (without type parameter):
/// - Only checks: `account_info.executable == true`
/// - Use this when you only need to verify that an address is executable,
///   without validating against a specific program ID.
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
///     pub program: Program<'info, MyProgram>,
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
/// ## Generic Program Example
/// ```ignore
/// #[derive(Accounts)]
/// pub struct ValidateExecutableProgram<'info> {
///     // Only validates that the provided account is executable
///     pub any_program: Program<'info>,
///     pub authority: Signer<'info>,
/// }
/// ```
///
/// # Out of the Box Types
///
/// Between the [`anchor_lang`](https://docs.rs/anchor-lang/latest/anchor_lang) and [`anchor_spl`](https://docs.rs/anchor_spl/latest/anchor_spl) crates,
/// the following `Program` types are provided out of the box:
///
/// - [`System`](https://docs.rs/anchor-lang/latest/anchor_lang/struct.System.html)
/// - [`AssociatedToken`](https://docs.rs/anchor-spl/latest/anchor_spl/associated_token/struct.AssociatedToken.html)
/// - [`Token`](https://docs.rs/anchor-spl/latest/anchor_spl/token/struct.Token.html)
///
#[derive(Clone)]
pub struct Program<'info, T = ()> {
    info: &'info AccountInfo<'info>,
    _phantom: PhantomData<T>,
}

impl<T: fmt::Debug> fmt::Debug for Program<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Program").field("info", &self.info).finish()
    }
}

impl<'a, T> Program<'a, T> {
    pub(crate) fn new(info: &'a AccountInfo<'a>) -> Program<'a, T> {
        Self {
            info,
            _phantom: PhantomData,
        }
    }

    pub fn programdata_address(&self) -> Result<Option<Pubkey>> {
        if *self.info.owner == bpf_loader_upgradeable::ID {
            let mut data: &[u8] = &self.info.try_borrow_data()?;
            let upgradable_loader_state =
                UpgradeableLoaderState::try_deserialize_unchecked(&mut data)?;

            match upgradable_loader_state {
                UpgradeableLoaderState::Uninitialized
                | UpgradeableLoaderState::Buffer {
                    authority_address: _,
                }
                | UpgradeableLoaderState::ProgramData {
                    slot: _,
                    upgrade_authority_address: _,
                } => {
                    // Unreachable because check in try_from
                    // ensures that program is executable
                    // and therefore a program account.
                    unreachable!()
                }
                UpgradeableLoaderState::Program {
                    programdata_address,
                } => Ok(Some(programdata_address)),
            }
        } else {
            Ok(None)
        }
    }
}

impl<'a, T: Id> TryFrom<&'a AccountInfo<'a>> for Program<'a, T> {
    type Error = Error;
    /// Deserializes the given `info` into a `Program`, checking the pubkey matches
    /// [`T::id`];
    fn try_from(info: &'a AccountInfo<'a>) -> Result<Self> {
        if info.key != &T::id() {
            return Err(Error::from(ErrorCode::InvalidProgramId).with_pubkeys((*info.key, T::id())));
        }
        if !info.executable {
            return Err(ErrorCode::InvalidProgramExecutable.into());
        }
        Ok(Program::new(info))
    }
}

/// `Progam<'info>` with no type parameter (or `Program<'info, ()>`) is a special shorthand for
/// 'any program account'.
impl<'a> TryFrom<&'a AccountInfo<'a>> for Program<'a, ()> {
    type Error = Error;
    /// Deserializes the given `info` into a `Program` without checking the pubkey.
    fn try_from(info: &'a AccountInfo<'a>) -> Result<Self> {
        if !info.executable {
            return Err(ErrorCode::InvalidProgramExecutable.into());
        }
        Ok(Program::new(info))
    }
}

impl<'info, B, T: Id> Accounts<'info, B> for Program<'info, T> {
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
        Program::try_from(account)
    }
}

impl<'info, B> Accounts<'info, B> for Program<'info, ()> {
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
        Program::try_from(account)
    }
}

impl<T> ToAccountMetas for Program<'_, T> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let is_signer = is_signer.unwrap_or(self.info.is_signer);
        let meta = match self.info.is_writable {
            false => AccountMeta::new_readonly(*self.info.key, is_signer),
            true => AccountMeta::new(*self.info.key, is_signer),
        };
        vec![meta]
    }
}

impl<'info, T> ToAccountInfos<'info> for Program<'info, T> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.info.clone()]
    }
}

impl<'info, T> AsRef<AccountInfo<'info>> for Program<'info, T> {
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.info
    }
}

impl<'info, T> Deref for Program<'info, T> {
    type Target = AccountInfo<'info>;

    fn deref(&self) -> &Self::Target {
        self.info
    }
}

impl<'info, T: AccountDeserialize> AccountsExit<'info> for Program<'info, T> {}

impl<T: AccountDeserialize> Key for Program<'_, T> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}

#[cfg(test)]
mod tests {
    use {super::*, crate::system_program::System};

    fn account_info<'a>(
        key: &'a Pubkey,
        owner: &'a Pubkey,
        lamports: &'a mut u64,
        data: &'a mut [u8],
        executable: bool,
    ) -> AccountInfo<'a> {
        AccountInfo::new(key, false, false, lamports, data, owner, executable)
    }

    #[test]
    fn program_system_rejects_non_system_account() {
        let wrong_key = Pubkey::new_from_array([7u8; 32]);
        let owner = Pubkey::default();
        let mut lamports = 0u64;
        let mut data = [];
        let info = account_info(&wrong_key, &owner, &mut lamports, &mut data, true);

        let err = Program::<System>::try_from(&info).expect_err("non-system key must be rejected");
        match err {
            Error::AnchorError(e) => {
                assert_eq!(e.error_code_number, ErrorCode::InvalidProgramId as u32)
            }
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    #[test]
    fn program_system_accepts_system_account() {
        let key = System::id();
        let owner = Pubkey::default();
        let mut lamports = 0u64;
        let mut data = [];
        let info = account_info(&key, &owner, &mut lamports, &mut data, true);

        Program::<System>::try_from(&info).expect("system program account must be accepted");
    }

    #[test]
    fn program_system_rejects_non_executable_system_account() {
        let key = System::id();
        let owner = Pubkey::default();
        let mut lamports = 0u64;
        let mut data = [];
        let info = account_info(&key, &owner, &mut lamports, &mut data, false);

        let err = Program::<System>::try_from(&info)
            .expect_err("non-executable account must be rejected");
        match err {
            Error::AnchorError(e) => assert_eq!(
                e.error_code_number,
                ErrorCode::InvalidProgramExecutable as u32,
            ),
            other => panic!("unexpected error variant: {other:?}"),
        }
    }

    // The `Program<'info, ()>` shorthand should still accept any executable account.
    #[test]
    fn program_unit_accepts_arbitrary_executable() {
        let key = Pubkey::new_from_array([7u8; 32]);
        let owner = Pubkey::default();
        let mut lamports = 0u64;
        let mut data = [];
        let info = account_info(&key, &owner, &mut lamports, &mut data, true);

        Program::<()>::try_from(&info).expect("any executable account must be accepted");
    }
}
