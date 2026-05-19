//! Account container that checks ownership on deserialization.

use {
    crate::{
        accounts::account::Account,
        error::ErrorCode,
        solana_program::{
            account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey, system_program,
        },
        AccountDeserialize, AccountSerialize, Accounts, AccountsClose, AccountsExit, CheckOwner,
        Key, Owners, Result, ToAccountInfos, ToAccountMetas,
    },
    std::{
        collections::BTreeSet,
        fmt,
        ops::{Deref, DerefMut},
    },
};

/// Wrapper around [`AccountInfo`]
/// that verifies program ownership and deserializes underlying data into a Rust type.
///
/// # Table of Contents
/// - [Basic Functionality](#basic-functionality)
/// - [Using InterfaceAccount with non-anchor types](#using-interface-account-with-non-anchor-types)
/// - [Out of the box wrapper types](#out-of-the-box-wrapper-types)
///
/// # Basic Functionality
///
/// InterfaceAccount checks that `T::owners().contains(Account.info.owner)`.
/// This means that the data type that Accounts wraps around (`=T`) needs to
/// implement the [Owners trait](crate::Owners).
/// The `#[account]` attribute implements the Owners trait for
/// a struct using multiple `crate::ID`s declared by [`declareId`](crate::declare_id)
/// in the same program. It follows that InterfaceAccount can also be used
/// with a `T` that comes from a different program.
///
/// Checks:
///
/// - `T::owners().contains(InterfaceAccount.info.owner)`
/// - `!(InterfaceAccount.info.owner == SystemProgram && InterfaceAccount.info.lamports() == 0)`
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
///     pub my_account: InterfaceAccount<'info, MyData> // checks that my_account.info.owner == Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS
///     pub auth_account: InterfaceAccount<'info, Auth> // checks that auth_account.info.owner == FEZGUxNhZWpYPj9MJCrZJvUo1iF9ys34UHx52y4SzVW9
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
/// # Using InterfaceAccount with non-anchor programs
///
/// InterfaceAccount can also be used with non-anchor programs. The data types from
/// those programs are not annotated with `#[account]` so you have to
/// - create a wrapper type around the structs you want to wrap with InterfaceAccount
/// - implement the functions required by InterfaceAccount yourself
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
/// ### SPL Types
///
/// Anchor provides wrapper types to access accounts owned by the token programs. Use
/// ```ignore
/// use anchor_spl::token_interface::TokenAccount;
///
/// #[derive(Accounts)]
/// pub struct Example {
///     pub my_acc: InterfaceAccount<'info, TokenAccount>
/// }
/// ```
/// to access token accounts and
/// ```ignore
/// use anchor_spl::token_interface::Mint;
///
/// #[derive(Accounts)]
/// pub struct Example {
///     pub my_acc: InterfaceAccount<'info, Mint>
/// }
/// ```
/// to access mint accounts.
#[derive(Clone)]
pub struct InterfaceAccount<'info, T: AccountSerialize + AccountDeserialize + Clone> {
    account: Account<'info, T>,
    // The owner here is used to make sure that changes aren't incorrectly propagated
    // to an account with a modified owner
    owner: Pubkey,
}

impl<T: AccountSerialize + AccountDeserialize + Clone + fmt::Debug> fmt::Debug
    for InterfaceAccount<'_, T>
{
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.account.fmt_with_name("InterfaceAccount", f)
    }
}

impl<'a, T: AccountSerialize + AccountDeserialize + Clone> InterfaceAccount<'a, T> {
    fn new(info: &'a AccountInfo<'a>, account: T) -> Self {
        let owner = *info.owner;
        Self {
            account: Account::new(info, account),
            owner,
        }
    }

    pub fn into_inner(self) -> T {
        self.account.into_inner()
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
        self.account.set_inner(inner);
    }
}

impl<'a, T: AccountSerialize + AccountDeserialize + CheckOwner + Clone> InterfaceAccount<'a, T> {
    /// Deserializes the given `info` into an `InterfaceAccount`.
    #[inline(never)]
    pub fn try_from(info: &'a AccountInfo<'a>) -> Result<Self> {
        if info.owner == &system_program::ID && info.lamports() == 0 {
            return Err(ErrorCode::AccountNotInitialized.into());
        }
        T::check_owner(info.owner)?;
        let mut data: &[u8] = &info.try_borrow_data()?;
        Ok(Self::new(info, T::try_deserialize(&mut data)?))
    }

    /// Deserializes the given `info` into an `InterfaceAccount` without checking the account
    /// discriminator. Be careful when using this and avoid it if possible.
    #[inline(never)]
    pub fn try_from_unchecked(info: &'a AccountInfo<'a>) -> Result<Self> {
        if info.owner == &system_program::ID && info.lamports() == 0 {
            return Err(ErrorCode::AccountNotInitialized.into());
        }
        T::check_owner(info.owner)?;
        let mut data: &[u8] = &info.try_borrow_data()?;
        Ok(Self::new(info, T::try_deserialize_unchecked(&mut data)?))
    }

    /// Reloads the account from storage. This is useful, for example, when observing side effects
    /// after CPI.
    ///
    /// This method also validates that the account is owned by one of the expected programs.
    pub fn reload(&mut self) -> Result<()> {
        let info: &AccountInfo = self.account.as_ref();
        T::check_owner(info.owner)?;

        // Re-deserialize fresh data into the inner account.
        self.account.set_inner({
            let mut data: &[u8] = &info.try_borrow_data()?;
            T::try_deserialize(&mut data)?
        });
        Ok(())
    }
}

impl<'info, B, T: AccountSerialize + AccountDeserialize + CheckOwner + Clone> Accounts<'info, B>
    for InterfaceAccount<'info, T>
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
        Self::try_from(account)
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Owners + Clone> AccountsExit<'info>
    for InterfaceAccount<'info, T>
{
    fn exit(&self, program_id: &Pubkey) -> Result<()> {
        self.account
            .exit_with_expected_owner(&self.owner, program_id)
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone> AccountsClose<'info>
    for InterfaceAccount<'info, T>
{
    fn close(&self, sol_destination: AccountInfo<'info>) -> Result<()> {
        self.account.close(sol_destination)
    }
}

impl<T: AccountSerialize + AccountDeserialize + Clone> ToAccountMetas for InterfaceAccount<'_, T> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        self.account.to_account_metas(is_signer)
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone> ToAccountInfos<'info>
    for InterfaceAccount<'info, T>
{
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        self.account.to_account_infos()
    }
}

impl<'info, T: AccountSerialize + AccountDeserialize + Clone> AsRef<AccountInfo<'info>>
    for InterfaceAccount<'info, T>
{
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.account.as_ref()
    }
}

impl<T: AccountSerialize + AccountDeserialize + Clone> AsRef<T> for InterfaceAccount<'_, T> {
    fn as_ref(&self) -> &T {
        self.account.as_ref()
    }
}

impl<T: AccountSerialize + AccountDeserialize + Clone> Deref for InterfaceAccount<'_, T> {
    type Target = T;

    fn deref(&self) -> &Self::Target {
        self.account.deref()
    }
}

impl<T: AccountSerialize + AccountDeserialize + Clone> DerefMut for InterfaceAccount<'_, T> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        self.account.deref_mut()
    }
}

impl<T: AccountSerialize + AccountDeserialize + Clone> Key for InterfaceAccount<'_, T> {
    fn key(&self) -> Pubkey {
        self.account.key()
    }
}
