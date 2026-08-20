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
//! Run: `cargo test -p anchor-lang --features testing --test account_wrapper_checks`

use {
    anchor_lang::{
        accounts::{
            Account, BorshAccount, Interface, Program, Signer, SlabSchema, SystemAccount, Sysvar,
            SysvarInstructions, UncheckedAccount,
        },
        programs::{System, Token},
        testing::AccountBuffer,
        Accounts, AnchorAccount, AnchorDeserialize, AnchorSerialize, Discriminator, ErrorCode, Ids,
        Owner, TryAccounts,
    },
    bytemuck::{Pod, Zeroable},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

const PROGRAM_ID: [u8; 32] = [0x42; 32];
const SYSTEM_PROGRAM_ID: [u8; 32] = [0u8; 32];

struct TestInterface;

impl Ids for TestInterface {
    fn ids() -> &'static [Address] {
        static IDS: [Address; 1] = [Address::new_from_array(SYSTEM_PROGRAM_ID)];
        &IDS
    }
}

#[derive(AnchorDeserialize, AnchorSerialize, Default)]
struct Counter {
    value: u64,
}

impl Owner for Counter {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
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
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for PodCounter {
    // sha256("account:PodCounter")[..8]
    const DISCRIMINATOR: &'static [u8] = &[0x4c, 0xde, 0x7f, 0x28, 0x61, 0x2f, 0x07, 0x73];
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ShortDiscBytes {
    value: [u8; 4],
}

impl Owner for ShortDiscBytes {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for ShortDiscBytes {
    const DISCRIMINATOR: &'static [u8] = &[0x11, 0x22, 0x33, 0x44];
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct LongDiscCounter {
    value: u64,
}

impl Owner for LongDiscCounter {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for LongDiscCounter {
    const DISCRIMINATOR: &'static [u8] = &[
        0x41, 0x42, 0x43, 0x44, 0x45, 0x46, 0x47, 0x48, 0x49, 0x4a, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f,
        0x50,
    ];
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
    let err = expect_err(Signer::load(view));
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
    let signer = Signer::load(view).unwrap();
    assert_eq!(signer.address().to_bytes(), [0x01; 32]);
}

#[test]
fn signer_load_mut_rejects_non_signer_non_writable() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init([0x01; 32], SYSTEM_PROGRAM_ID, 0, false, false, false);
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { Signer::load_mut(view) });
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
    let err = expect_err(unsafe { Signer::load_mut(view) });
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
    let err = expect_err(unsafe { Signer::load_mut(view) });
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
    let signer = unsafe { Signer::load_mut(view) }.unwrap();
    assert_eq!(signer.address().to_bytes(), [0x01; 32]);
}

// -- SystemAccount ------------------------------------------------------

#[test]
fn system_account_load_rejects_non_system_owner() {
    let mut buf = AccountBuffer::<128>::new();
    // Owner = [0x42; 32] (program_id), not the all-zero System program id.
    buf.init([0x01; 32], PROGRAM_ID, 0, false, false, false);
    let view = unsafe { buf.view() };
    let err = expect_err(SystemAccount::load(view));
    assert_eq!(err, ProgramError::IllegalOwner);
}

#[test]
fn system_account_load_accepts_system_owner() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init([0x01; 32], SYSTEM_PROGRAM_ID, 0, false, false, false);
    let view = unsafe { buf.view() };
    let sa = SystemAccount::load(view).unwrap();
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
    let err = expect_err(unsafe { SystemAccount::load_mut(view) });
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
    let err = expect_err(unsafe { SystemAccount::load_mut(view) });
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
    let ua = UncheckedAccount::load(view).unwrap();
    assert_eq!(ua.address().to_bytes(), [0xAB; 32]);
}

#[test]
fn unchecked_account_default_load_mut_rejects_non_writable() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0xAB; 32], [0x99; 32], 0, false, /*writable*/ false, false,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { UncheckedAccount::load_mut(view) });
    assert_eq!(err, ErrorCode::ConstraintMut.into());
}

