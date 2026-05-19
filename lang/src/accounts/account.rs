//! Account container that checks ownership on deserialization.

use {
    crate::{
        bpf_writer::BpfWriter,
        error::{Error, ErrorCode},
        solana_program::{
            account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey, system_program,
        },
        AccountDeserialize, AccountSerialize, Accounts, AccountsClose, AccountsExit, Key, Owner,
        Result, ToAccountInfos, ToAccountMetas,
    },
    std::{
        collections::BTreeSet,
        fmt,
        ops::{Deref, DerefMut},
    },
};

/// Wrapper around [`AccountInfo`] that verifies program ownership
/// and deserializes underlying data into a Rust type.
///
/// # Table of Contents
/// - [Basic Functionality](#basic-functionality)
/// - [Using Account with non-anchor types](#using-account-with-non-anchor-types)
/// - [Out of the box wrapper types](#out-of-the-box-wrapper-types)
///
/// # Basic Functionality
///
/// Account checks that `Account.info.owner == T::owner()`.
/// This means that the data type that Accounts wraps around (`=T`) needs to
/// implement the [Owner trait](crate::Owner).
/// The `#[account]` attribute implements the Owner trait for
/// a struct using the `crate::ID` declared by [`declare_id`](crate::declare_id)
/// in the same program. It follows that Account can also be used
/// with a `T` that comes from a different program.
///
/// Checks:
///
/// - `Account.info.owner == T::owner()`
/// - `!(Account.info.owner == SystemProgram && Account.info.lamports() == 0)`
///
/// # Example
/// ```ignore
/// use anchor_lang::prelude::*;
/// use other_program::Auth;
///
/// declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
///
/// #[program]
/// mod hello_anchor {
///     use super::*;
///     pub fn set_data(ctx: Context<SetData>, data: u64) -> Result<()> {
///         if (*ctx.accounts.auth_account).authorized {
///             (*ctx.accounts.my_account).data = data;
///         }
///         Ok(())
///     }
/// }
///
/// #[account]
/// #[derive(Default)]
/// pub struct MyData {
///     pub data: u64
/// }
///
/// #[derive(Accounts)]
/// pub struct SetData<'info> {
///     #[account(mut)]
///     pub my_account: Account<'info, MyData> // checks that my_account.info.owner == Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS
///     pub auth_account: Account<'info, Auth> // checks that auth_account.info.owner == FEZGUxNhZWpYPj9MJCrZJvUo1iF9ys34UHx52y4SzVW9
/// }
///
/// // In a different program
///
/// ...
/// declare_id!("FEZGUxNhZWpYPj9MJCrZJvUo1iF9ys34UHx52y4SzVW9");
/// #[account]
/// #[derive(Default)]
/// pub struct Auth {
///     pub authorized: bool
/// }
/// ...
/// ```
///
/// # Using Account with non-anchor programs
///
/// Account can also be used with non-anchor programs. The data types from
/// those programs are not annotated with `#[account]` so you have to
/// - create a wrapper type around the structs you want to wrap with Account
/// - implement the functions required by Account yourself
///
/// instead of using `#[account]`. You only have to implement a fraction of the
/// functions `#[account]` generates. See the example below for the code you have
/// to write.
///
/// The mint wrapper type that Anchor provides out of the box for the token program ([source](https://github.com/otter-sec/anchor/blob/master/spl/src/token.rs))
/// ```ignore
/// #[derive(Clone)]
/// pub struct Mint(spl_token::state::Mint);
///
/// // This is necessary so we can use "anchor_spl::token::Mint::LEN"
/// // because rust does not resolve "anchor_spl::token::Mint::LEN" to
/// // "spl_token::state::Mint::LEN" automatically
/// impl Mint {
///     pub const LEN: usize = spl_token::state::Mint::LEN;
/// }
///
/// // You don't have to implement the "try_deserialize" function
/// // from this trait. It delegates to
/// // "try_deserialize_unchecked" by default which is what we want here
/// // because non-anchor accounts don't have a discriminator to check
/// impl anchor_lang::AccountDeserialize for Mint {
///     fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
///         spl_token::state::Mint::unpack(buf).map(Mint)
///     }
/// }
/// // AccountSerialize defaults to a no-op which is what we want here
/// // because it's a foreign program, so our program does not
/// // have permission to write to the foreign program's accounts anyway
/// impl anchor_lang::AccountSerialize for Mint {}
///
/// impl anchor_lang::Owner for Mint {
///     fn owner() -> Pubkey {
///         // pub use spl_token::ID is used at the top of the file
///         ID
///     }
/// }
///
/// // Implement the "std::ops::Deref" trait for better user experience
/// impl Deref for Mint {
///     type Target = spl_token::state::Mint;
///
///     fn deref(&self) -> &Self::Target {
///         &self.0
///     }
/// }
/// ```
///
/// ## Out of the box wrapper types
///
/// ### Accessing BPFUpgradeableLoader Data
///
/// Anchor provides wrapper types to access data stored in programs owned by the BPFUpgradeableLoader
/// such as the upgrade authority. If you're interested in the data of a program account, you can use
/// ```ignore
/// Account<'info, BpfUpgradeableLoaderState>
/// ```
/// and then match on its contents inside your instruction function.
///
/// Alternatively, you can use
/// ```ignore
/// Account<'info, ProgramData>
/// ```
/// to let anchor do the matching for you and return the ProgramData variant of BpfUpgradeableLoaderState.
///
/// # Example
/// ```ignore
/// use anchor_lang::prelude::*;
/// use crate::program::MyProgram;
///
/// declare_id!("Cum9tTyj5HwcEiAmhgaS7Bbj4UczCwsucrCkxRECzM4e");
///
/// #[program]
/// pub mod my_program {
///     use super::*;
///
///     pub fn set_initial_admin(
///         ctx: Context<SetInitialAdmin>,
///         admin_key: Pubkey
///     ) -> Result<()> {
///         ctx.accounts.admin_settings.admin_key = admin_key;
///         Ok(())
///     }
///
///     pub fn set_admin(...){...}
///
///     pub fn set_settings(...){...}
/// }
///
/// #[account]
/// #[derive(Default, Debug)]
/// pub struct AdminSettings {
///     admin_key: Pubkey
/// }
///
/// #[derive(Accounts)]
/// pub struct SetInitialAdmin<'info> {
///     #[account(init, payer = authority, seeds = [b"admin"], bump)]
///     pub admin_settings: Account<'info, AdminSettings>,
///     #[account(mut)]
///     pub authority: Signer<'info>,
///     #[account(constraint = program.programdata_address()? == Some(program_data.key()))]
///     pub program: Program<'info, MyProgram>,
///     #[account(constraint = program_data.upgrade_authority_address == Some(authority.key()))]
///     pub program_data: Account<'info, ProgramData>,
///     pub system_program: Program<'info, System>,
/// }
/// ```
///
/// This example solves a problem you may face if your program has admin settings: How do you set the
/// admin key for restricted functionality after deployment? Setting the admin key itself should
/// be a restricted action but how do you restrict it without having set an admin key?
/// You're stuck in a loop.
/// One solution is to use the upgrade authority of the program as the initial
/// (or permanent) admin key.
///
/// ### SPL Types
///
/// Anchor provides wrapper types to access accounts owned by the token program. Use
/// ```ignore
/// use anchor_spl::token::TokenAccount;
///
/// #[derive(Accounts)]
/// pub struct Example {
///     pub my_acc: Account<'info, TokenAccount>
/// }
/// ```
/// to access token accounts and
/// ```ignore
/// use anchor_spl::token::Mint;
///
/// #[derive(Accounts)]
/// pub struct Example {
///     pub my_acc: Account<'info, Mint>
/// }
/// ```
/// to access mint accounts.
#[derive(Clone)]
pub struct Account<'info, T: AccountSerialize + AccountDeserialize + Clone> {
    account: T,
    info: &'info AccountInfo<'info>,
}

