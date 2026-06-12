//! Account container for migrating from one account type to another.

use {
    crate::{
        bpf_writer::BpfWriter,
        error::{Error, ErrorCode},
        solana_program::{
            account_info::AccountInfo, instruction::AccountMeta, pubkey::Pubkey, system_program,
        },
        AccountDeserialize, AccountSerialize, Accounts, AccountsExit, Key, Owner, Result,
        ToAccountInfos, ToAccountMetas,
    },
    std::{
        collections::BTreeSet,
        ops::{Deref, DerefMut},
    },
};

/// Internal representation of the migration state.
#[derive(Debug)]
pub enum MigrationInner<From, To> {
    /// Account is in old format, will be migrated and serialized on exit
    From(From),
    /// Account is already in new format, will be serialized on exit
    To(To),
}

/// Wrapper around [`AccountInfo`]
/// that handles account schema migrations from one type to another.
///
/// # Table of Contents
/// - [Basic Functionality](#basic-functionality)
/// - [Usage Patterns](#usage-patterns)
/// - [Example](#example)
///
/// # Basic Functionality
///
/// `Migration` facilitates migrating account data from an old schema (`From`) to a new
/// schema (`To`). During deserialization, the account must be in the `From` format -
/// accounts already in the `To` format will be rejected with an error.
///
/// The migrated data is stored in memory and will be serialized to the account when the
/// instruction exits. On exit, the account must be in the migrated state or an error will
/// be returned.
///
/// This type is typically used with the `realloc` constraint to resize the account
/// during migration.
///
/// Checks:
///
/// - `Account.info.owner == From::owner()`
/// - `!(Account.info.owner == SystemProgram && Account.info.lamports() == 0)`
/// - Account must deserialize as `From` (not `To`)
///
/// # Usage Patterns
///
/// There are multiple ways to work with Migration accounts:
///
/// ## 1. Explicit Migration with `migrate()`
///
/// ```ignore
/// ctx.accounts.my_account.migrate(AccountV2 {
///     data: ctx.accounts.my_account.data,
///     new_field: 42,
/// })?;
/// ```
///
/// ## 2. Direct Field Access via Deref (before migration)
///
/// ```ignore
/// // Access old account fields directly
/// let old_value = ctx.accounts.my_account.data;
/// let old_timestamp = ctx.accounts.my_account.timestamp;
///
/// // Then migrate
/// ctx.accounts.my_account.migrate(AccountV2 { ... })?;
/// ```
///
/// ## 3. Idempotent Migration with `into_inner()`
///
/// ```ignore
/// // Migrates if needed, returns reference to new data
/// // Access old fields directly via deref!
/// let migrated = ctx.accounts.my_account.into_inner(AccountV2 {
///     data: ctx.accounts.my_account.data,
///     new_field: ctx.accounts.my_account.data * 2,
/// })?;
///
/// // Use migrated data (safe to call multiple times!)
/// msg!("New field: {}", migrated.new_field);
/// ```
///
/// ## 4. Idempotent Migration with Mutation via `into_inner_mut()`
///
/// ```ignore
/// // Migrates if needed, returns mutable reference
/// let migrated = ctx.accounts.my_account.into_inner_mut(AccountV2 {
///     data: ctx.accounts.my_account.data,
///     new_field: 0,
/// })?;
///
/// // Mutate the new data
/// migrated.new_field = 42;
/// ```
///
/// # Example
/// ```ignore
/// use anchor_lang::prelude::*;
///
/// declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
///
/// #[program]
/// pub mod my_program {
///     use super::*;
///
///     pub fn migrate(ctx: Context<MigrateAccount>) -> Result<()> {
///         // Use idempotent migration with into_inner
///         let migrated = ctx.accounts.my_account.into_inner(AccountV2 {
///             data: ctx.accounts.my_account.data,
///             new_field: ctx.accounts.my_account.data * 2,
///         })?;
///
///         msg!("Migrated! New field: {}", migrated.new_field);
///         Ok(())
///     }
/// }
///
/// #[account]
/// pub struct AccountV1 {
///     pub data: u64,
/// }
///
/// #[account]
/// pub struct AccountV2 {
///     pub data: u64,
///     pub new_field: u64,
/// }
///
/// #[derive(Accounts)]
/// pub struct MigrateAccount<'info> {
///     #[account(mut)]
///     pub payer: Signer<'info>,
///     #[account(
///         mut,
///         realloc = 8 + AccountV2::INIT_SPACE,
///         realloc::payer = payer,
///         realloc::zero = false
///     )]
///     pub my_account: Migration<'info, AccountV1, AccountV2>,
///     pub system_program: Program<'info, System>,
/// }
/// ```
#[derive(Debug)]
pub struct Migration<'info, From, To>
where
    From: AccountDeserialize,
    To: AccountSerialize,
{
    /// Account info reference
    info: &'info AccountInfo<'info>,
    /// Internal migration state
    inner: MigrationInner<From, To>,
}

