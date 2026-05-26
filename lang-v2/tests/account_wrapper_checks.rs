//! Negative-path tests for the view-wrapper account types.
//!
//! Each wrapper's `load` / `load_mut` runs a small number of checks
//! (`is_signer`, `is_writable`, owner match, address match) before
//! returning a typed handle. When the check fails the wrapper must
//! surface a precise `ProgramError`, not silently accept the account.
//!
//! These tests pin those rejection paths because they're the security
//! boundary — the derive-level constraint layer runs *after* the
//! wrapper's own gate, so a wrapper false-accept is an unrecoverable
//! auth bypass (see `program.rs` and `sysvar.rs` docs on why
//! `address = X @ MyErr` cannot override these).
//!
//! Run: `cargo test -p anchor-lang-v2 --features testing --test account_wrapper_checks`

use {
    anchor_lang_v2::{
        accounts::{
            Account, BorshAccount, Program, Signer, SystemAccount, Sysvar, UncheckedAccount,
        },
        programs::{System, Token},
        testing::AccountBuffer,
        wincode::{SchemaRead, SchemaWrite},
        Accounts, AnchorAccount, Discriminator, ErrorCode, Owner, TryAccounts,
    },
    bytemuck::{Pod, Zeroable},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

const PROGRAM_ID: [u8; 32] = [0x42; 32];
const SYSTEM_PROGRAM_ID: [u8; 32] = [0u8; 32];

fn program_id() -> Address {
    Address::new_from_array(PROGRAM_ID)
}

#[derive(SchemaRead, SchemaWrite, Default)]
struct Counter {
    value: u64,
}

impl Owner for Counter {
    fn owner(program_id: &Address) -> Address {
        *program_id
    }
}

impl Discriminator for Counter {
    // sha256("account:Counter")[..8]
    const DISCRIMINATOR: &'static [u8] = &[0xff, 0xb0, 0x04, 0xf5, 0xbc, 0xfd, 0x7c, 0x19];
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct PodCounter {
    value: u64,
}

impl Owner for PodCounter {
    fn owner(program_id: &Address) -> Address {
        *program_id
    }
}

impl Discriminator for PodCounter {
    // sha256("account:PodCounter")[..8]
    const DISCRIMINATOR: &'static [u8] = &[0x4c, 0xde, 0x7f, 0x28, 0x61, 0x2f, 0x07, 0x73];
}

#[derive(Accounts)]
struct CloseUnchecked {
    #[account(mut, close = receiver)]
    data: UncheckedAccount,
    #[account(mut)]
    receiver: UncheckedAccount,
}

fn setup_borsh_counter_buf(
    buf: &mut AccountBuffer<128>,
    owner: [u8; 32],
    writable: bool,
    value: u64,
) {
    buf.init([0x44; 32], owner, 16, false, writable, false);
    let mut data = [0u8; 16];
    data[..8].copy_from_slice(Counter::DISCRIMINATOR);
    data[8..16].copy_from_slice(&value.to_le_bytes());
    buf.write_data(&data);
}

fn setup_pod_counter_buf(
    buf: &mut AccountBuffer<128>,
    owner: [u8; 32],
    writable: bool,
    value: u64,
) {
    buf.init([0x45; 32], owner, 16, false, writable, false);
    let mut data = [0u8; 16];
    data[..8].copy_from_slice(PodCounter::DISCRIMINATOR);
    data[8..16].copy_from_slice(&value.to_le_bytes());
    buf.write_data(&data);
}

// The account wrappers don't `#[derive(Debug)]`, so `Result::unwrap_err`
// can't format the `Ok` branch. Local helper extracts the error without
// triggering the `T: Debug` bound.
fn expect_err<T>(r: Result<T, ProgramError>) -> ProgramError {
    match r {
        Ok(_) => panic!("expected Err, got Ok"),
        Err(e) => e,
    }
}

// -- Signer -------------------------------------------------------------

#[test]
fn signer_load_rejects_non_signer() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0x01; 32],
        SYSTEM_PROGRAM_ID,
        0,
        /*signer*/ false,
        false,
        false,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(Signer::load(view, &program_id()));
    assert_eq!(err, ProgramError::MissingRequiredSignature);
}