impl<T: AccountSerialize + AccountDeserialize + Clone + fmt::Debug> fmt::Debug for Account<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.fmt_with_name("Account", f)
    }
}

impl<T: AccountSerialize + AccountDeserialize + Clone + fmt::Debug> Account<'_, T> {
    pub(crate) fn fmt_with_name(&self, name: &str, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct(name)
            .field("account", &self.account)
            .field("info", &self.info)
            .finish()
    }
}

impl<'a, T: AccountSerialize + AccountDeserialize + Clone> Account<'a, T> {
    pub(crate) fn new(info: &'a AccountInfo<'a>, account: T) -> Account<'a, T> {
        Self { info, account }
    }

    pub(crate) fn exit_with_expected_owner(
        &self,
        expected_owner: &Pubkey,
        program_id: &Pubkey,
    ) -> Result<()> {
        // Only persist if the owner is the current program and the account is not closed.
        if expected_owner == program_id && !crate::common::is_closed(self.info) {
            let mut data = self.info.try_borrow_mut_data()?;
            let dst: &mut [u8] = &mut data;
            let mut writer = BpfWriter::new(dst);
            self.account.try_serialize(&mut writer)?;
        }
        Ok(())
    }

    pub fn into_inner(self) -> T {
        self.account
    }

    /// Sets the inner account.
    ///
    /// Instead of this:
    /// ```ignore
    /// pub fn new_user(ctx: Context<CreateUser>, new_user:User) -> Result<()> {
    ///     (*ctx.accounts.user_to_create).name = new_user.name;
    ///     (*ctx.accounts.user_to_create).age = new_user.age;
    ///     (*ctx.accounts.user_to_create).address = new_user.address;
    /// }
    /// ```
    /// You can do this:
    /// ```ignore
    /// pub fn new_user(ctx: Context<CreateUser>, new_user:User) -> Result<()> {
    ///     ctx.accounts.user_to_create.set_inner(new_user);
    /// }
    /// ```
    pub fn set_inner(&mut self, inner: T) {
        self.account = inner;
    }
}

