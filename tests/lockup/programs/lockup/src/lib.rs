//! A relatively advanced example of a lockup program. If you're new to Anchor,
//! it's suggested to start with the other examples.

use anchor_lang::accounts::state::ProgramState;
use anchor_lang::prelude::*;
use anchor_lang::solana_program;
use anchor_lang::solana_program::instruction::Instruction;
use anchor_spl::token::{self, TokenAccount, Transfer};

mod calculator;

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

#[program]
pub mod lockup {
    use super::*;

    #[state]
    pub struct Lockup {
        /// The key with the ability to change the whitelist.
        pub authority: Pubkey,
        /// List of programs locked tokens can be sent to. These programs
        /// are completely trusted to maintain the locked property.
        pub whitelist: Vec<WhitelistEntry>,
    }

    impl Lockup {
        pub const WHITELIST_SIZE: usize = 10;

        pub fn new(ctx: Context<Auth>) -> Result<Self> {
            let mut whitelist = vec![];
            whitelist.resize(Self::WHITELIST_SIZE, Default::default());
            Ok(Lockup {
                authority: *ctx.accounts.authority.key,
                whitelist,
            })
        }

        #[access_control(whitelist_auth(self, &ctx))]
        pub fn whitelist_add(&mut self, ctx: Context<Auth>, entry: WhitelistEntry) -> Result<()> {
            if self.whitelist.len() == Self::WHITELIST_SIZE {
                return err!(ErrorCode::WhitelistFull);
            }
            if self.whitelist.contains(&entry) {
                return err!(ErrorCode::WhitelistEntryAlreadyExists);
            }
            self.whitelist.push(entry);
            Ok(())
        }

        #[access_control(whitelist_auth(self, &ctx))]
        pub fn whitelist_delete(
            &mut self,
            ctx: Context<Auth>,
            entry: WhitelistEntry,
        ) -> Result<()> {
            if !self.whitelist.contains(&entry) {
                return err!(ErrorCode::WhitelistEntryNotFound);
            }
            self.whitelist.retain(|e| e != &entry);
            Ok(())
        }

        #[access_control(whitelist_auth(self, &ctx))]
        pub fn set_authority(&mut self, ctx: Context<Auth>, new_authority: Pubkey) -> Result<()> {
            self.authority = new_authority;
            Ok(())
        }
    }

    #[access_control(CreateVesting::accounts(&ctx, nonce))]
    pub fn create_vesting(
        ctx: Context<CreateVesting>,
        beneficiary: Pubkey,
        deposit_amount: u64,
        nonce: u8,
        start_ts: i64,
        end_ts: i64,
        period_count: u64,
        realizor: Option<Realizor>,
    ) -> Result<()> {
        if deposit_amount == 0 {
            return err!(ErrorCode::InvalidDepositAmount);
        }
        if !is_valid_schedule(start_ts, end_ts, period_count) {
            return err!(ErrorCode::InvalidSchedule);
        }
        let vesting = &mut ctx.accounts.vesting;
        vesting.beneficiary = beneficiary;
        vesting.mint = ctx.accounts.vault.mint;
        vesting.vault = *ctx.accounts.vault.to_account_info().key;
        vesting.period_count = period_count;
        vesting.start_balance = deposit_amount;
        vesting.end_ts = end_ts;
        vesting.start_ts = start_ts;
        vesting.created_ts = ctx.accounts.clock.unix_timestamp;
        vesting.outstanding = deposit_amount;
        vesting.whitelist_owned = 0;
        vesting.grantor = *ctx.accounts.depositor_authority.key;
        vesting.nonce = nonce;
        vesting.realizor = realizor;

        token::transfer(ctx.accounts.into(), deposit_amount)?;

        Ok(())
    }

