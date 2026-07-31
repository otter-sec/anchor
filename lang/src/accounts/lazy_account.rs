//! Like [`Account`](crate::Account), but deserializes on-demand.

use {
    crate::{
        error::{Error, ErrorCode},
        AccountInfo, AccountMeta, AccountSerialize, Accounts, AccountsClose, Discriminator, Key,
        Owner, Pubkey, Result, ToAccountInfo, ToAccountInfos, ToAccountMetas,
    },
    std::{cell::RefCell, collections::BTreeSet, fmt, mem::MaybeUninit, rc::Rc},
};

/// Deserialize account data lazily (on-demand).
///
/// Anchor uses [`borsh`] deserialization by default, which can be expensive for both memory and
/// compute units usage.
///
/// With the regular [`Account`] type, all account data gets deserialized, even the fields not used
/// by your instruction. On contrast, [`LazyAccount`] allows you to deserialize individual fields,
/// saving both memory and compute units.
///
/// # Table of contents
///
/// - [When to use](#when-to-use)
/// - [Features](#features)
/// - [Example](#example)
/// - [Safety](#safety)
/// - [Performance](#performance)
///     - [Memory](#memory)
///     - [Compute units](#compute-units)
///
/// # When to use
///
/// This is currently an experimental account type, and therefore should only be used when you're
/// running into performance issues.
///
/// It's best to use [`LazyAccount`] when you only need to deserialize some of the fields,
/// especially if the account is read-only.
///
/// Replacing [`Account`] (including `Box`ed) with [`LazyAccount`] *can* improve both stack memory
/// and compute unit usage. However, this is not guaranteed. For example, if you need to
/// deserialize the account fully, using [`LazyAccount`] will have additional overhead and
/// therefore use slightly more compute units.
///
/// Currently, using the `mut` constraint eventually results in the whole account getting
/// deserialized, meaning it won't use fewer compute units compared to [`Account`]. This might get
/// optimized in the future.
///
/// # Features
///
/// - Can be used as a replacement for [`Account`].
/// - Checks the account owner and its discriminator.
/// - Does **not** check the type layout matches the defined layout.
/// - All account data can be deserialized with `load` and `load_mut` methods. These methods are
///   non-inlined, meaning that they're less likely to cause stack violation errors.
/// - Each individual field can be deserialized with the generated `load_<field>` and
///   `load_mut_<field>` methods.
///
/// # Example
///
/// ```ignore
/// use anchor_lang::prelude::*;
///
/// declare_id!("LazyAccount11111111111111111111111111111111");
///
/// #[program]
/// pub mod lazy_account {
///     use super::*;
///
///     pub fn init(ctx: Context<Init>) -> Result<()> {
///         let mut my_account = ctx.accounts.my_account.load_mut()?;
///         my_account.authority = ctx.accounts.authority.key();
///
///         // Fill the dynamic data
///         for _ in 0..MAX_DATA_LEN {
///             my_account.dynamic.push(ctx.accounts.authority.key());
///         }
///
///         Ok(())
///     }
///
///     pub fn read(ctx: Context<Read>) -> Result<()> {
///         // Cached load due to the `has_one` constraint
///         let authority = ctx.accounts.my_account.load_authority()?;
///         msg!("Authority: {}", authority);
///         Ok(())
///     }
///
///     pub fn write(ctx: Context<Write>, new_authority: Pubkey) -> Result<()> {
///         // Cached load due to the `has_one` constraint
///         *ctx.accounts.my_account.load_mut_authority()? = new_authority;
///         Ok(())
///     }
/// }
///
/// #[derive(Accounts)]
/// pub struct Init<'info> {
///     #[account(mut)]
///     pub authority: Signer<'info>,
///     #[account(
///         init,
///         payer = authority,
///         space = MyAccount::DISCRIMINATOR.len() + MyAccount::INIT_SPACE
///     )]
///     pub my_account: LazyAccount<'info, MyAccount>,
///     pub system_program: Program<'info, System>,
/// }
///
/// #[derive(Accounts)]
/// pub struct Read<'info> {
///     pub authority: Signer<'info>,
///     #[account(has_one = authority)]
///     pub my_account: LazyAccount<'info, MyAccount>,
/// }
///
/// #[derive(Accounts)]
/// pub struct Write<'info> {
///     pub authority: Signer<'info>,
///     #[account(mut, has_one = authority)]
///     pub my_account: LazyAccount<'info, MyAccount>,
/// }
///
/// const MAX_DATA_LEN: usize = 256;
///
/// #[account]
/// #[derive(InitSpace)]
/// pub struct MyAccount {
///     pub authority: Pubkey,
///     pub fixed: [Pubkey; 8],
///     // Dynamic sized data also works, unlike `AccountLoader`
///     #[max_len(MAX_DATA_LEN)]
///     pub dynamic: Vec<Pubkey>,
/// }
/// ```
///
/// # Safety
///
/// The safety checks are done using the account's discriminator and the account's owner (similar
/// to [`Account`]). However, you should be extra careful when deserializing individual fields if,
/// for example, the account needs to be migrated. Make sure the previously serialized data always
/// matches the account's type identically.
///
/// # Performance
///
/// ## Memory
///
/// All fields (including the inner account type) are heap allocated. It only uses 24 bytes (3x
/// pointer size) of stack memory in total.
///
/// It's worth noting that where the account is being deserialized matters. For example, the main
/// place where Anchor programs are likely to hit stack violation errors is a generated function
/// called `try_accounts` (you might be familiar with it from the mangled build logs). This is
/// where the instruction is deserialized and constraints are run. Although having everything at the
/// same place is convenient for using constraints, this also makes it very easy to use the fixed
/// amount of stack space (4096 bytes) SVM allocates just by increasing the number of accounts the
/// instruction has. In SVM, each function has its own stack frame, meaning that it's possible to
/// deserialize more accounts simply by deserializing them inside other functions (rather than in
/// `try_accounts` which is already quite heavy).
///
/// The mentioned stack limitation can be solved using dynamic stack frames, see [SIMD-0166].
///
/// ## Compute units
///
/// Compute is harder to formulate, as it varies based on the inner account's type. That being said,
/// there are a few things you can do to optimize compute units usage when using [`LazyAccount`]:
///
/// - Order account fields from fixed-size data (e.g. `u8`, `Pubkey`) to dynamic data (e.g. `Vec`).
/// - Order account fields based on how frequently the field is accessed (starting with the most
///   frequent).
/// - Reduce or limit dynamic fields.
///
/// [`borsh`]: crate::prelude::borsh
/// [`Account`]: crate::prelude::Account
/// [SIMD-0166]: https://github.com/solana-foundation/solana-improvement-documents/pull/166
pub struct LazyAccount<'info, T>
where
    T: AccountSerialize + Discriminator + Owner + Clone,
{
    /// **INTERNAL FIELD DO NOT USE!**
    #[doc(hidden)]
    pub __info: &'info AccountInfo<'info>,
    /// **INTERNAL FIELD DO NOT USE!**
    #[doc(hidden)]
    pub __account: Rc<RefCell<MaybeUninit<T>>>,
    /// **INTERNAL FIELD DO NOT USE!**
    #[doc(hidden)]
    pub __fields: Rc<RefCell<Option<Vec<bool>>>>,
}