#[test]
fn signer_load_accepts_signer() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0x01; 32],
        SYSTEM_PROGRAM_ID,
        0,
        /*signer*/ true,
        false,
        false,
    );
    let view = unsafe { buf.view() };
    let signer = Signer::load(view, &program_id()).unwrap();
    assert_eq!(signer.address().to_bytes(), [0x01; 32]);
}

#[test]
fn signer_load_mut_rejects_non_signer_non_writable() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init([0x01; 32], SYSTEM_PROGRAM_ID, 0, false, false, false);
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { Signer::load_mut(view, &program_id()) });
    // Fused check: either flag missing maps to ConstraintSigner.
    assert_eq!(err, ErrorCode::ConstraintSigner.into());
}

#[test]
fn signer_load_mut_rejects_signer_without_writable() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0x01; 32],
        SYSTEM_PROGRAM_ID,
        0,
        /*signer*/ true,
        /*writable*/ false,
        false,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { Signer::load_mut(view, &program_id()) });
    assert_eq!(err, ErrorCode::ConstraintSigner.into());
}

#[test]
fn signer_load_mut_rejects_writable_without_signer() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0x01; 32],
        SYSTEM_PROGRAM_ID,
        0,
        /*signer*/ false,
        /*writable*/ true,
        false,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { Signer::load_mut(view, &program_id()) });
    assert_eq!(err, ErrorCode::ConstraintSigner.into());
}

#[test]
fn signer_load_mut_accepts_signer_and_writable() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0x01; 32],
        SYSTEM_PROGRAM_ID,
        0,
        /*signer*/ true,
        /*writable*/ true,
        false,
    );
    let view = unsafe { buf.view() };
    let signer = unsafe { Signer::load_mut(view, &program_id()) }.unwrap();
    assert_eq!(signer.address().to_bytes(), [0x01; 32]);
}

// -- SystemAccount ------------------------------------------------------

#[test]
fn system_account_load_rejects_non_system_owner() {
    let mut buf = AccountBuffer::<128>::new();
    // Owner = [0x42; 32] (program_id), not the all-zero System program id.
    buf.init([0x01; 32], PROGRAM_ID, 0, false, false, false);
    let view = unsafe { buf.view() };
    let err = expect_err(SystemAccount::load(view, &program_id()));
    assert_eq!(err, ProgramError::IllegalOwner);
}

#[test]
fn system_account_load_accepts_system_owner() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init([0x01; 32], SYSTEM_PROGRAM_ID, 0, false, false, false);
    let view = unsafe { buf.view() };
    let sa = SystemAccount::load(view, &program_id()).unwrap();
    assert_eq!(sa.address().to_bytes(), [0x01; 32]);
}

#[test]
fn system_account_default_load_mut_rejects_non_writable() {
    // SystemAccount doesn't override `load_mut`, so the default impl runs
    // an `is_writable` check first — a non-writable account must surface
    // `ConstraintMut`, not `IllegalOwner`, even though the owner check
    // would also fail.
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0x01; 32], PROGRAM_ID, 0, false, /*writable*/ false, false,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { SystemAccount::load_mut(view, &program_id()) });
    assert_eq!(err, ErrorCode::ConstraintMut.into());
}

#[test]
fn system_account_default_load_mut_rejects_writable_wrong_owner() {
    // Writable passes, then the owner check fires.
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0x01; 32], PROGRAM_ID, 0, false, /*writable*/ true, false,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { SystemAccount::load_mut(view, &program_id()) });
    assert_eq!(err, ProgramError::IllegalOwner);
}

// -- UncheckedAccount ---------------------------------------------------