#[test]
fn unchecked_account_load_mut_accepts_writable() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0xAB; 32], [0x99; 32], 0, false, /*writable*/ true, false,
    );
    let view = unsafe { buf.view() };
    let ua = unsafe { UncheckedAccount::load_mut(view) }.unwrap();
    assert_eq!(ua.address().to_bytes(), [0xAB; 32]);
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
    let err = expect_err(Program::<System>::load(view));
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
    let p = Program::<System>::load(view).unwrap();
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
    let err = expect_err(Program::<System>::load(view));
    assert_eq!(err, ErrorCode::ConstraintExecutable.into());
}

#[test]
fn program_load_token_wrong_address_rejects() {
    // Arbitrary non-Token address — must reject on the address compare.
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0x01; 32], [0u8; 32], 0, false, false, /*executable*/ true,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(Program::<Token>::load(view));
    assert_eq!(err, ProgramError::IncorrectProgramId);
}

#[cfg(feature = "guardrails")]
#[test]
fn interface_load_rejects_non_executable_under_guardrails() {
    let mut buf = AccountBuffer::<128>::new();
    buf.init(
        [0u8; 32], [0u8; 32], 0, false, false, /*executable*/ false,
    );
    let view = unsafe { buf.view() };
    let err = expect_err(Interface::<TestInterface>::load(view));
    assert_eq!(err, ErrorCode::ConstraintExecutable.into());
}

// -- Sysvar<T> ----------------------------------------------------------

#[test]
fn sysvar_load_rejects_wrong_address() {
    // Passing a non-Clock address for `Sysvar<Clock>` must reject before
    // any syscall runs — see `sysvar.rs`'s `InvalidArgument` path.
    let mut buf = AccountBuffer::<128>::new();
    buf.init([0x01; 32], [0u8; 32], 0, false, false, false);
    let view = unsafe { buf.view() };
    let err = expect_err(Sysvar::<pinocchio::sysvars::clock::Clock>::load(view));
    assert_eq!(err, ProgramError::InvalidArgument);
}

// -- Sysvar<SysvarInstructions> -----------------------------------------------
//
// Unlike `Clock` / `Rent`, this sysvar has no `sol_get_sysvar` syscall — the
// value is read out of the account's data buffer. These tests build a
// synthetic sysvar blob matching the runtime's serialization so the pointer
// arithmetic in pinocchio's `Instructions` accessors is exercised for real.
//
// Layout (agave's `construct_instructions_data`):
//
//   sysvar  := [u16 num_ix] [u16 offset; num_ix] <ix blobs> [u16 current_index]
//   ix blob := [u16 num_accounts] [{u8 flags, [u8; 32] key} * num_accounts]
//              [[u8; 32] program_id] [u16 data_len] [data]
//   flags   := 0b01 signer | 0b10 writable
//
// `current_index` lives in the *last two bytes*, so `data_len` in the account
// header must match the blob length exactly.

struct TestIx {
    program_id: [u8; 32],
    /// `(key, is_signer, is_writable)`
    accounts: Vec<([u8; 32], bool, bool)>,
    data: Vec<u8>,
}

fn build_instructions_sysvar(ixs: &[TestIx], current_index: u16) -> Vec<u8> {
    let header_len = 2 + 2 * ixs.len();
    let mut offsets = Vec::with_capacity(ixs.len());
    let mut blobs = Vec::with_capacity(ixs.len());
    let mut cursor = header_len;

    for ix in ixs {
        offsets.push(cursor as u16);
        let mut blob = Vec::new();
        blob.extend_from_slice(&(ix.accounts.len() as u16).to_le_bytes());
        for (key, is_signer, is_writable) in &ix.accounts {
            let mut flags = 0u8;
            if *is_signer {
                flags |= 0b01;
            }
            if *is_writable {
                flags |= 0b10;
            }
            blob.push(flags);
            blob.extend_from_slice(key);
        }
        blob.extend_from_slice(&ix.program_id);
        blob.extend_from_slice(&(ix.data.len() as u16).to_le_bytes());
        blob.extend_from_slice(&ix.data);
        cursor += blob.len();
        blobs.push(blob);
    }

    let mut out = Vec::with_capacity(cursor + 2);
    out.extend_from_slice(&(ixs.len() as u16).to_le_bytes());
    for offset in &offsets {
        out.extend_from_slice(&offset.to_le_bytes());
    }
    for blob in &blobs {
        out.extend_from_slice(blob);
    }
    out.extend_from_slice(&current_index.to_le_bytes());
    out
}