    #[access_control(is_realized(&ctx))]
    pub fn withdraw(ctx: Context<Withdraw>, amount: u64) -> Result<()> {
        // Has the given amount vested?
        if amount
            > calculator::available_for_withdrawal(
                &ctx.accounts.vesting,
                ctx.accounts.clock.unix_timestamp,
            )
        {
            return err!(ErrorCode::InsufficientWithdrawalBalance);
        }

        // Transfer funds out.
        let seeds = &[
            ctx.accounts.vesting.to_account_info().key.as_ref(),
            &[ctx.accounts.vesting.nonce],
        ];
        let signer = &[&seeds[..]];
        let cpi_ctx = CpiContext::from(&*ctx.accounts).with_signer(signer);
        token::transfer(cpi_ctx, amount)?;

        // Bookkeeping.
        let vesting = &mut ctx.accounts.vesting;
        vesting.outstanding -= amount;

        Ok(())
    }

    // Sends funds from the lockup program to a whitelisted program.
    pub fn whitelist_withdraw(
        ctx: Context<WhitelistWithdraw>,
        instruction_data: Vec<u8>,
        amount: u64,
    ) -> Result<()> {
        let before_amount = ctx.accounts.transfer.vault.amount;
        whitelist_relay_cpi(
            &ctx.accounts.transfer,
            ctx.remaining_accounts,
            instruction_data,
        )?;
        ctx.accounts.transfer.vault.reload()?;
        let after_amount = ctx.accounts.transfer.vault.amount;

        // CPI safety checks.
        let withdraw_amount = before_amount - after_amount;
        if withdraw_amount > amount {
            return err!(ErrorCode::WhitelistWithdrawLimit);
        }

        // Bookkeeping.
        ctx.accounts.transfer.vesting.whitelist_owned += withdraw_amount;

        Ok(())
    }

    // Sends funds from a whitelisted program back to the lockup program.
    pub fn whitelist_deposit(
        ctx: Context<WhitelistDeposit>,
        instruction_data: Vec<u8>,
    ) -> Result<()> {
        let before_amount = ctx.accounts.transfer.vault.amount;
        whitelist_relay_cpi(
            &ctx.accounts.transfer,
            ctx.remaining_accounts,
            instruction_data,
        )?;
        ctx.accounts.transfer.vault.reload()?;
        let after_amount = ctx.accounts.transfer.vault.amount;

        // CPI safety checks.
        let deposit_amount = after_amount - before_amount;
        if deposit_amount <= 0 {
            return err!(ErrorCode::InsufficientWhitelistDepositAmount);
        }
        if deposit_amount > ctx.accounts.transfer.vesting.whitelist_owned {
            return err!(ErrorCode::WhitelistDepositOverflow)?;
        }

        // Bookkeeping.
        ctx.accounts.transfer.vesting.whitelist_owned -= deposit_amount;

        Ok(())
    }

    // Convenience function for UI's to calculate the withdrawable amount.
    pub fn available_for_withdrawal(ctx: Context<AvailableForWithdrawal>) -> Result<()> {
        let available = calculator::available_for_withdrawal(
            &ctx.accounts.vesting,
            ctx.accounts.clock.unix_timestamp,
        );
        // Log as string so that JS can read as a BN.
        msg!(&format!("{{ \"result\": \"{}\" }}", available));
        Ok(())
    }
}

#[derive(Accounts)]
pub struct Auth<'info> {
    #[account(signer)]
    authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct CreateVesting<'info> {
    // Vesting.
    #[account(zero)]
    pub vesting: Account<'info, Vesting>,
    #[account(mut)]
    pub vault: Account<'info, TokenAccount>,
    // Depositor.
    #[account(mut)]
    pub depositor: UncheckedAccount<'info>,
    #[account(signer)]
    pub depositor_authority: UncheckedAccount<'info>,
    // Misc.
    #[account(constraint = token_program.key == &token::ID)]
    pub token_program: UncheckedAccount<'info>,
    pub clock: Sysvar<'info, Clock>,
}

