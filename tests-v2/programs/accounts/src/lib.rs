//! Examples for account-wrapper types that sit outside the constraints,
//! seeds, and CPI suites: Sysvar, Box<Account>, SystemAccount, and bare
//! UncheckedAccount values.

extern crate alloc;

use {
    alloc::string::String,
    anchor_lang_v2::{
        accounts::Slab,
        prelude::*,
        programs::{AssociatedToken, Memo},
    },
    foreign_borsh_account::ForeignBorshCounter,
    pinocchio::sysvars::{clock::Clock, rent::Rent},
    solana_program_error::ProgramError,
};

declare_id!("Acc1111111111111111111111111111111111111111");

const PROGRAM_OWNER: Address =
    anchor_lang_v2::address!("Acc1111111111111111111111111111111111111111");
const SYSTEM_SEED: &str = "anchor-v2-seed";
const SYSTEM_TRANSFER_SEED: &str = "anchor-v2-transfer";

#[account]
pub struct Counter {
    pub value: u64,
}

#[account(borsh)]
pub struct BorshCounter {
    pub value: u64,
}

#[account]
pub struct Ledger {
    pub checksum: u64,
    pub last_space: u64,
}

#[repr(C)]
#[derive(Clone, Copy, Default, bytemuck::Pod, bytemuck::Zeroable)]
pub struct LedgerEntry {
    pub amount: u64,
}

type LedgerAccount = Slab<Ledger, LedgerEntry>;

#[program]
pub mod accounts_test {
    use super::*;