impl<'info, From, To> Migration<'info, From, To>
where
    From: AccountDeserialize + Owner,
    To: AccountSerialize + Owner,
{
    /// Creates a new Migration in the From (unmigrated) state.
    fn new(info: &'info AccountInfo<'info>, account: From) -> Self {
        Self {
            info,
            inner: MigrationInner::From(account),
        }
    }

    /// Returns `true` if the account has been migrated.
    #[inline(always)]
    pub fn is_migrated(&self) -> bool {
        matches!(self.inner, MigrationInner::To(_))
    }

    /// Returns a reference to the old account data if not yet migrated.
    ///
    /// # Errors
    /// Returns an error if the account has already been migrated.
    pub fn try_as_from(&self) -> Result<&From> {
        match &self.inner {
            MigrationInner::From(from) => Ok(from),
            MigrationInner::To(_) => Err(ErrorCode::AccountAlreadyMigrated.into()),
        }
    }

    /// Returns a mutable reference to the old account data if not yet migrated.
    ///
    /// # Errors
    /// Returns an error if the account has already been migrated.
    pub fn try_as_from_mut(&mut self) -> Result<&mut From> {
        match &mut self.inner {
            MigrationInner::From(from) => Ok(from),
            MigrationInner::To(_) => Err(ErrorCode::AccountAlreadyMigrated.into()),
        }
    }

    /// Migrates the account by providing the new data.
    ///
    /// This method stores the new data in memory. The data will be
    /// serialized to the account when the instruction exits.
    ///
    /// # Errors
    /// Returns an error if the account has already been migrated.
    pub fn migrate(&mut self, new_data: To) -> Result<()> {
        if self.is_migrated() {
            return Err(ErrorCode::AccountAlreadyMigrated.into());
        }

        self.inner = MigrationInner::To(new_data);
        Ok(())
    }

    /// Gets a reference to the migrated value, or migrates it with the provided data.
    ///
    /// This method provides flexible access to the migrated state:
    /// - If already migrated, returns a reference to the existing value
    /// - If not migrated, migrates with the provided data, then returns a reference
    ///
    /// # Arguments
    /// * `new_data` - The new `To` value to migrate to (only used if not yet migrated)
    ///
    /// # Example
    /// ```ignore
    /// pub fn process(ctx: Context<MyInstruction>) -> Result<()> {
    ///     // Migrate and get reference in one call
    ///     // Access old fields directly via deref!
    ///     let migrated = ctx.accounts.my_account.into_inner(AccountV2 {
    ///         data: ctx.accounts.my_account.data,
    ///         new_field: 42,
    ///     })?;
    ///
    ///     // Use migrated...
    ///     msg!("Migrated data: {}", migrated.data);
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn into_inner(&mut self, new_data: To) -> &To {
        if !self.is_migrated() {
            self.inner = MigrationInner::To(new_data);
        }

        match &self.inner {
            MigrationInner::To(to) => to,
            _ => unreachable!(),
        }
    }

    /// Gets a mutable reference to the migrated value, or migrates it with the provided data.
    ///
    /// This method provides flexible mutable access to the migrated state:
    /// - If already migrated, returns a mutable reference to the existing value
    /// - If not migrated, migrates with the provided data, then returns a mutable reference
    ///
    /// # Arguments
    /// * `new_data` - The new `To` value to migrate to (only used if not yet migrated)
    ///
    /// # Example
    /// ```ignore
    /// pub fn process(ctx: Context<MyInstruction>) -> Result<()> {
    ///     // Migrate and get mutable reference in one call
    ///     // Access old fields directly via deref!
    ///     let migrated = ctx.accounts.my_account.into_inner_mut(AccountV2 {
    ///         data: ctx.accounts.my_account.data,
    ///         new_field: 0,
    ///     })?;
    ///
    ///     // Mutate the migrated value
    ///     migrated.new_field = 42;
    ///
    ///     Ok(())
    /// }
    /// ```
    pub fn into_inner_mut(&mut self, new_data: To) -> &mut To {
        if !self.is_migrated() {
            self.inner = MigrationInner::To(new_data);
        }

        match &mut self.inner {
            MigrationInner::To(to) => to,
            _ => unreachable!(),
        }
    }

    /// Deserializes the given `info` into a `Migration`.
    ///
    /// Only accepts accounts in the `From` format. Accounts already in the `To`
    /// format will be rejected.
    #[inline(never)]
    pub fn try_from(info: &'info AccountInfo<'info>) -> Result<Self> {
        if info.owner == &system_program::ID && info.lamports() == 0 {
            return Err(ErrorCode::AccountNotInitialized.into());
        }

        if info.owner != &From::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
                .with_pubkeys((*info.owner, From::owner())));
        }

        let mut data: &[u8] = &info.try_borrow_data()?;
        Ok(Self::new(info, From::try_deserialize(&mut data)?))
    }

    /// Deserializes the given `info` into a `Migration` without checking
    /// the account discriminator.
    ///
    /// **Warning:** Use with caution. This skips discriminator validation.
    #[inline(never)]
    pub fn try_from_unchecked(info: &'info AccountInfo<'info>) -> Result<Self> {
        if info.owner == &system_program::ID && info.lamports() == 0 {
            return Err(ErrorCode::AccountNotInitialized.into());
        }

        if info.owner != &From::owner() {
            return Err(Error::from(ErrorCode::AccountOwnedByWrongProgram)
                .with_pubkeys((*info.owner, From::owner())));
        }

        let mut data: &[u8] = &info.try_borrow_data()?;
        Ok(Self::new(info, From::try_deserialize_unchecked(&mut data)?))
    }
}