impl<'info> CreateVesting<'info> {
    fn accounts(ctx: &Context<CreateVesting>, nonce: u8) -> Result<()> {
        let vault_authority = Pubkey::create_program_address(
            &[
                ctx.accounts.vesting.to_account_info().key.as_ref(),
                &[nonce],
            ],
            ctx.program_id,
        )
        .map_err(|_| error!(ErrorCode::InvalidProgramAddress))?;
        if ctx.accounts.vault.owner != vault_authority {
            return err!(ErrorCode::InvalidVaultOwner)?;
        }

        Ok(())
    }
}

// All accounts not included here, i.e., the "remaining accounts" should be
// ordered according to the realization interface.
#[derive(Accounts)]
pub struct Withdraw<'info> {
    // Vesting.
    #[account(mut, has_one = beneficiary, has_one = vault)]
    vesting: Account<'info, Vesting>,
    beneficiary: Signer<'info>,
    #[account(mut)]
    vault: Account<'info, TokenAccount>,
    #[account(
        seeds = [vesting.to_account_info().key.as_ref()],
        bump = vesting.nonce,
    )]
    vesting_signer: UncheckedAccount<'info>,
    // Withdraw receiving target..
    #[account(mut)]
    token: Account<'info, TokenAccount>,
    // Misc.
    #[account(constraint = token_program.key == &token::ID)]
    token_program: UncheckedAccount<'info>,
    clock: Sysvar<'info, Clock>,
}

#[derive(Accounts)]
pub struct WhitelistWithdraw<'info> {
    transfer: WhitelistTransfer<'info>,
}

#[derive(Accounts)]
pub struct WhitelistDeposit<'info> {
    transfer: WhitelistTransfer<'info>,
}

#[derive(Accounts)]
pub struct WhitelistTransfer<'info> {
    lockup: ProgramState<'info, Lockup>,
    beneficiary: Signer<'info>,
    whitelisted_program: UncheckedAccount<'info>,

    // Whitelist interface.
    #[account(mut, has_one = beneficiary, has_one = vault)]
    vesting: Account<'info, Vesting>,
    #[account(mut, constraint = &vault.owner == vesting_signer.key)]
    vault: Account<'info, TokenAccount>,
    #[account(
        seeds = [vesting.to_account_info().key.as_ref()],
        bump = vesting.nonce,
    )]
    vesting_signer: UncheckedAccount<'info>,
    #[account("token_program.key == &token::ID")]
    token_program: UncheckedAccount<'info>,
    #[account(mut)]
    whitelisted_program_vault: UncheckedAccount<'info>,
    whitelisted_program_vault_authority: UncheckedAccount<'info>,
}

#[derive(Accounts)]
pub struct AvailableForWithdrawal<'info> {
    vesting: Account<'info, Vesting>,
    clock: Sysvar<'info, Clock>,
}

#[account]
pub struct Vesting {
    /// The owner of this Vesting account.
    pub beneficiary: Pubkey,
    /// The mint of the SPL token locked up.
    pub mint: Pubkey,
    /// Address of the account's token vault.
    pub vault: Pubkey,
    /// The owner of the token account funding this account.
    pub grantor: Pubkey,
    /// The outstanding SRM deposit backing this vesting account. All
    /// withdrawals will deduct this balance.
    pub outstanding: u64,
    /// The starting balance of this vesting account, i.e., how much was
    /// originally deposited.
    pub start_balance: u64,
    /// The unix timestamp at which this vesting account was created.
    pub created_ts: i64,
    /// The time at which vesting begins.
    pub start_ts: i64,
    /// The time at which all tokens are vested.
    pub end_ts: i64,
    /// The number of times vesting will occur. For example, if vesting
    /// is once a year over seven years, this will be 7.
    pub period_count: u64,
    /// The amount of tokens in custody of whitelisted programs.
    pub whitelist_owned: u64,
    /// Signer nonce.
    pub nonce: u8,
    /// The program that determines when the locked account is **realized**.
    /// In addition to the lockup schedule, the program provides the ability
    /// for applications to determine when locked tokens are considered earned.
    /// For example, when earning locked tokens via the staking program, one
    /// cannot receive the tokens until unstaking. As a result, if one never
    /// unstakes, one would never actually receive the locked tokens.
    pub realizor: Option<Realizor>,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Debug)]