fn instructions_sysvar_id() -> [u8; 32] {
    pinocchio::sysvars::instructions::INSTRUCTIONS_ID.to_bytes()
}

fn sample_instructions() -> Vec<TestIx> {
    vec![
        TestIx {
            program_id: [0xAA; 32],
            accounts: vec![([0x11; 32], true, true), ([0x22; 32], false, false)],
            data: vec![1, 2, 3, 4],
        },
        TestIx {
            program_id: PROGRAM_ID,
            accounts: vec![([0x33; 32], false, true)],
            data: vec![9],
        },
    ]
}

#[test]
fn sysvar_instructions_load_rejects_wrong_address() {
    // The address gate must reject before the data borrow, exactly like
    // `Sysvar<Clock>` — a non-instructions account never reaches
    // `SysvarLoad::read`.
    let buf = AccountBuffer::<512>::new();
    let blob = build_instructions_sysvar(&sample_instructions(), 0);
    buf.init([0x01; 32], [0u8; 32], blob.len(), false, false, false);
    buf.write_data(&blob);
    let view = unsafe { buf.view() };
    let err = expect_err(Sysvar::<SysvarInstructions>::load(view));
    assert_eq!(err, ProgramError::InvalidArgument);
}

#[test]
fn sysvar_instructions_reads_synthetic_blob() {
    let buf = AccountBuffer::<512>::new();
    let blob = build_instructions_sysvar(&sample_instructions(), 1);
    buf.init(
        instructions_sysvar_id(),
        [0u8; 32],
        blob.len(),
        false,
        false,
        false,
    );
    buf.write_data(&blob);
    let view = unsafe { buf.view() };
    let sysvar = Sysvar::<SysvarInstructions>::load(view).unwrap();

    assert_eq!(sysvar.num_instructions(), 2);
    assert_eq!(sysvar.load_current_index(), 1);

    let first = sysvar.load_instruction_at(0).unwrap();
    assert_eq!(first.get_program_id().to_bytes(), [0xAA; 32]);
    assert_eq!(first.get_instruction_data(), &[1, 2, 3, 4]);
    assert_eq!(first.num_account_metas(), 2);

    let signer = first.get_instruction_account_at(0).unwrap();
    assert_eq!(signer.key.to_bytes(), [0x11; 32]);
    assert!(signer.is_signer());
    assert!(signer.is_writable());

    let readonly = first.get_instruction_account_at(1).unwrap();
    assert_eq!(readonly.key.to_bytes(), [0x22; 32]);
    assert!(!readonly.is_signer());
    assert!(!readonly.is_writable());

    // `current_index` is 1, so relative 0 is the second instruction and
    // relative -1 walks back to the first.
    let current = sysvar.get_instruction_relative(0).unwrap();
    assert_eq!(current.get_program_id().to_bytes(), PROGRAM_ID);
    assert_eq!(current.get_instruction_data(), &[9]);

    let previous = sysvar.get_instruction_relative(-1).unwrap();
    assert_eq!(previous.get_program_id().to_bytes(), [0xAA; 32]);
}

#[test]
fn sysvar_instructions_rejects_out_of_range_index() {
    let buf = AccountBuffer::<512>::new();
    let blob = build_instructions_sysvar(&sample_instructions(), 0);
    buf.init(
        instructions_sysvar_id(),
        [0u8; 32],
        blob.len(),
        false,
        false,
        false,
    );
    buf.write_data(&blob);
    let view = unsafe { buf.view() };
    let sysvar = Sysvar::<SysvarInstructions>::load(view).unwrap();

    assert_eq!(
        expect_err(sysvar.load_instruction_at(2)),
        ProgramError::InvalidInstructionData
    );
    // `current_index` is 0, so there is no preceding instruction.
    assert_eq!(
        expect_err(sysvar.get_instruction_relative(-1)),
        ProgramError::InvalidInstructionData
    );
}

