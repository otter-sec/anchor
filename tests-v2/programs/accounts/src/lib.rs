//! Test program exercising account-wrapper types that sit outside the
//! constraints/seeds/cpi suites — Sysvar, Box<Account>, SystemAccount,
//! and bare UncheckedAccount read paths.

extern crate alloc;

use {
    alloc::string::String,
    anchor_lang_v2::{
        prelude::*,
        programs::{AssociatedToken, Memo},
    },
    pinocchio::sysvars::{clock::Clock, rent::Rent},
    solana_program_error::ProgramError,
};

declare_id!("Acc1111111111111111111111111111111111111111");

const PROGRAM_OWNER: Address =
    Address::from_str_const("Acc1111111111111111111111111111111111111111");
const FOREIGN_BORSH_OWNER: Address =
    Address::from_str_const("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");
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

#[account(borsh, owner = FOREIGN_BORSH_OWNER)]
pub struct ForeignBorshCounter {
    pub value: u64,
}

#[program]
pub mod accounts_test {
    use super::*;

    /// Initialize a counter. Exercises `BorshAccount`'s init path through a
    /// regular `Account<T>` — also kicks the `Box<Account<T>>` handler below
    /// into a known state.
    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.counter.value = 1;
        Ok(())
    }

    /// Loads the counter inside a `Box` and mutates it through `Deref`.
    /// Hits `AnchorAccount for Box<T>` (`accounts/boxed.rs`).
    #[discrim = 1]
    pub fn bump_boxed(ctx: &mut Context<BumpBoxed>) -> Result<()> {
        ctx.accounts.counter.value = ctx.accounts.counter.value.wrapping_add(1);
        Ok(())
    }

    /// Loads the counter immutably inside a `Box` and only reads through
    /// `Deref`. Hits `AnchorAccount::load` for `Box<T>`.
    #[discrim = 2]
    pub fn read_boxed(ctx: &mut Context<ReadBoxed>) -> Result<()> {
        let _ = ctx.accounts.counter.value;
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

    /// Reads the Clock sysvar. Exercises `Sysvar<Clock>::load` and `Deref`
    /// forwarding to the inner pinocchio type.
    #[discrim = 5]
    pub fn read_clock(ctx: &mut Context<ReadClock>) -> Result<()> {
        // Touch several Clock fields so the register trace covers the
        // deref/getter path.
        let clock = &*ctx.accounts.clock;
        let _ = clock.slot;
        let _ = clock.epoch;
        let _ = clock.unix_timestamp;
        Ok(())
    }

    /// Reads the Rent sysvar. Same rationale as `read_clock`.
    #[discrim = 6]
    pub fn read_rent(ctx: &mut Context<ReadRent>) -> Result<()> {
        let rent = &*ctx.accounts.rent;
        let _ = rent.try_minimum_balance(100);
        Ok(())
    }

    /// Take a `SystemAccount` — validates the passed account is owned by
    /// the System program. Exercises `accounts/system_account.rs`.
    #[discrim = 7]
    pub fn check_system(ctx: &mut Context<CheckSystem>) -> Result<()> {
        let _ = ctx.accounts.wallet.address();
        Ok(())
    }

    /// Read-only UncheckedAccount — exercises load + accessor paths on
    /// `accounts/unchecked_account.rs`.
    #[discrim = 8]
    pub fn touch_unchecked(ctx: &mut Context<TouchUnchecked>) -> Result<()> {
        let _ = ctx.accounts.any_account.address();
        Ok(())
    }

    /// Checks the well-known System program marker address.
    #[discrim = 9]
    pub fn check_system_program(ctx: &mut Context<CheckSystemProgram>) -> Result<()> {
        let _ = ctx.accounts.program.address();
        Ok(())
    }

    /// Checks the well-known SPL Token program marker address.
    #[discrim = 10]
    pub fn check_token_program(ctx: &mut Context<CheckTokenProgram>) -> Result<()> {
        let _ = ctx.accounts.program.address();
        Ok(())
    }

    /// Checks the well-known Token-2022 program marker address.
    #[discrim = 11]
    pub fn check_token_2022_program(ctx: &mut Context<CheckToken2022Program>) -> Result<()> {
        let _ = ctx.accounts.program.address();
        Ok(())
    }

    /// Checks the well-known Associated Token program marker address.
    #[discrim = 12]
    pub fn check_associated_token_program(
        ctx: &mut Context<CheckAssociatedTokenProgram>,
    ) -> Result<()> {
        let _ = ctx.accounts.program.address();
        Ok(())
    }

    /// Checks the well-known Memo program marker address.
    #[discrim = 13]
    pub fn check_memo_program(ctx: &mut Context<CheckMemoProgram>) -> Result<()> {
        let _ = ctx.accounts.program.address();
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

    /// Initializes a borsh-backed counter for lamport-helper coverage.
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
    /// path must not serialize the mutation back to the account data because
    /// the runtime owner is not this program.
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
    #[account(init, payer = payer, space = 8 + core::mem::size_of::<Counter>(), unsafe(dup))]
    pub counter: Account<Counter>,
    #[account(mut, unsafe(dup))]
    pub payer: Signer,
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