pub struct Realizor {
    /// Program to invoke to check a realization condition. This program must
    /// implement the `RealizeLock` trait.
    pub program: Pubkey,
    /// Address of an arbitrary piece of metadata interpretable by the realizor
    /// program. For example, when a vesting account is allocated, the program
    /// can define its realization condition as a function of some account
    /// state. The metadata is the address of that account.
    ///
    /// In the case of staking, the metadata is a `Member` account address. When
    /// the realization condition is checked, the staking program will check the
    /// `Member` account defined by the `metadata` has no staked tokens.
    pub metadata: Pubkey,
}

#[derive(AnchorSerialize, AnchorDeserialize, PartialEq, Default, Copy, Clone)]
pub struct WhitelistEntry {
    pub program_id: Pubkey,
}

#[error_code]
pub enum ErrorCode {
    #[msg("Vesting end must be greater than the current unix timestamp.")]
    InvalidTimestamp,
    #[msg("The number of vesting periods must be greater than zero.")]
    InvalidPeriod,
    #[msg("The vesting deposit amount must be greater than zero.")]
    InvalidDepositAmount,
    #[msg("The Whitelist entry is not a valid program address.")]
    InvalidWhitelistEntry,
    #[msg("Invalid program address. Did you provide the correct nonce?")]
    InvalidProgramAddress,
    #[msg("Invalid vault owner.")]
    InvalidVaultOwner,
    #[msg("Vault amount must be zero.")]
    InvalidVaultAmount,
    #[msg("Insufficient withdrawal balance.")]
    InsufficientWithdrawalBalance,
    #[msg("Whitelist is full")]
    WhitelistFull,
    #[msg("Whitelist entry already exists")]
    WhitelistEntryAlreadyExists,
    #[msg("Balance must go up when performing a whitelist deposit")]
    InsufficientWhitelistDepositAmount,
    #[msg("Cannot deposit more than withdrawn")]
    WhitelistDepositOverflow,
    #[msg("Tried to withdraw over the specified limit")]
    WhitelistWithdrawLimit,
    #[msg("Whitelist entry not found.")]
    WhitelistEntryNotFound,
    #[msg("You do not have sufficient permissions to perform this action.")]
    Unauthorized,
    #[msg("You are unable to realize projected rewards until unstaking.")]
    UnableToWithdrawWhileStaked,
    #[msg("The given lock realizor doesn't match the vesting account.")]
    InvalidLockRealizor,
    #[msg("You have not realized this vesting account.")]
    UnrealizedVesting,
    #[msg("Invalid vesting schedule given.")]
    InvalidSchedule,
}

