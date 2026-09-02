//! Type facilitating on demand zero copy deserialization.

use {
    crate::{
        bpf_writer::BpfWriter,
        error::{Error, ErrorCode},
        solana_program::{account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey},
        Accounts, AccountsClose, AccountsExit, Key, Owner, Result, ToAccountInfos, ToAccountMetas,
        ZeroCopy,
    },
    std::{
        cell::{Ref, RefMut},
        collections::BTreeSet,
        fmt,
        io::Write,
        marker::PhantomData,
        mem,
        ops::DerefMut,
    },
};

/// Type facilitating on demand zero copy deserialization.
///
/// Note that using accounts in this way is distinctly different from using,
/// for example, the [`Account`](crate::accounts::account::Account). Namely,
/// one must call
/// - `load_init` after initializing an account (this will ignore the missing
///   account discriminator that gets added only after the user's instruction code)
/// - `load` when the account is not mutable
/// - `load_mut` when the account is mutable
///
/// For more details on zero-copy-deserialization, see the
/// [`account`](crate::account) attribute.
/// <p style=";padding:0.75em;border: 1px solid #ee6868">
/// <strong>⚠️ </strong> When using this type it's important to be mindful
/// of any calls to the <code>load</code> functions so as not to
/// induce a <code>RefCell</code> panic, especially when sharing accounts across CPI
/// boundaries. When in doubt, one should make sure all refs resulting from
/// a call to a <code>load</code> function are dropped before CPI.
/// This can be done explicitly by calling <code>drop(my_var)</code> or implicitly
/// by wrapping the code using the <code>Ref</code> in braces <code>{..}</code> or
/// moving it into its own function.
/// </p>
///
/// # Example
/// ```ignore
/// use anchor_lang::prelude::*;
///
/// declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
///
/// #[program]
/// pub mod bar {
///     use super::*;
///
///     pub fn create_bar(ctx: Context<CreateBar>, data: u64) -> Result<()> {
///         let bar = &mut ctx.accounts.bar.load_init()?;
///         bar.authority = ctx.accounts.authority.key();
///         bar.data = data;
///         Ok(())
///     }
///
///     pub fn update_bar(ctx: Context<UpdateBar>, data: u64) -> Result<()> {
///         (*ctx.accounts.bar.load_mut()?).data = data;
///         Ok(())
///     }
/// }
///
/// #[account(zero_copy)]
/// #[derive(Default)]
/// pub struct Bar {
///     authority: Pubkey,
///     data: u64
/// }
///
/// #[derive(Accounts)]
/// pub struct CreateBar<'info> {
///     #[account(
///         init,
///         payer = authority
///     )]
///     bar: AccountLoader<'info, Bar>,
///     #[account(mut)]
///     authority: Signer<'info>,
///     system_program: AccountInfo<'info>,
/// }
///
/// #[derive(Accounts)]
/// pub struct UpdateBar<'info> {
///     #[account(
///         mut,
///         has_one = authority,
///     )]
///     pub bar: AccountLoader<'info, Bar>,
///     pub authority: Signer<'info>,
/// }
/// ```
#[derive(Clone)]
pub struct AccountLoader<'info, T: ZeroCopy + Owner> {
    acc_info: &'info AccountInfo<'info>,
    phantom: PhantomData<&'info T>,
}

impl<T: ZeroCopy + Owner + fmt::Debug> fmt::Debug for AccountLoader<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("AccountLoader")
            .field("acc_info", &self.acc_info)
            .field("phantom", &self.phantom)
            .finish()
    }
}