#[test]
fn unchecked_account_load_accepts_anything() {
    // Whatever the flags / owner, `UncheckedAccount::load` must succeed:
    // it's the escape hatch for programs that want to run validation
    // themselves in a derive-level `address = X @ MyErr` constraint.
    let mut buf = AccountBuffer::<128>::new();
    buf.init([0xAB; 32], [0x99; 32], 0, false, false, false);
    let view = unsafe { buf.view() };
    let ua = UncheckedAccount::load(view, &program_id()).unwrap();
    assert_eq!(ua.address().to_bytes(), [0xAB; 32]);
}

#[test]
fn unchecked_account_default_load_mut_rejects_non_writable() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0xAB; 32], [0x99; 32], 0, false, /*writable*/ false, false,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { UncheckedAccount::load_mut(view, &program_id()) });
    assert_eq!(err, ErrorCode::ConstraintMut.into());
}

#[test]
fn unchecked_account_load_mut_accepts_writable() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0xAB; 32], [0x99; 32], 0, false, /*writable*/ true, false,
    );
    let view = unsafe { buf.view() };
    let ua = unsafe { UncheckedAccount::load_mut(view, &program_id()) }.unwrap();
    assert_eq!(ua.address().to_bytes(), [0xAB; 32]);
}

#[test]
fn unchecked_account_close_constraint_fails_at_runtime() {
    let data_buf = AccountBuffer::<128>::new();
    data_buf.init([0xAB; 32], [0x99; 32], 0, false, true, false);

    let receiver_buf = AccountBuffer::<128>::new();
    receiver_buf.init([0xCD; 32], [0x99; 32], 0, false, true, false);

    let views = [unsafe { data_buf.view() }, unsafe { receiver_buf.view() }];
    let (mut accounts, _, _) =
        CloseUnchecked::try_accounts(&program_id(), &views, None, 0, &[]).unwrap();

    let err = expect_err(accounts.exit_accounts());
    assert_eq!(err, ProgramError::InvalidArgument);
}

// -- Program<T> ---------------------------------------------------------

#[test]
fn program_load_rejects_wrong_address() {
    let mut buf = AccountBuffer::<128>::new();
    // Address = [0x01; 32], expecting System (all-zero).
    buf.init(
        [0x01; 32], [0u8; 32], 0, false, false, /*executable*/ true,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(Program::<System>::load(view, &program_id()));
    assert_eq!(err, ProgramError::IncorrectProgramId);
}

#[test]
fn program_load_accepts_matching_system_address() {
    let mut buf = AccountBuffer::<128>::new();
    // System program address is all-zero.
    buf.init(
        [0u8; 32], [0u8; 32], 0, false, false, /*executable*/ true,
    );
    let view = unsafe { buf.view() };
    let p = Program::<System>::load(view, &program_id()).unwrap();
    assert_eq!(p.address().to_bytes(), [0u8; 32]);
}

#[cfg(feature = "guardrails")]
#[test]
fn program_load_rejects_non_executable_under_guardrails() {
    let mut buf = AccountBuffer::<128>::new();
    // Correct address but not executable.
    buf.init(
        [0u8; 32], [0u8; 32], 0, false, false, /*executable*/ false,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(Program::<System>::load(view, &program_id()));
    assert_eq!(err, ProgramError::InvalidAccountData);
}

#[test]
fn program_load_token_wrong_address_rejects() {
    // Arbitrary non-Token address — must reject on the address compare.
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0x01; 32], [0u8; 32], 0, false, false, /*executable*/ true,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(Program::<Token>::load(view, &program_id()));
    assert_eq!(err, ProgramError::IncorrectProgramId);
}

// -- Sysvar<T> ----------------------------------------------------------

#[test]
fn sysvar_load_rejects_wrong_address() {
    // Passing a non-Clock address for `Sysvar<Clock>` must reject before
    // any syscall runs — see `sysvar.rs`'s `InvalidArgument` path.
    let mut buf = AccountBuffer::<128>::new();
    buf.init([0x01; 32], [0u8; 32], 0, false, false, false);
    let view = unsafe { buf.view() };
    let err = expect_err(Sysvar::<pinocchio::sysvars::clock::Clock>::load(
        view,
        &program_id(),
    ));
    assert_eq!(err, ProgramError::InvalidArgument);
}

// -- Account<T> / Slab<H, HeaderOnly> ----------------------------------

#[test]
fn account_load_accepts_valid_owner_and_discriminator() {
    let mut buf = AccountBuffer::<128>::new();
    setup_pod_counter_buf(&mut buf, PROGRAM_ID, false, 17);
    let view = unsafe { buf.view() };
    let acct = Account::<PodCounter>::load(view, &program_id()).unwrap();
    assert_eq!(acct.value, 17);
}

#[cfg(feature = "guardrails")]
#[test]
fn account_load_mut_rejects_non_writable() {
    let mut buf = AccountBuffer::<128>::new();
    setup_pod_counter_buf(&mut buf, PROGRAM_ID, false, 17);
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { Account::<PodCounter>::load_mut(view, &program_id()) });
    assert_eq!(err, ProgramError::InvalidAccountData);
}