impl<'a, 'b, 'c, 'info> From<&mut CreateVesting<'info>>
    for CpiContext<'a, 'b, 'c, 'info, Transfer<'info>>
{
    fn from(accounts: &mut CreateVesting<'info>) -> CpiContext<'a, 'b, 'c, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: accounts.depositor.clone(),
            to: accounts.vault.to_account_info(),
            authority: accounts.depositor_authority.clone(),
        };
        let cpi_program = accounts.token_program.clone();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

impl<'a, 'b, 'c, 'info> From<&Withdraw<'info>> for CpiContext<'a, 'b, 'c, 'info, Transfer<'info>> {
    fn from(accounts: &Withdraw<'info>) -> CpiContext<'a, 'b, 'c, 'info, Transfer<'info>> {
        let cpi_accounts = Transfer {
            from: accounts.vault.to_account_info(),
            to: accounts.token.to_account_info(),
            authority: accounts.vesting_signer.to_account_info(),
        };
        let cpi_program = accounts.token_program.to_account_info();
        CpiContext::new(cpi_program, cpi_accounts)
    }
}

#[access_control(is_whitelisted(transfer))]
pub fn whitelist_relay_cpi<'info>(
    transfer: &WhitelistTransfer<'info>,
    remaining_accounts: &[AccountInfo<'info>],
    instruction_data: Vec<u8>,
) -> Result<()> {
    let mut meta_accounts = vec![
        AccountMeta::new_readonly(*transfer.vesting.to_account_info().key, false),
        AccountMeta::new(*transfer.vault.to_account_info().key, false),
        AccountMeta::new_readonly(*transfer.vesting_signer.to_account_info().key, true),
        AccountMeta::new_readonly(*transfer.token_program.to_account_info().key, false),
        AccountMeta::new(
            *transfer.whitelisted_program_vault.to_account_info().key,
            false,
        ),
        AccountMeta::new_readonly(
            *transfer
                .whitelisted_program_vault_authority
                .to_account_info()
                .key,
            false,
        ),
    ];
    meta_accounts.extend(remaining_accounts.iter().map(|a| {
        if a.is_writable {
            AccountMeta::new(*a.key, a.is_signer)
        } else {
            AccountMeta::new_readonly(*a.key, a.is_signer)
        }
    }));
    let relay_instruction = Instruction {
        program_id: *transfer.whitelisted_program.to_account_info().key,
        accounts: meta_accounts,
        data: instruction_data.to_vec(),
    };

    let seeds = &[
        transfer.vesting.to_account_info().key.as_ref(),
        &[transfer.vesting.nonce],
    ];
    let signer = &[&seeds[..]];
    let mut accounts = transfer.to_account_infos();
    accounts.extend_from_slice(&remaining_accounts);
    solana_program::program::invoke_signed(&relay_instruction, &accounts, signer)
        .map_err(Into::into)
}

pub fn is_whitelisted<'info>(transfer: &WhitelistTransfer<'info>) -> Result<()> {
    if !transfer.lockup.whitelist.contains(&WhitelistEntry {
        program_id: *transfer.whitelisted_program.key,
    }) {
        return err!(ErrorCode::WhitelistEntryNotFound);
    }
    Ok(())
}

fn whitelist_auth(lockup: &Lockup, ctx: &Context<Auth>) -> Result<()> {
    if &lockup.authority != ctx.accounts.authority.key {
        return err!(ErrorCode::Unauthorized);
    }
    Ok(())
}

pub fn is_valid_schedule(start_ts: i64, end_ts: i64, period_count: u64) -> bool {
    if end_ts <= start_ts {
        return false;
    }
    if period_count > (end_ts - start_ts) as u64 {
        return false;
    }
    if period_count == 0 {
        return false;
    }
    true
}

// Returns Ok if the locked vesting account has been "realized". Realization
// is application dependent. For example, in the case of staking, one must first
// unstake before being able to earn locked tokens.
fn is_realized(ctx: &Context<Withdraw>) -> Result<()> {
    if let Some(realizor) = &ctx.accounts.vesting.realizor {
        let cpi_program = {
            let p = ctx.remaining_accounts[0].clone();
            if p.key != &realizor.program {
                return err!(ErrorCode::InvalidLockRealizor);
            }
            p
        };
        let cpi_accounts = ctx.remaining_accounts.to_vec()[1..].to_vec();
        let cpi_ctx = CpiContext::new(cpi_program, cpi_accounts);
        let vesting = (*ctx.accounts.vesting).clone();
        realize_lock::is_realized(cpi_ctx, vesting)
            .map_err(|_| error!(ErrorCode::UnrealizedVesting))?;
    }
    Ok(())
}

/// RealizeLock defines the interface an external program must implement if
/// they want to define a "realization condition" on a locked vesting account.
/// This condition must be satisfied *even if a vesting schedule has
/// completed*. Otherwise the user can never earn the locked funds. For example,
/// in the case of the staking program, one cannot received a locked reward
/// until one has completely unstaked.
#[interface]
pub trait RealizeLock<'info, T: Accounts<'info>> {
    fn is_realized(ctx: Context<T>, v: Vesting) -> Result<()>;
}