impl<'info, T: ZeroCopy + Owner> AccountLoader<'info, T> {
    /// Constructs a new [`AccountLoader`] without performing any account validation checks.
    ///
    /// - [`Self::try_from`] to perform all checks, or
    /// - [`Self::try_from_unchecked`] to check the owner but not the discriminator
    pub fn new_unchecked(acc_info: &'info AccountInfo<'info>) -> AccountLoader<'info, T> {
        Self {
            acc_info,
            phantom: PhantomData,
        }
    }

    /// Constructs a new [`AccountLoader`] from a previously initialized account.
    #[inline(never)]
    pub fn try_from(acc_info: &'info AccountInfo<'info>) -> Result<AccountLoader<'info, T>> {
        if acc_info.owner != &T::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
                .with_pubkeys((*acc_info.owner, T::owner())));
        }

        let data = &acc_info.try_borrow_data()?;
        let disc = T::DISCRIMINATOR;
        if data.len() < disc.len() {
            return Err(ErrorCode::AccountDiscriminatorNotFound.into());
        }

        let given_disc = &data[..disc.len()];
        if given_disc != disc {
            return Err(ErrorCode::AccountDiscriminatorMismatch.into());
        }

        Ok(AccountLoader::new_unchecked(acc_info))
    }

    /// Constructs a new [`AccountLoader`] from an uninitialized account.
    #[inline(never)]
    pub fn try_from_unchecked(
        _program_id: &Pubkey,
        acc_info: &'info AccountInfo<'info>,
    ) -> Result<AccountLoader<'info, T>> {
        if acc_info.owner != &T::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
                .with_pubkeys((*acc_info.owner, T::owner())));
        }
        Ok(AccountLoader::new_unchecked(acc_info))
    }

    fn check_size(&self, data: &[u8]) -> Result<()> {
        let required = T::DISCRIMINATOR
            .len()
            .checked_add(mem::size_of::<T>())
            .ok_or(ErrorCode::AccountDidNotDeserialize)?;
        if data.len() < required {
            return Err(ErrorCode::AccountDidNotDeserialize.into());
        }
        Ok(())
    }

    /// Returns a Ref to the account data structure for reading.
    pub fn load(&self) -> Result<Ref<'_, T>> {
        let data = self.acc_info.try_borrow_data()?;
        let disc = T::DISCRIMINATOR;
        if data.len() < disc.len() {
            return Err(ErrorCode::AccountDiscriminatorNotFound.into());
        }

        let given_disc = &data[..disc.len()];
        if given_disc != disc {
            return Err(ErrorCode::AccountDiscriminatorMismatch.into());
        }

        self.check_size(&data)?;

        Ok(Ref::map(data, |data| {
            bytemuck::from_bytes(&data[disc.len()..mem::size_of::<T>() + disc.len()])
        }))
    }

    /// Returns a `RefMut` to the account data structure for reading or writing.
    pub fn load_mut(&self) -> Result<RefMut<'_, T>> {
        // AccountInfo api allows you to borrow mut even if the account isn't
        // writable, so add this check for a better dev experience.
        if !self.acc_info.is_writable {
            return Err(ErrorCode::AccountNotMutable.into());
        }

        let data = self.acc_info.try_borrow_mut_data()?;
        let disc = T::DISCRIMINATOR;
        if data.len() < disc.len() {
            return Err(ErrorCode::AccountDiscriminatorNotFound.into());
        }

        let given_disc = &data[..disc.len()];
        if given_disc != disc {
            return Err(ErrorCode::AccountDiscriminatorMismatch.into());
        }

        self.check_size(&data)?;

        Ok(RefMut::map(data, |data| {
            bytemuck::from_bytes_mut(
                &mut data.deref_mut()[disc.len()..mem::size_of::<T>() + disc.len()],
            )
        }))
    }

    /// Returns a `RefMut` to the account data structure for reading or writing.
    /// Should only be called once, when the account is being initialized.
    pub fn load_init(&self) -> Result<RefMut<'_, T>> {
        // AccountInfo api allows you to borrow mut even if the account isn't
        // writable, so add this check for a better dev experience.
        if !self.acc_info.is_writable {
            return Err(ErrorCode::AccountNotMutable.into());
        }

        let data = self.acc_info.try_borrow_mut_data()?;

        // The discriminator should be zero, since we're initializing.
        let disc = T::DISCRIMINATOR;
        self.check_size(&data)?;

        let given_disc = &data[..disc.len()];
        let has_disc = given_disc.iter().any(|b| *b != 0);
        if has_disc {
            return Err(ErrorCode::AccountDiscriminatorAlreadySet.into());
        }

        Ok(RefMut::map(data, |data| {
            bytemuck::from_bytes_mut(
                &mut data.deref_mut()[disc.len()..mem::size_of::<T>() + disc.len()],
            )
        }))
    }
}

impl<'info, B, T: ZeroCopy + Owner> Accounts<'info, B> for AccountLoader<'info, T> {
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
        let l = AccountLoader::try_from(account)?;
        Ok(l)
    }
}

impl<'info, T: ZeroCopy + Owner> AccountsExit<'info> for AccountLoader<'info, T> {
    // The account *cannot* be loaded when this is called.
    fn exit(&self, program_id: &Pubkey) -> Result<()> {
        // Only persist if the owner is the current program and the account is not closed.
        if &T::owner() == program_id && !crate::common::is_closed(self.acc_info) {
            // Guard against truncation: refuse to rewrite the discriminator over an undersized buffer.
            let required = T::DISCRIMINATOR.len() + mem::size_of::<T>();
            if self.acc_info.try_data_len()? < required {
                return Err(ErrorCode::AccountDidNotDeserialize.into());
            }
            let mut data = self.acc_info.try_borrow_mut_data()?;
            let dst: &mut [u8] = &mut data;
            let mut writer = BpfWriter::new(dst);
            writer.write_all(T::DISCRIMINATOR).unwrap();
        }
        Ok(())
    }
}

impl<'info, T: ZeroCopy + Owner> AccountsClose<'info> for AccountLoader<'info, T> {
    fn close(&self, sol_destination: AccountInfo<'info>) -> Result<()> {
        crate::common::close(self.as_ref(), sol_destination.as_ref())
    }
}

impl<T: ZeroCopy + Owner> ToAccountMetas for AccountLoader<'_, T> {
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let is_signer = is_signer.unwrap_or(self.acc_info.is_signer);
        let meta = match self.acc_info.is_writable {
            false => AccountMeta::new_readonly(*self.acc_info.key, is_signer),
            true => AccountMeta::new(*self.acc_info.key, is_signer),
        };
        vec![meta]
    }
}

impl<'info, T: ZeroCopy + Owner> AsRef<AccountInfo<'info>> for AccountLoader<'info, T> {
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.acc_info
    }
}

impl<'info, T: ZeroCopy + Owner> ToAccountInfos<'info> for AccountLoader<'info, T> {
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.acc_info.clone()]
    }
}

impl<T: ZeroCopy + Owner> Key for AccountLoader<'_, T> {
    fn key(&self) -> Pubkey {
        *self.acc_info.key
    }
}