impl<T> fmt::Debug for LazyAccount<'_, T>
where
    T: AccountSerialize + Discriminator + Owner + Clone + fmt::Debug,
{
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.debug_struct("LazyAccount")
            .field("info", &self.__info)
            .field("account", &self.__account)
            .field("fields", &self.__fields)
            .finish()
    }
}

impl<'info, T> LazyAccount<'info, T>
where
    T: AccountSerialize + Discriminator + Owner + Clone,
{
    fn new(info: &'info AccountInfo<'info>) -> LazyAccount<'info, T> {
        Self {
            __info: info,
            __account: Rc::new(RefCell::new(MaybeUninit::uninit())),
            __fields: Rc::new(RefCell::new(None)),
        }
    }

    /// Check both the owner and the discriminator.
    pub fn try_from(info: &'info AccountInfo<'info>) -> Result<LazyAccount<'info, T>> {
        let data = &info.try_borrow_data()?;
        let disc = T::DISCRIMINATOR;
        if data.len() < disc.len() {
            return Err(ErrorCode::AccountDiscriminatorNotFound.into());
        }

        let given_disc = &data[..disc.len()];
        if given_disc != disc {
            return Err(ErrorCode::AccountDiscriminatorMismatch.into());
        }

        Self::try_from_unchecked(info)
    }

    /// Check the owner but **not** the discriminator.
    pub fn try_from_unchecked(info: &'info AccountInfo<'info>) -> Result<LazyAccount<'info, T>> {
        if info.owner != &T::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
                .with_pubkeys((*info.owner, T::owner())));
        }

        Ok(LazyAccount::new(info))
    }

    /// Unload the deserialized account value by resetting the cache.
    ///
    /// This is useful when observing side-effects of CPIs, and will re-check the account
    /// owner and discriminator.
    ///
    /// # Usage
    ///
    /// ```ignore
    /// // Load the initial value
    /// let initial_value = ctx.accounts.my_account.load_field()?;
    ///
    /// // Do CPI...
    ///
    /// // We still have a reference to the account from `initial_value`, drop it before `unload`
    /// drop(initial_value);
    ///
    /// // Load the updated value
    /// let updated_value = ctx.accounts.my_account.unload()?.load_field()?;
    /// ```
    ///
    /// # Panics
    ///
    /// If there is an existing reference (mutable or not) created by any of the `load` methods.
    pub fn unload(&self) -> Result<&Self> {
        // Re-run load-time checks (owner, discriminator)
        drop(Self::try_from(self.__info)?);
        // TODO: Should we drop the initialized fields manually?
        *self.__account.borrow_mut() = MaybeUninit::uninit();
        *self.__fields.borrow_mut() = None;
        Ok(self)
    }
}