impl<'info, B, From, To> Accounts<'info, B> for Migration<'info, From, To>
where
    From: AccountDeserialize + Owner,
    To: AccountSerialize + Owner,
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

impl<'info, From, To> AccountsExit<'info> for Migration<'info, From, To>
where
    From: AccountDeserialize + Owner,
    To: AccountSerialize + Owner,
{
    fn exit(&self, program_id: &Pubkey) -> Result<()> {
        // Check if account is closed
        if crate::common::is_closed(self.info) {
            return Ok(());
        }

        // Check that the account has been migrated and serialize
        match &self.inner {
            MigrationInner::From(_) => {
                // Account was not migrated - this is an error
                return Err(ErrorCode::AccountNotMigrated.into());
            }
            MigrationInner::To(to) => {
                // Only persist if the owner is the current program
                let expected_owner = To::owner();
                if &expected_owner != program_id {
                    return Err(Error::from(ErrorCode::InvalidProgramId)
                        .with_pubkeys((*program_id, expected_owner)));
                }

                // Serialize the migrated data
                let mut data = self.info.try_borrow_mut_data()?;
                let dst: &mut [u8] = &mut data;
                let mut writer = BpfWriter::new(dst);
                to.try_serialize(&mut writer)?;
            }
        }

        Ok(())
    }
}

impl<From, To> ToAccountMetas for Migration<'_, From, To>
where
    From: AccountDeserialize,
    To: AccountSerialize,
{
    fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<AccountMeta> {
        let is_signer = is_signer.unwrap_or(self.info.is_signer);
        let meta = match self.info.is_writable {
            false => AccountMeta::new_readonly(*self.info.key, is_signer),
            true => AccountMeta::new(*self.info.key, is_signer),
        };
        vec![meta]
    }
}

impl<'info, From, To> ToAccountInfos<'info> for Migration<'info, From, To>
where
    From: AccountDeserialize,
    To: AccountSerialize,
{
    fn to_account_infos(&self) -> Vec<AccountInfo<'info>> {
        vec![self.info.clone()]
    }
}