#[test]
fn sysvar_instructions_holds_a_shared_borrow_not_an_exclusive_one() {
    // The wrapper keeps a `Ref` alive for its whole lifetime. That must stay a
    // *shared* borrow: `program.rs` calls `check_borrow()` before a readonly
    // CPI, and an exclusive marker would break passing the sysvar through.
    let buf = AccountBuffer::<512>::new();
    let blob = build_instructions_sysvar(&sample_instructions(), 0);
    buf.init(
        instructions_sysvar_id(),
        [0u8; 32],
        blob.len(),
        false,
        false,
        false,
    );
    buf.write_data(&blob);
    let view = unsafe { buf.view() };
    let sysvar = Sysvar::<SysvarInstructions>::load(view).unwrap();

    assert!(sysvar.account().check_borrow().is_ok());
    assert!(sysvar.account().check_borrow_mut().is_err());

    // Dropping the wrapper releases the guard.
    drop(sysvar);
    let view = unsafe { buf.view() };
    assert!(view.check_borrow_mut().is_ok());
}

#[cfg(feature = "guardrails")]
#[test]
fn sysvar_instructions_rejects_undersized_data() {
    // A genuine sysvar is always at least `[u16 num][u16 current_index]`. The
    // guardrails check stops a truncated mock from underflowing the pointer
    // arithmetic in `load_current_index`.
    let buf = AccountBuffer::<128>::new();
    buf.init(instructions_sysvar_id(), [0u8; 32], 2, false, false, false);
    buf.write_data(&[0u8; 2]);
    let view = unsafe { buf.view() };
    let err = expect_err(Sysvar::<SysvarInstructions>::load(view));
    assert_eq!(err, ProgramError::AccountDataTooSmall);
}

// -- Account<T> / Slab<H, HeaderOnly> ----------------------------------

#[test]
fn account_load_accepts_valid_owner_and_discriminator() {
    let mut buf = AccountBuffer::<128>::new();
    setup_pod_counter_buf(&mut buf, PROGRAM_ID, false, 17);
    let view = unsafe { buf.view() };
    let acct = Account::<PodCounter>::load(view).unwrap();
    assert_eq!(acct.value, 17);
}

#[test]
fn account_load_uses_short_discriminator_len_as_data_offset() {
    assert_eq!(
        <ShortDiscBytes as SlabSchema>::DATA_OFFSET,
        ShortDiscBytes::DISCRIMINATOR.len()
    );
    assert_eq!(
        <ShortDiscBytes as SlabSchema>::MIN_DATA_LEN,
        ShortDiscBytes::DISCRIMINATOR.len() + core::mem::size_of::<ShortDiscBytes>()
    );

    let buf = AccountBuffer::<128>::new();
    buf.init(
        [0x46; 32],
        PROGRAM_ID,
        ShortDiscBytes::DISCRIMINATOR.len() + core::mem::size_of::<ShortDiscBytes>() + 4,
        false,
        false,
        false,
    );
    let mut data = [0u8; 12];
    data[..ShortDiscBytes::DISCRIMINATOR.len()].copy_from_slice(ShortDiscBytes::DISCRIMINATOR);
    data[4..8].copy_from_slice(&[1, 2, 3, 4]);
    data[8..12].copy_from_slice(&[9, 9, 9, 9]);
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let acct = Account::<ShortDiscBytes>::load(view).unwrap();
    assert_eq!(acct.value, [1, 2, 3, 4]);
}