#[test]
#[should_panic(
    expected = "Slab<H, T> mutably dereferenced but loaded read-only. Add #[account(mut)] to your \
                accounts struct."
)]
fn account_deref_mut_panics_when_loaded_read_only() {
    let mut buf = AccountBuffer::<128>::new();
    setup_pod_counter_buf(&mut buf, PROGRAM_ID, false, 17);
    let view = unsafe { buf.view() };
    let mut acct = Account::<PodCounter>::load(view, &program_id()).unwrap();
    acct.value = 18;
}

// -- BorshAccount<T> ---------------------------------------------------

#[test]
fn borsh_account_load_accepts_valid_owner_and_discriminator() {
    let mut buf = AccountBuffer::<128>::new();
    setup_borsh_counter_buf(&mut buf, PROGRAM_ID, false, 9);
    let view = unsafe { buf.view() };
    let acct = BorshAccount::<Counter>::load(view, &program_id()).unwrap();
    assert_eq!(acct.value, 9);
}

#[cfg(feature = "guardrails")]
#[test]
fn borsh_account_load_mut_rejects_non_writable() {
    let mut buf = AccountBuffer::<128>::new();
    setup_borsh_counter_buf(&mut buf, PROGRAM_ID, false, 9);
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { BorshAccount::<Counter>::load_mut(view, &program_id()) });
    assert_eq!(err, ProgramError::InvalidAccountData);
}

#[test]
fn borsh_account_load_rejects_wrong_owner() {
    let mut buf = AccountBuffer::<128>::new();
    setup_borsh_counter_buf(&mut buf, [0x99; 32], true, 9);
    let view = unsafe { buf.view() };
    let err = expect_err(BorshAccount::<Counter>::load(view, &program_id()));
    assert_eq!(err, ProgramError::IllegalOwner);
}

#[test]
fn borsh_account_load_mut_accepts_writable_account() {
    let mut buf = AccountBuffer::<128>::new();
    setup_borsh_counter_buf(&mut buf, PROGRAM_ID, true, 9);
    let view = unsafe { buf.view() };
    let acct = unsafe { BorshAccount::<Counter>::load_mut(view, &program_id()) }.unwrap();
    assert_eq!(acct.value, 9);
}

#[test]
#[should_panic(expected = "use #[account(mut)] for mutable access")]
fn borsh_account_deref_mut_panics_when_loaded_read_only() {
    let mut buf = AccountBuffer::<128>::new();
    setup_borsh_counter_buf(&mut buf, PROGRAM_ID, false, 9);
    let view = unsafe { buf.view() };
    let mut acct = BorshAccount::<Counter>::load(view, &program_id()).unwrap();
    acct.value = 10;
}