impl<'info, From, To> AsRef<AccountInfo<'info>> for Migration<'info, From, To>
where
    From: AccountDeserialize,
    To: AccountSerialize,
{
    fn as_ref(&self) -> &AccountInfo<'info> {
        self.info
    }
}

impl<From, To> Key for Migration<'_, From, To>
where
    From: AccountDeserialize,
    To: AccountSerialize,
{
    fn key(&self) -> Pubkey {
        *self.info.key
    }
}

// Deref to From when account is in Old state
impl<'info, From, To> Deref for Migration<'info, From, To>
where
    From: AccountDeserialize,
    To: AccountSerialize,
{
    type Target = From;

    fn deref(&self) -> &Self::Target {
        match &self.inner {
            MigrationInner::From(from) => from,
            MigrationInner::To(_) => {
                crate::solana_program::msg!("Cannot deref to From: account is already migrated.");
                panic!();
            }
        }
    }
}

// DerefMut to From when account is in Old state
impl<'info, From, To> DerefMut for Migration<'info, From, To>
where
    From: AccountDeserialize,
    To: AccountSerialize,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        match &mut self.inner {
            MigrationInner::From(from) => from,
            MigrationInner::To(_) => {
                crate::solana_program::msg!(
                    "Cannot deref_mut to From: account is already migrated."
                );
                panic!();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{AnchorDeserialize, AnchorSerialize, Discriminator},
    };

    const TEST_DISCRIMINATOR_V1: [u8; 8] = [1, 2, 3, 4, 5, 6, 7, 8];
    const TEST_DISCRIMINATOR_V2: [u8; 8] = [8, 7, 6, 5, 4, 3, 2, 1];
    const TEST_OWNER: Pubkey = Pubkey::new_from_array([1u8; 32]);
    const TEST_OTHER_OWNER: Pubkey = Pubkey::new_from_array([2u8; 32]);

    #[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize, PartialEq)]
    struct AccountV1 {
        pub data: u64,
    }

    impl Discriminator for AccountV1 {
        const DISCRIMINATOR: &'static [u8] = &TEST_DISCRIMINATOR_V1;
    }

    impl Owner for AccountV1 {
        fn owner() -> Pubkey {
            TEST_OWNER
        }
    }

    impl AccountSerialize for AccountV1 {
        fn try_serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
            writer.write_all(&TEST_DISCRIMINATOR_V1)?;
            AnchorSerialize::serialize(self, writer)?;
            Ok(())
        }
    }

    impl AccountDeserialize for AccountV1 {
        fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
            if buf.len() < 8 {
                return Err(ErrorCode::AccountDiscriminatorNotFound.into());
            }
            let disc = &buf[..8];
            if disc != TEST_DISCRIMINATOR_V1 {
                return Err(ErrorCode::AccountDiscriminatorMismatch.into());
            }
            Self::try_deserialize_unchecked(buf)
        }

        fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
            let mut data = &buf[8..];
            AnchorDeserialize::deserialize(&mut data)
                .map_err(|_| ErrorCode::AccountDidNotDeserialize.into())
        }
    }

    #[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize, PartialEq)]
    struct AccountV2 {
        pub data: u64,
        pub new_field: u64,
    }

    impl Discriminator for AccountV2 {
        const DISCRIMINATOR: &'static [u8] = &TEST_DISCRIMINATOR_V2;
    }

    impl Owner for AccountV2 {
        fn owner() -> Pubkey {
            TEST_OWNER
        }
    }

    impl AccountSerialize for AccountV2 {
        fn try_serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
            writer.write_all(&TEST_DISCRIMINATOR_V2)?;
            AnchorSerialize::serialize(self, writer)?;
            Ok(())
        }
    }

    #[derive(Debug, Clone, AnchorSerialize, AnchorDeserialize, PartialEq)]
    struct AccountV2OtherOwner {
        pub data: u64,
        pub new_field: u64,
    }

    impl Owner for AccountV2OtherOwner {
        fn owner() -> Pubkey {
            TEST_OTHER_OWNER
        }
    }

    impl AccountSerialize for AccountV2OtherOwner {
        fn try_serialize<W: std::io::Write>(&self, writer: &mut W) -> Result<()> {
            writer.write_all(&TEST_DISCRIMINATOR_V2)?;
            AnchorSerialize::serialize(self, writer)?;
            Ok(())
        }
    }

    impl AccountDeserialize for AccountV2 {
        fn try_deserialize(buf: &mut &[u8]) -> Result<Self> {
            if buf.len() < 8 {
                return Err(ErrorCode::AccountDiscriminatorNotFound.into());
            }
            let disc = &buf[..8];
            if disc != TEST_DISCRIMINATOR_V2 {
                return Err(ErrorCode::AccountDiscriminatorMismatch.into());
            }
            Self::try_deserialize_unchecked(buf)
        }

        fn try_deserialize_unchecked(buf: &mut &[u8]) -> Result<Self> {
            let mut data = &buf[8..];
            AnchorDeserialize::deserialize(&mut data)
                .map_err(|_| ErrorCode::AccountDidNotDeserialize.into())
        }
    }

    fn create_account_info<'a>(
        key: &'a Pubkey,
        owner: &'a Pubkey,
        lamports: &'a mut u64,
        data: &'a mut [u8],
    ) -> AccountInfo<'a> {
        AccountInfo::new(key, false, true, lamports, data, owner, false)
    }

    // Verifies that a freshly deserialized Migration account reports
    // is_migrated() as false, since it starts in the From state.
    #[test]
    fn test_is_migrated_returns_false_initially() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let migration: Migration<AccountV1, AccountV2> = Migration::try_from(&info).unwrap();

        assert!(!migration.is_migrated());
    }

    // Verifies that after calling migrate(), the account correctly
    // reports is_migrated() as true.
    #[test]
    fn test_is_migrated_returns_true_after_migrate() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let mut migration: Migration<AccountV1, AccountV2> = Migration::try_from(&info).unwrap();

        migration
            .migrate(AccountV2 {
                data: 42,
                new_field: 100,
            })
            .unwrap();

        assert!(migration.is_migrated());
    }

    // Verifies that try_as_from() successfully returns a reference to the
    // old account data before migration has occurred.
    #[test]
    fn test_try_as_from_returns_data_before_migration() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let migration: Migration<AccountV1, AccountV2> = Migration::try_from(&info).unwrap();

        let from = migration.try_as_from().unwrap();
        assert_eq!(from.data, 42);
    }

    // Verifies that try_as_from() returns an error after migration,
    // providing a safe alternative to Deref that won't panic.
    #[test]
    fn test_try_as_from_returns_error_after_migration() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let mut migration: Migration<AccountV1, AccountV2> = Migration::try_from(&info).unwrap();

        migration
            .migrate(AccountV2 {
                data: 42,
                new_field: 100,
            })
            .unwrap();

        assert!(migration.try_as_from().is_err());
    }

    // Verifies that try_as_from_mut() allows mutable access to the old
    // account data before migration, and changes are persisted.
    #[test]
    fn test_try_as_from_mut_works_before_migration() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let mut migration: Migration<AccountV1, AccountV2> = Migration::try_from(&info).unwrap();

        let from = migration.try_as_from_mut().unwrap();
        from.data = 100;
        assert_eq!(migration.try_as_from().unwrap().data, 100);
    }

    // Verifies that calling migrate() twice returns an error,
    // preventing accidental double-migration.
    #[test]
    fn test_migrate_fails_if_already_migrated() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let mut migration: Migration<AccountV1, AccountV2> = Migration::try_from(&info).unwrap();

        migration
            .migrate(AccountV2 {
                data: 42,
                new_field: 100,
            })
            .unwrap();
        let result = migration.migrate(AccountV2 {
            data: 42,
            new_field: 200,
        });

        assert!(result.is_err());
    }

    // Verifies that into_inner() performs migration and returns a
    // reference to the new account data in a single call.
    #[test]
    fn test_into_inner_migrates_and_returns_reference() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let mut migration: Migration<AccountV1, AccountV2> = Migration::try_from(&info).unwrap();

        let to = migration.into_inner(AccountV2 {
            data: 42,
            new_field: 100,
        });

        assert_eq!(to.data, 42);
        assert_eq!(to.new_field, 100);
        assert!(migration.is_migrated());
    }

    // Verifies that into_inner() is idempotent - calling it multiple times
    // returns the existing migrated data and ignores subsequent new_data arguments.
    #[test]
    fn test_into_inner_is_idempotent() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let mut migration: Migration<AccountV1, AccountV2> = Migration::try_from(&info).unwrap();

        let to1 = migration.into_inner(AccountV2 {
            data: 42,
            new_field: 100,
        });
        assert_eq!(to1.new_field, 100);

        // Second call should return existing value, not use the new data
        let to2 = migration.into_inner(AccountV2 {
            data: 42,
            new_field: 999,
        });
        assert_eq!(to2.new_field, 100); // Still 100, not 999
    }

    // Verifies that into_inner_mut() returns a mutable reference,
    // allowing modification of the migrated account data.
    #[test]
    fn test_into_inner_mut_allows_mutation() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let mut migration: Migration<AccountV1, AccountV2> = Migration::try_from(&info).unwrap();

        let to = migration.into_inner_mut(AccountV2 {
            data: 42,
            new_field: 100,
        });
        to.new_field = 200;

        let to_ref = migration.into_inner(AccountV2 {
            data: 0,
            new_field: 0,
        });
        assert_eq!(to_ref.new_field, 200);
    }

    // Verifies that Deref allows direct field access (e.g., account.data)
    // before migration has occurred.
    #[test]
    fn test_deref_works_before_migration() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let migration: Migration<AccountV1, AccountV2> = Migration::try_from(&info).unwrap();

        assert_eq!(migration.data, 42);
    }

    // Verifies that Deref panics after migration. This documents the current
    // behavior - use try_as_from() for safe access that returns Result instead.
    #[test]
    #[should_panic]
    fn test_deref_panics_after_migration() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let mut migration: Migration<AccountV1, AccountV2> = Migration::try_from(&info).unwrap();

        migration
            .migrate(AccountV2 {
                data: 42,
                new_field: 100,
            })
            .unwrap();

        // This should panic
        let _ = migration.data;
    }

    // Verifies that deserialization fails when the account owner doesn't
    // match the expected program, preventing unauthorized access.
    #[test]
    fn test_try_from_fails_with_wrong_owner() {
        let key = Pubkey::default();
        let wrong_owner = Pubkey::new_from_array([99u8; 32]);
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &wrong_owner, &mut lamports, &mut data);
        let result: Result<Migration<AccountV1, AccountV2>> = Migration::try_from(&info);

        assert!(result.is_err());
    }

    // Verifies that deserialization fails for uninitialized accounts
    // (owned by system program with zero lamports).
    #[test]
    fn test_try_from_fails_with_uninitialized_account() {
        let key = Pubkey::default();
        let mut lamports = 0;
        let mut data = vec![0u8; 100];

        let info = create_account_info(&key, &system_program::ID, &mut lamports, &mut data);
        let result: Result<Migration<AccountV1, AccountV2>> = Migration::try_from(&info);

        assert!(result.is_err());
    }

    #[test]
    fn test_exit_fails_when_migrated_owner_does_not_match_program_id() {
        let key = Pubkey::default();
        let mut lamports = 100;
        let v1 = AccountV1 { data: 42 };
        let mut data = vec![0u8; 100];
        data[..8].copy_from_slice(&TEST_DISCRIMINATOR_V1);
        v1.serialize(&mut &mut data[8..]).unwrap();

        let info = create_account_info(&key, &TEST_OWNER, &mut lamports, &mut data);
        let mut migration: Migration<AccountV1, AccountV2OtherOwner> =
            Migration::try_from(&info).unwrap();

        migration
            .migrate(AccountV2OtherOwner {
                data: 42,
                new_field: 100,
            })
            .unwrap();

        let err = migration
            .exit(&TEST_OWNER)
            .expect_err("exit must fail when migrated data cannot be persisted");
        match err {
            Error::AnchorError(e) => {
                assert_eq!(e.error_code_number, ErrorCode::InvalidProgramId as u32);
            }
            other => panic!("unexpected error variant: {other:?}"),
        }

        let mut persisted_data: &[u8] = &info.try_borrow_data().unwrap();
        let persisted = AccountV1::try_deserialize(&mut persisted_data).unwrap();
        assert_eq!(persisted.data, 42);
    }
}