    /// Initializes the canonical counter used by the boxed-account handlers.
    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.counter.value = 1;
        Ok(())
    }

    /// Loads the counter inside a `Box` and mutates it through `Deref`.
    #[discrim = 1]
    pub fn bump_boxed(ctx: &mut Context<BumpBoxed>) -> Result<()> {
        ctx.accounts.counter.value = ctx.accounts.counter.value.wrapping_add(1);
        Ok(())
    }

    /// Loads the counter immutably inside a `Box` and reads through `Deref`.
    #[discrim = 2]
    pub fn read_boxed(ctx: &mut Context<ReadBoxed>, expected_value: u64) -> Result<()> {
        require_eq!(
            ctx.accounts.counter.value,
            expected_value,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Initializes a boxed counter via `AccountInitialize for Box<T>`.
    #[discrim = 3]
    pub fn initialize_boxed(ctx: &mut Context<InitializeBoxed>) -> Result<()> {
        ctx.accounts.counter.value = 7;
        Ok(())
    }

    /// Closes a boxed counter, forwarding through `AnchorAccount::close`.
    #[discrim = 4]
    pub fn close_boxed(_ctx: &mut Context<CloseBoxed>) -> Result<()> {
        Ok(())
    }

    /// Reads the Clock sysvar through `Deref` forwarding to the inner type.
    #[discrim = 5]
    pub fn read_clock(ctx: &mut Context<ReadClock>) -> Result<()> {
        // Touch several Clock fields to show ordinary typed sysvar access.
        let clock = &*ctx.accounts.clock;
        require!(clock.epoch <= clock.slot, ProgramError::InvalidAccountData);
        require!(clock.unix_timestamp >= 0, ProgramError::InvalidAccountData);
        Ok(())
    }

    /// Reads the Rent sysvar through typed helper methods.
    #[discrim = 6]
    pub fn read_rent(ctx: &mut Context<ReadRent>) -> Result<()> {
        let rent = &*ctx.accounts.rent;
        require!(
            rent.try_minimum_balance(100)? > 0,
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Takes a `SystemAccount`, which validates that the account is owned by
    /// the System program.
    #[discrim = 7]
    pub fn check_system(ctx: &mut Context<CheckSystem>, expected_wallet: Address) -> Result<()> {
        require!(
            anchor_lang_v2::address_eq(ctx.accounts.wallet.address(), &expected_wallet),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Reads the address from an unchecked account when the program does its
    /// own validation.
    #[discrim = 8]
    pub fn touch_unchecked(
        ctx: &mut Context<TouchUnchecked>,
        expected_account: Address,
    ) -> Result<()> {
        require!(
            anchor_lang_v2::address_eq(ctx.accounts.any_account.address(), &expected_account),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Checks the well-known System program marker address.
    #[discrim = 9]
    pub fn check_system_program(ctx: &mut Context<CheckSystemProgram>) -> Result<()> {
        require!(
            anchor_lang_v2::address_eq(ctx.accounts.program.address(), &system_program::ID),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Checks the well-known SPL Token program marker address.
    #[discrim = 10]
    pub fn check_token_program(ctx: &mut Context<CheckTokenProgram>) -> Result<()> {
        require!(
            anchor_lang_v2::address_eq(ctx.accounts.program.address(), &Token::id()),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Checks the well-known Token-2022 program marker address.
    #[discrim = 11]
    pub fn check_token_2022_program(ctx: &mut Context<CheckToken2022Program>) -> Result<()> {
        require!(
            anchor_lang_v2::address_eq(ctx.accounts.program.address(), &Token2022::id()),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Checks the well-known Associated Token program marker address.
    #[discrim = 12]
    pub fn check_associated_token_program(
        ctx: &mut Context<CheckAssociatedTokenProgram>,
    ) -> Result<()> {
        require!(
            anchor_lang_v2::address_eq(ctx.accounts.program.address(), &AssociatedToken::id()),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Checks the well-known Memo program marker address.
    #[discrim = 13]
    pub fn check_memo_program(ctx: &mut Context<CheckMemoProgram>) -> Result<()> {
        require!(
            anchor_lang_v2::address_eq(ctx.accounts.program.address(), &Memo::id()),
            ProgramError::InvalidAccountData
        );
        Ok(())
    }

    /// Transfers lamports through `anchor_lang_v2::system_program::transfer`.
    #[discrim = 14]
    pub fn transfer_lamports(ctx: &mut Context<TransferLamports>, amount: u64) -> Result<()> {
        let cpi_accounts = system_program::Transfer {
            from: ctx.accounts.from.cpi_handle_mut(),
            to: ctx.accounts.to.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::transfer(cpi_ctx, amount)?;
        Ok(())
    }

    #[discrim = 15]
    pub fn create_system_account(
        ctx: &mut Context<CreateSystemAccount>,
        lamports: u64,
        space: u64,
    ) -> Result<()> {
        let cpi_accounts = system_program::CreateAccount {
            from: ctx.accounts.from.cpi_handle_mut(),
            to: ctx.accounts.to.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::create_account(cpi_ctx, lamports, space, &PROGRAM_OWNER)?;
        Ok(())
    }

    #[discrim = 16]
    pub fn create_system_account_with_seed(
        ctx: &mut Context<CreateSystemAccountWithSeed>,
        lamports: u64,
        space: u64,
    ) -> Result<()> {
        let cpi_accounts = system_program::CreateAccountWithSeed {
            from: ctx.accounts.from.cpi_handle_mut(),
            to: ctx.accounts.to.cpi_handle_mut(),
            base: ctx.accounts.base.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::create_account_with_seed(
            cpi_ctx,
            SYSTEM_SEED,
            lamports,
            space,
            &PROGRAM_OWNER,
        )?;
        Ok(())
    }

    #[discrim = 17]
    pub fn allocate_system_account(
        ctx: &mut Context<AllocateSystemAccount>,
        space: u64,
    ) -> Result<()> {
        let cpi_accounts = system_program::Allocate {
            account_to_allocate: ctx.accounts.account_to_allocate.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::allocate(cpi_ctx, space)?;
        Ok(())
    }

    #[discrim = 18]
    pub fn allocate_system_account_with_seed(
        ctx: &mut Context<AllocateSystemAccountWithSeed>,
        space: u64,
    ) -> Result<()> {
        let cpi_accounts = system_program::AllocateWithSeed {
            account_to_allocate: ctx.accounts.account_to_allocate.cpi_handle_mut(),
            base: ctx.accounts.base.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::allocate_with_seed(cpi_ctx, SYSTEM_SEED, space, &PROGRAM_OWNER)?;
        Ok(())
    }

    #[discrim = 19]
    pub fn assign_system_account(ctx: &mut Context<AssignSystemAccount>) -> Result<()> {
        let cpi_accounts = system_program::Assign {
            account_to_assign: ctx.accounts.account_to_assign.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::assign(cpi_ctx, &PROGRAM_OWNER)?;
        Ok(())
    }

    #[discrim = 20]
    pub fn assign_system_account_with_seed(
        ctx: &mut Context<AssignSystemAccountWithSeed>,
    ) -> Result<()> {
        let cpi_accounts = system_program::AssignWithSeed {
            account_to_assign: ctx.accounts.account_to_assign.cpi_handle_mut(),
            base: ctx.accounts.base.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::assign_with_seed(cpi_ctx, SYSTEM_SEED, &PROGRAM_OWNER)?;
        Ok(())
    }

    #[discrim = 21]
    pub fn transfer_lamports_with_seed(
        ctx: &mut Context<TransferLamportsWithSeed>,
        amount: u64,
    ) -> Result<()> {
        let cpi_accounts = system_program::TransferWithSeed {
            from: ctx.accounts.from.cpi_handle_mut(),
            base: ctx.accounts.base.cpi_handle(),
            to: ctx.accounts.to.cpi_handle_mut(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::transfer_with_seed(
            cpi_ctx,
            String::from(SYSTEM_TRANSFER_SEED),
            &system_program::ID,
            amount,
        )?;
        Ok(())
    }

    #[discrim = 22]
    pub fn create_nonce(ctx: &mut Context<CreateNonce>, lamports: u64) -> Result<()> {
        let cpi_accounts = system_program::CreateNonceAccount {
            from: ctx.accounts.from.cpi_handle_mut(),
            nonce: ctx.accounts.nonce.cpi_handle_mut(),
            recent_blockhashes: ctx.accounts.recent_blockhashes.cpi_handle(),
            rent: ctx.accounts.rent.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::create_nonce_account(cpi_ctx, lamports, ctx.accounts.authority.address())?;
        Ok(())
    }

    #[discrim = 23]
    pub fn create_nonce_with_seed(
        ctx: &mut Context<CreateNonceWithSeed>,
        lamports: u64,
    ) -> Result<()> {
        let cpi_accounts = system_program::CreateNonceAccountWithSeed {
            from: ctx.accounts.from.cpi_handle_mut(),
            nonce: ctx.accounts.nonce.cpi_handle_mut(),
            base: ctx.accounts.base.cpi_handle(),
            recent_blockhashes: ctx.accounts.recent_blockhashes.cpi_handle(),
            rent: ctx.accounts.rent.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::create_nonce_account_with_seed(
            cpi_ctx,
            lamports,
            SYSTEM_SEED,
            ctx.accounts.authority.address(),
        )?;
        Ok(())
    }

    #[discrim = 24]
    pub fn advance_nonce(ctx: &mut Context<AdvanceNonce>) -> Result<()> {
        let cpi_accounts = system_program::AdvanceNonceAccount {
            nonce: ctx.accounts.nonce.cpi_handle_mut(),
            authorized: ctx.accounts.authorized.cpi_handle(),
            recent_blockhashes: ctx.accounts.recent_blockhashes.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::advance_nonce_account(cpi_ctx)?;
        Ok(())
    }

    #[discrim = 25]
    pub fn authorize_nonce(ctx: &mut Context<AuthorizeNonce>) -> Result<()> {
        let cpi_accounts = system_program::AuthorizeNonceAccount {
            nonce: ctx.accounts.nonce.cpi_handle_mut(),
            authorized: ctx.accounts.authorized.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::authorize_nonce_account(cpi_ctx, ctx.accounts.new_authority.address())?;
        Ok(())
    }

    #[discrim = 26]
    pub fn withdraw_nonce(ctx: &mut Context<WithdrawNonce>, amount: u64) -> Result<()> {
        let cpi_accounts = system_program::WithdrawNonceAccount {
            nonce: ctx.accounts.nonce.cpi_handle_mut(),
            to: ctx.accounts.to.cpi_handle_mut(),
            recent_blockhashes: ctx.accounts.recent_blockhashes.cpi_handle(),
            rent: ctx.accounts.rent.cpi_handle(),
            authorized: ctx.accounts.authorized.cpi_handle(),
        };
        let cpi_ctx = CpiContext::new(ctx.accounts.system_program.address(), cpi_accounts);
        system_program::withdraw_nonce_account(cpi_ctx, amount)?;
        Ok(())
    }

    #[discrim = 27]
    pub fn reject_wrong_system_program(
        ctx: &mut Context<RejectWrongSystemProgram>,
        opcode: u8,
    ) -> Result<()> {
        let program = ctx.accounts.program.address();
        let authority = *ctx.accounts.a.address();
        match opcode {
            0 => system_program::advance_nonce_account(CpiContext::new(
                program,
                system_program::AdvanceNonceAccount {
                    nonce: ctx.accounts.a.cpi_handle_mut(),
                    authorized: ctx.accounts.b.cpi_handle(),
                    recent_blockhashes: ctx.accounts.c.cpi_handle(),
                },
            )),
            1 => system_program::allocate(
                CpiContext::new(
                    program,
                    system_program::Allocate {
                        account_to_allocate: ctx.accounts.a.cpi_handle_mut(),
                    },
                ),
                8,
            ),
            2 => system_program::allocate_with_seed(
                CpiContext::new(
                    program,
                    system_program::AllocateWithSeed {
                        account_to_allocate: ctx.accounts.a.cpi_handle_mut(),
                        base: ctx.accounts.b.cpi_handle(),
                    },
                ),
                SYSTEM_SEED,
                8,
                &PROGRAM_OWNER,
            ),
            3 => system_program::assign(
                CpiContext::new(
                    program,
                    system_program::Assign {
                        account_to_assign: ctx.accounts.a.cpi_handle_mut(),
                    },
                ),
                &PROGRAM_OWNER,
            ),
            4 => system_program::assign_with_seed(
                CpiContext::new(
                    program,
                    system_program::AssignWithSeed {
                        account_to_assign: ctx.accounts.a.cpi_handle_mut(),
                        base: ctx.accounts.b.cpi_handle(),
                    },
                ),
                SYSTEM_SEED,
                &PROGRAM_OWNER,
            ),
            5 => system_program::authorize_nonce_account(
                CpiContext::new(
                    program,
                    system_program::AuthorizeNonceAccount {
                        nonce: ctx.accounts.a.cpi_handle_mut(),
                        authorized: ctx.accounts.b.cpi_handle(),
                    },
                ),
                ctx.accounts.c.address(),
            ),
            6 => system_program::create_account(
                CpiContext::new(
                    program,
                    system_program::CreateAccount {
                        from: ctx.accounts.a.cpi_handle_mut(),
                        to: ctx.accounts.b.cpi_handle_mut(),
                    },
                ),
                1,
                0,
                &PROGRAM_OWNER,
            ),
            7 => system_program::create_account_with_seed(
                CpiContext::new(
                    program,
                    system_program::CreateAccountWithSeed {
                        from: ctx.accounts.a.cpi_handle_mut(),
                        to: ctx.accounts.b.cpi_handle_mut(),
                        base: ctx.accounts.c.cpi_handle(),
                    },
                ),
                SYSTEM_SEED,
                1,
                0,
                &PROGRAM_OWNER,
            ),
            8 => system_program::create_nonce_account(
                CpiContext::new(
                    program,
                    system_program::CreateNonceAccount {
                        from: ctx.accounts.a.cpi_handle_mut(),
                        nonce: ctx.accounts.b.cpi_handle_mut(),
                        recent_blockhashes: ctx.accounts.c.cpi_handle(),
                        rent: ctx.accounts.d.cpi_handle(),
                    },
                ),
                1,
                ctx.accounts.e.address(),
            ),
            9 => system_program::create_nonce_account_with_seed(
                CpiContext::new(
                    program,
                    system_program::CreateNonceAccountWithSeed {
                        from: ctx.accounts.a.cpi_handle_mut(),
                        nonce: ctx.accounts.b.cpi_handle_mut(),
                        base: ctx.accounts.c.cpi_handle(),
                        recent_blockhashes: ctx.accounts.d.cpi_handle(),
                        rent: ctx.accounts.e.cpi_handle(),
                    },
                ),
                1,
                SYSTEM_SEED,
                &authority,
            ),
            10 => system_program::transfer(
                CpiContext::new(
                    program,
                    system_program::Transfer {
                        from: ctx.accounts.a.cpi_handle_mut(),
                        to: ctx.accounts.b.cpi_handle_mut(),
                    },
                ),
                1,
            ),
            11 => system_program::transfer_with_seed(
                CpiContext::new(
                    program,
                    system_program::TransferWithSeed {
                        from: ctx.accounts.a.cpi_handle_mut(),
                        base: ctx.accounts.b.cpi_handle(),
                        to: ctx.accounts.c.cpi_handle_mut(),
                    },
                ),
                String::from(SYSTEM_TRANSFER_SEED),
                &system_program::ID,
                1,
            ),
            12 => system_program::withdraw_nonce_account(
                CpiContext::new(
                    program,
                    system_program::WithdrawNonceAccount {
                        nonce: ctx.accounts.a.cpi_handle_mut(),
                        to: ctx.accounts.b.cpi_handle_mut(),
                        recent_blockhashes: ctx.accounts.c.cpi_handle(),
                        rent: ctx.accounts.d.cpi_handle(),
                        authorized: ctx.accounts.e.cpi_handle(),
                    },
                ),
                1,
            ),
            _ => Err(ProgramError::InvalidInstructionData),
        }?;
        Ok(())
    }

    /// Initializes an account whose init constraint references accounts that
    /// appear later in the account struct.
    #[discrim = 28]
    pub fn initialize_with_later_seed(ctx: &mut Context<InitializeWithLaterSeed>) -> Result<()> {
        ctx.accounts.counter.value = 42;
        Ok(())
    }

    /// Transfers lamports directly from a program-owned `Account<T>` with the
    /// v1-compatible `Lamports` helpers.
    #[discrim = 29]
    pub fn transfer_from_counter_with_lamports_helpers(
        ctx: &mut Context<TransferFromCounterWithLamportsHelpers>,
        amount: u64,
    ) -> Result<()> {
        ctx.accounts.counter.sub_lamports(amount)?;
        ctx.accounts.recipient.add_lamports(amount)?;
        Ok(())
    }

    /// Initializes a borsh-backed counter for the lamport-helper examples.
    #[discrim = 30]
    pub fn initialize_borsh_counter(ctx: &mut Context<InitializeBorshCounter>) -> Result<()> {
        ctx.accounts.counter.value = 11;
        Ok(())
    }

    /// Transfers lamports directly from a program-owned `BorshAccount<T>` with
    /// the v1-compatible `Lamports` helpers.
    #[discrim = 31]
    pub fn transfer_from_borsh_counter_with_lamports_helpers(
        ctx: &mut Context<TransferFromBorshCounterWithLamportsHelpers>,
        amount: u64,
    ) -> Result<()> {
        ctx.accounts.counter.sub_lamports(amount)?;
        ctx.accounts.recipient.add_lamports(amount)?;
        Ok(())
    }

    /// Mutates a foreign-owned `BorshAccount<T>` in memory. The generated exit
    /// path serializes mutable borrows; the runtime rejects the resulting
    /// foreign account data write.
    #[discrim = 32]
    pub fn mutate_foreign_borsh_counter(
        ctx: &mut Context<MutateForeignBorshCounter>,
    ) -> Result<()> {
        ctx.accounts.counter.value = 999;
        Ok(())
    }

    #[discrim = 33]
    pub fn initialize_target_before_payer(
        ctx: &mut Context<InitializeTargetBeforePayer>,
    ) -> Result<()> {
        ctx.accounts.counter.value = 11;
        Ok(())
    }

    /// Tops up a loaded `Account<T>` to its rent-exempt floor while the
    /// slab-backed account is already borrowed.
    #[discrim = 34]
    pub fn top_up_counter(ctx: &mut Context<TopUpCounter>) -> Result<()> {
        ctx.accounts.counter.top_up(ctx.accounts.payer.as_ref())?;
        Ok(())
    }

    #[discrim = 35]
    pub fn initialize_ledger(ctx: &mut Context<InitializeLedger>) -> Result<()> {
        require_eq!(
            ctx.accounts.ledger.current_space(),
            LedgerAccount::space_for(4),
            ProgramError::InvalidAccountData
        );
        require_eq!(
            LedgerAccount::try_space_for(4)?,
            ctx.accounts.ledger.current_space(),
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ctx.accounts.ledger.capacity(),
            4,
            ProgramError::InvalidAccountData
        );
        require!(
            ctx.accounts.ledger.is_empty(),
            ProgramError::InvalidAccountData
        );
        require!(
            !ctx.accounts.ledger.is_full(),
            ProgramError::InvalidAccountData
        );
        ctx.accounts.ledger.checksum = 1;
        ctx.accounts.ledger.last_space = ctx.accounts.ledger.current_space() as u64;
        Ok(())
    }

    #[discrim = 36]
    pub fn exercise_ledger_methods(ctx: &mut Context<ExerciseLedgerMethods>) -> Result<()> {
        let ledger = &mut ctx.accounts.ledger;
        require_eq!(
            ledger.address(),
            ledger.view().address(),
            ProgramError::InvalidAccountData
        );
        require_eq!(ledger.len(), 0, ProgramError::InvalidAccountData);

        ledger.try_push(LedgerEntry { amount: 10 })?;
        ledger.try_push(LedgerEntry { amount: 20 })?;
        ledger.try_push(LedgerEntry { amount: 30 })?;
        require_eq!(ledger.len(), 3, ProgramError::InvalidAccountData);
        require_eq!(ledger.as_slice().len(), 3, ProgramError::InvalidAccountData);
        require_eq!(
            ledger.first().map(|entry| entry.amount),
            Some(10),
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ledger.last().map(|entry| entry.amount),
            Some(30),
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ledger.get(1).map(|entry| entry.amount),
            Some(20),
            ProgramError::InvalidAccountData
        );

        for entry in ledger.iter_mut() {
            entry.amount += 1;
        }
        ledger
            .get_mut(1)
            .ok_or(ProgramError::InvalidAccountData)?
            .amount = 99;
        ledger[0].amount += 100;
        ledger[2].amount += 1_000;
        require_eq!(
            ledger.iter().map(|entry| entry.amount).sum::<u64>(),
            1_241,
            ProgramError::InvalidAccountData
        );

        let removed = ledger.swap_remove(1);
        require_eq!(removed.amount, 99, ProgramError::InvalidAccountData);
        require_eq!(ledger.len(), 2, ProgramError::InvalidAccountData);
        require_eq!(
            ledger.as_slice()[0].amount,
            111,
            ProgramError::InvalidAccountData
        );
        require_eq!(
            ledger.as_slice()[1].amount,
            1_031,
            ProgramError::InvalidAccountData
        );

        let popped = ledger.pop().ok_or(ProgramError::InvalidAccountData)?;
        require_eq!(popped.amount, 1_031, ProgramError::InvalidAccountData);
        ledger.truncate(8);
        require_eq!(ledger.len(), 1, ProgramError::InvalidAccountData);
        ledger.clear();
        require!(ledger.is_empty(), ProgramError::InvalidAccountData);

        while !ledger.is_full() {
            ledger.try_push(LedgerEntry {
                amount: ledger.len() as u64,
            })?;
        }
        require_eq!(
            ledger.try_push(LedgerEntry { amount: 99 }).err(),
            Some(ProgramError::AccountDataTooSmall),
            ProgramError::InvalidAccountData
        );

        ledger.resize_to_capacity(8)?;
        ledger.top_up(ctx.accounts.payer.as_ref())?;
        require_eq!(ledger.capacity(), 8, ProgramError::InvalidAccountData);
        require_eq!(
            ledger.current_space(),
            LedgerAccount::space_for(8),
            ProgramError::InvalidAccountData
        );
        require_eq!(ledger.len(), 4, ProgramError::InvalidAccountData);

        ledger.resize_to_capacity(2)?;
        require_eq!(ledger.capacity(), 2, ProgramError::InvalidAccountData);
        require_eq!(ledger.len(), 2, ProgramError::InvalidAccountData);
        ledger[1].amount += 7;
        require_eq!(ledger[1].amount, 8, ProgramError::InvalidAccountData);
        let mut payer_view = *ctx.accounts.payer.as_ref();
        ledger.refund(&mut payer_view)?;

        ledger.checksum = ledger.as_slice().iter().map(|entry| entry.amount).sum();
        ledger.last_space = ledger.current_space() as u64;
        Ok(())
    }
}

// -- Accounts structs --------------------------------------------------------

#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, seeds = [b"counter"], bump)]
    pub counter: Account<Counter>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitializeWithLaterSeed {
    #[account(init, payer = payer, seeds = [b"later-seed", payer.address().as_ref()], bump)]
    pub counter: Account<Counter>,
    #[account(mut)]
    pub payer: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitializeTargetBeforePayer {
    #[account(init, payer = payer, unsafe(dup))]
    pub counter: Account<Counter>,
    #[account(mut, unsafe(dup))]
    pub payer: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct TopUpCounter {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, seeds = [b"counter"], bump)]
    pub counter: Account<Counter>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct InitializeLedger {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = LedgerAccount::space_for(4), seeds = [b"ledger"], bump)]
    pub ledger: LedgerAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct ExerciseLedgerMethods {
    #[account(mut)]
    pub payer: Signer,
    #[account(mut, seeds = [b"ledger"], bump)]
    pub ledger: LedgerAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct TransferFromCounterWithLamportsHelpers {
    #[account(mut, seeds = [b"counter"], bump)]
    pub counter: Account<Counter>,
    #[account(mut)]
    pub recipient: SystemAccount,
}

#[derive(Accounts)]
pub struct InitializeBorshCounter {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, space = 16, seeds = [b"borsh-counter"], bump)]
    pub counter: BorshAccount<BorshCounter>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct TransferFromBorshCounterWithLamportsHelpers {
    #[account(mut, seeds = [b"borsh-counter"], bump)]
    pub counter: BorshAccount<BorshCounter>,
    #[account(mut)]
    pub recipient: SystemAccount,
}

#[derive(Accounts)]
pub struct MutateForeignBorshCounter {
    #[account(mut)]
    pub counter: BorshAccount<ForeignBorshCounter>,
}

#[derive(Accounts)]
pub struct BumpBoxed {
    #[account(mut)]
    pub counter: Box<Account<Counter>>,
}

#[derive(Accounts)]
pub struct ReadBoxed {
    #[account(seeds = [b"boxed-counter"], bump)]
    pub counter: Box<Account<Counter>>,
}

#[derive(Accounts)]
pub struct InitializeBoxed {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, seeds = [b"boxed-counter"], bump)]
    pub counter: Box<Account<Counter>>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct CloseBoxed {
    #[account(mut, close = receiver, seeds = [b"boxed-counter"], bump)]
    pub counter: Box<Account<Counter>>,
    #[account(mut)]
    pub receiver: SystemAccount,
}

#[derive(Accounts)]
pub struct ReadClock {
    pub clock: Sysvar<Clock>,
}

#[derive(Accounts)]
pub struct ReadRent {
    pub rent: Sysvar<Rent>,
}

#[derive(Accounts)]
pub struct CheckSystem {
    pub wallet: SystemAccount,
}

#[derive(Accounts)]
pub struct TouchUnchecked {
    pub any_account: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CheckSystemProgram {
    pub program: Program<System>,
}

#[derive(Accounts)]
pub struct CheckTokenProgram {
    pub program: Program<Token>,
}

#[derive(Accounts)]
pub struct CheckToken2022Program {
    pub program: Program<Token2022>,
}

#[derive(Accounts)]
pub struct CheckAssociatedTokenProgram {
    pub program: Program<AssociatedToken>,
}

#[derive(Accounts)]
pub struct CheckMemoProgram {
    pub program: Program<Memo>,
}

#[derive(Accounts)]
pub struct TransferLamports {
    #[account(mut)]
    pub from: Signer,
    #[account(mut)]
    pub to: SystemAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct CreateSystemAccount {
    #[account(mut)]
    pub from: Signer,
    #[account(mut)]
    pub to: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct CreateSystemAccountWithSeed {
    #[account(mut)]
    pub from: Signer,
    #[account(mut)]
    pub to: UncheckedAccount,
    pub base: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct AllocateSystemAccount {
    #[account(mut)]
    pub account_to_allocate: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct AllocateSystemAccountWithSeed {
    #[account(mut)]
    pub account_to_allocate: UncheckedAccount,
    pub base: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct AssignSystemAccount {
    #[account(mut)]
    pub account_to_assign: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct AssignSystemAccountWithSeed {
    #[account(mut)]
    pub account_to_assign: UncheckedAccount,
    pub base: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct TransferLamportsWithSeed {
    #[account(mut)]
    pub from: UncheckedAccount,
    pub base: Signer,
    #[account(mut)]
    pub to: SystemAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct CreateNonce {
    #[account(mut)]
    pub from: Signer,
    #[account(mut)]
    pub nonce: Signer,
    pub authority: UncheckedAccount,
    pub recent_blockhashes: UncheckedAccount,
    pub rent: UncheckedAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct CreateNonceWithSeed {
    #[account(mut)]
    pub from: Signer,
    #[account(mut)]
    pub nonce: UncheckedAccount,
    pub base: Signer,
    pub authority: UncheckedAccount,
    pub recent_blockhashes: UncheckedAccount,
    pub rent: UncheckedAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct AdvanceNonce {
    #[account(mut)]
    pub nonce: UncheckedAccount,
    pub authorized: Signer,
    pub recent_blockhashes: UncheckedAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct AuthorizeNonce {
    #[account(mut)]
    pub nonce: UncheckedAccount,
    pub authorized: Signer,
    pub new_authority: UncheckedAccount,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct WithdrawNonce {
    #[account(mut)]
    pub nonce: UncheckedAccount,
    #[account(mut)]
    pub to: SystemAccount,
    pub recent_blockhashes: UncheckedAccount,
    pub rent: UncheckedAccount,
    pub authorized: Signer,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct RejectWrongSystemProgram {
    #[account(mut)]
    pub a: UncheckedAccount,
    #[account(mut)]
    pub b: UncheckedAccount,
    #[account(mut)]
    pub c: UncheckedAccount,
    #[account(mut)]
    pub d: UncheckedAccount,
    #[account(mut)]
    pub e: UncheckedAccount,
    pub program: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CheckAssociatedTokenProgramSeed {
    #[account(seeds = [b"vault"], bump, seeds::program = AssociatedToken::id())]
    pub data: UncheckedAccount,
}