impl<'a, T: AccountSerialize + AccountDeserialize + Owner + Clone> Account<'a, T> {
    /// Reloads the account from storage. This is useful, for example, when
    /// observing side effects after CPI.
    ///
    /// This method also re-validates that the program owner has not
    /// changed since the initial validation
    pub fn reload(&mut self) -> Result<()> {
        if self.info.owner != &T::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
                .with_pubkeys((*self.info.owner, T::owner())));
        }

        let mut data: &[u8] = &self.info.try_borrow_data()?;
        self.account = T::try_deserialize(&mut data)?;
        Ok(())
    }

    /// Deserializes the given `info` into a `Account`.
    #[inline(never)]
    pub fn try_from(info: &'a AccountInfo<'a>) -> Result<Account<'a, T>> {
        if info.owner == &system_program::ID && info.lamports() == 0 {
            return Err(ErrorCode::AccountNotInitialized.into());
        }
        if info.owner != &T::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
                .with_pubkeys((*info.owner, T::owner())));
        }
        let mut data: &[u8] = &info.try_borrow_data()?;
        Ok(Account::new(info, T::try_deserialize(&mut data)?))
    }

    /// Deserializes the given `info` into a `Account` without checking
    /// the account discriminator. Be careful when using this and avoid it if
    /// possible.
    #[inline(never)]
    pub fn try_from_unchecked(info: &'a AccountInfo<'a>) -> Result<Account<'a, T>> {
        if info.owner == &system_program::ID && info.lamports() == 0 {
            return Err(ErrorCode::AccountNotInitialized.into());
        }
        if info.owner != &T::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
                .with_pubkeys((*info.owner, T::owner())));
        }
        let mut data: &[u8] = &info.try_borrow_data()?;
        Ok(Account::new(info, T::try_deserialize_unchecked(&mut data)?))
    }
}

impl<'info, B, T: AccountSerialize + AccountDeserialize + Owner + Clone> Accounts<'info, B>
    for Account<'info, T>
where
    T: AccountSerialize + AccountDeserialize + Owner + Clone,
{
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
        Account::try_from(account)
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Owner + Clone> AccountsExit<'info>
    for Account<'info, T>
{
    fn exit(&self, program_id: &Pubkey) -> Result<()> {
        self.exit_with_expected_owner(&T::owner(), program_id)
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone> AccountsClose<'info>
    for Account<'info, T>
{
    fn close(&self, sol_destination: AccountInfo<'info>) -> Result<()> {
        crate::common::close(self.as_ref(), sol_destination.as_ref())
    }
}

impl<T: AccountSerialize + AccountDeserialize + Clone> ToAccountMetas for Account<'_, T> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let is_signer = is_signer.unwrap_or(self.info.is_signer);
        let meta = match self.info.is_writable {
            false => AccountMeta::new_readonly(*self.info.key, is_signer),
            true => AccountMeta::new(*self.info.key, is_signer),
        };
        vec![meta]
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone> ToAccountInfos<'info>
    for Account<'info, T>
{
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.info.clone()]
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone> AsRef<AccountInfo<'info>>
    for Account<'info, T>
{
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.info
    }
}

impl<T: AccountSerialize + AccountDeserialize + Clone> AsRef<T> for Account<'_, T> {
    fn as_ref(&self) -> &T {
        &self.account
    }
}

impl<T: AccountSerialize + AccountDeserialize + Clone> Deref for Account<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        &(self).account
    }
}

impl<T: AccountSerialize + AccountDeserialize + Clone> DerefMut for Account<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        #[cfg(feature = "anchor-debug")]
        if !self.info.is_writable {
            crate::solana_program::msg!("The given Account is not mutable");
            panic!();
        }
        &mut self.account
    }
}

impl<T: AccountSerialize + AccountDeserialize + Clone> Key for Account<'_, T> {
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}