impl<'info, B, T> Accounts<'info, B> for LazyAccount<'info, T>
where
    T: AccountSerialize + Discriminator + Owner + Clone,
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
        LazyAccount::try_from(account)
    }
}

impl<'info, T> AccountsClose<'info> for LazyAccount<'info, T>
where
    T: AccountSerialize + Discriminator + Owner + Clone,
{
    fn close(&self, sol_destination: AccountInfo<'info>) -> Result<()> {
        crate::common::close(self.as_ref(), sol_destination.as_ref())
    }
}

impl<T> ToAccountMetas for LazyAccount<'_, T>
where
    T: AccountSerialize + Discriminator + Owner + Clone,
{
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let is_signer = is_signer.unwrap_or(self.__info.is_signer);
        let meta = match self.__info.is_writable {
            false => AccountMeta::new_readonly(*self.__info.key, is_signer),
            true => AccountMeta::new(*self.__info.key, is_signer),
        };
        vec![meta]
    }
}

impl<'info, T> ToAccountInfos<'info> for LazyAccount<'info, T>
where
    T: AccountSerialize + Discriminator + Owner + Clone,
{
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.to_account_info()]
    }
}

impl<'info, T> AsRef<AccountInfo<'info>> for LazyAccount<'info, T>
where
    T: AccountSerialize + Discriminator + Owner + Clone,
{
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.__info
    }
}

impl<T> Key for LazyAccount<'_, T>
where
    T: AccountSerialize + Discriminator + Owner + Clone,
{
    fn key(&self) -> Pubkey {
        *self.__info.key
    }
}