#[test]
fn account_load_uses_long_discriminator_len_as_data_offset() {
    assert_eq!(
        <LongDiscCounter as SlabSchema>::DATA_OFFSET,
        LongDiscCounter::DISCRIMINATOR.len()
    );
    assert_eq!(
        <LongDiscCounter as SlabSchema>::MIN_DATA_LEN,
        LongDiscCounter::DISCRIMINATOR.len() + core::mem::size_of::<LongDiscCounter>()
    );

    let buf = AccountBuffer::<128>::new();
    buf.init(
        [0x47; 32],
        PROGRAM_ID,
        LongDiscCounter::DISCRIMINATOR.len() + core::mem::size_of::<LongDiscCounter>(),
        false,
        false,
        false,
    );
    let mut data = [0u8; 24];
    data[..LongDiscCounter::DISCRIMINATOR.len()].copy_from_slice(LongDiscCounter::DISCRIMINATOR);
    data[16..24].copy_from_slice(&123u64.to_le_bytes());
    buf.write_data(&data);

    let view = unsafe { buf.view() };
    let acct = Account::<LongDiscCounter>::load(view).unwrap();
    assert_eq!(acct.value, 123);
}

#[cfg(feature = "guardrails")]
#[test]
fn account_load_mut_rejects_non_writable() {
    let mut buf = AccountBuffer::<128>::new();
    setup_pod_counter_buf(&mut buf, PROGRAM_ID, false, 17);
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { Account::<PodCounter>::load_mut(view) });
    assert_eq!(err, ErrorCode::ConstraintMut.into());
}

#[test]
#[should_panic(
    expected = "Tried to mutate `Slab<H, T>` through a read-only load"
)]
fn account_deref_mut_panics_when_loaded_read_only() {
    let mut buf = AccountBuffer::<128>::new();
    setup_pod_counter_buf(&mut buf, PROGRAM_ID, false, 17);
    let view = unsafe { buf.view() };
    let mut acct = Account::<PodCounter>::load(view).unwrap();
    acct.value = 18;
}

// -- BorshAccount<T> ---------------------------------------------------

#[test]
fn borsh_account_load_accepts_valid_owner_and_discriminator() {
    let mut buf = AccountBuffer::<128>::new();
    setup_borsh_counter_buf(&mut buf, PROGRAM_ID, false, 9);
    let view = unsafe { buf.view() };
    let acct = BorshAccount::<Counter>::load(view).unwrap();
    assert_eq!(acct.value, 9);
}

#[cfg(feature = "guardrails")]
#[test]
fn borsh_account_load_mut_rejects_non_writable() {
    let mut buf = AccountBuffer::<128>::new();
    setup_borsh_counter_buf(&mut buf, PROGRAM_ID, false, 9);
    let view = unsafe { buf.view() };
    let err = expect_err(unsafe { BorshAccount::<Counter>::load_mut(view) });
    assert_eq!(err, ErrorCode::ConstraintMut.into());
}

#[test]
fn borsh_account_load_rejects_wrong_owner() {
    let mut buf = AccountBuffer::<128>::new();
    setup_borsh_counter_buf(&mut buf, [0x99; 32], true, 9);
    let view = unsafe { buf.view() };
    let err = expect_err(BorshAccount::<Counter>::load(view));
    assert_eq!(err, ProgramError::IllegalOwner);
}

#[test]
fn borsh_account_load_mut_accepts_writable_account() {
    let mut buf = AccountBuffer::<128>::new();
    setup_borsh_counter_buf(&mut buf, PROGRAM_ID, true, 9);
    let view = unsafe { buf.view() };
    let acct = unsafe { BorshAccount::<Counter>::load_mut(view) }.unwrap();
    assert_eq!(acct.value, 9);
}

#[test]
#[should_panic(expected = "use #[account(mut)] for mutable access")]
fn borsh_account_deref_mut_panics_when_loaded_read_only() {
    let mut buf = AccountBuffer::<128>::new();
    setup_borsh_counter_buf(&mut buf, PROGRAM_ID, false, 9);
    let view = unsafe { buf.view() };
    let mut acct = BorshAccount::<Counter>::load(view).unwrap();
    acct.value = 10;
}
