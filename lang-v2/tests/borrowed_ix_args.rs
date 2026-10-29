//! Instruction args that borrow straight from the entrypoint's instruction
//! data instead of being copied onto the heap.
//!
//! Run: `cargo test -p anchor-lang --test borrowed_ix_args`

// `unused_variables`: `exit_accounts` (from the `Accounts` derive) decodes
// `ix`, but no exit reads it.
#![allow(unexpected_cfgs, unused_variables)]

use {
    anchor_lang::{prelude::*, ErrorCode, InstructionData, TryAccounts},
    core::{
        cell::{Cell, RefCell},
        mem::size_of,
        ops::Range,
        ptr,
    },
    solana_program_error::ProgramError,
};

declare_id!("11111111111111111111111111111111");

#[derive(AnchorSerialize, AnchorDeserialize, IdlType)]
pub struct MyInstruction<'a> {
    pub a: u32,
    pub some_data: &'a [u8],
}

/// `#[repr(C)]`, padding-free and built from alignment-1 fields, so wincode
/// derives `ZeroCopy` for it and `&[Entry]` can be cast in place at any
/// offset in the instruction data.
#[repr(C)]
#[derive(AnchorSerialize, AnchorDeserialize, IdlType, Clone, Copy, Debug, PartialEq)]
pub struct Entry {
    pub owner: Address,
    pub amount: [u8; 8],
    pub flag: u8,
}

#[derive(AnchorSerialize, AnchorDeserialize, IdlType)]
pub struct Batch<'a> {
    pub a: u32,
    pub entries: &'a [Entry],
}

/// Zero-copy eligible too, but `u64` needs 8-byte alignment, which the
/// position inside the instruction data doesn't guarantee.
#[repr(C)]
#[derive(AnchorSerialize, AnchorDeserialize, IdlType, Clone, Copy)]
pub struct WideEntry {
    pub owner: Address,
    pub amount: u64,
}

#[derive(Accounts)]
pub struct Noop {}

#[derive(Accounts)]
#[instruction(ix: MyInstruction<'_>)]
pub struct WithBorrowedArgs {}

/// What the handler saw: `a`, plus the address range of the borrowed slice.
#[derive(Clone, Debug, PartialEq)]
struct Seen {
    a: u32,
    data: Range<usize>,
}

std::thread_local! {
    static SEEN: Cell<Option<(u32, usize, usize)>> = const { Cell::new(None) };
    static SEEN_ENTRIES: RefCell<Vec<Entry>> = const { RefCell::new(Vec::new()) };
}

fn record<T>(a: u32, data: &[T]) {
    SEEN.with(|seen| {
        seen.set(Some((
            a,
            data.as_ptr() as usize,
            core::mem::size_of_val(data),
        )))
    });
}

fn record_entries(a: u32, entries: &[Entry]) {
    record(a, entries);
    SEEN_ENTRIES.with(|seen| *seen.borrow_mut() = entries.to_vec());
}

fn take_seen() -> Seen {
    let (a, start, len) = SEEN.with(|seen| seen.take()).expect("handler did not run");
    Seen {
        a,
        data: start..start + len,
    }
}

#[program]
pub mod borrowed_ix_args {
    use super::*;

    pub fn structured(_ctx: &mut Context<Noop>, ix: MyInstruction<'_>) -> Result<()> {
        record(ix.a, ix.some_data);
        Ok(())
    }

    pub fn flat(_ctx: &mut Context<Noop>, a: u32, some_data: &[u8], label: &str) -> Result<()> {
        require!(label == "borrowed", ErrorCode::ConstraintRaw);
        record(a, some_data);
        Ok(())
    }

    pub fn with_accounts_args(
        _ctx: &mut Context<WithBorrowedArgs>,
        ix: MyInstruction<'_>,
    ) -> Result<()> {
        record(ix.a, ix.some_data);
        Ok(())
    }

    pub fn submit_batch(_ctx: &mut Context<Noop>, ix: Batch<'_>) -> Result<()> {
        record_entries(ix.a, ix.entries);
        Ok(())
    }

    pub fn entries_flat(_ctx: &mut Context<Noop>, entries: &[Entry]) -> Result<()> {
        record_entries(0, entries);
        Ok(())
    }

    pub fn wide(_ctx: &mut Context<Noop>, entries: &[WideEntry]) -> Result<()> {
        record(0, entries);
        Ok(())
    }

    pub fn wide_after_u32(_ctx: &mut Context<Noop>, a: u32, entries: &[WideEntry]) -> Result<()> {
        record(a, entries);
        Ok(())
    }
}

/// SIMD-0321 instruction-data region: `[len: u64][data][program_id]`.
/// Returns the 8-byte-aligned backing plus the address range of `data`.
fn build_ix_buffer(data: &[u8]) -> (Vec<u64>, Range<usize>) {
    let mut bytes = Vec::with_capacity(size_of::<u64>() + data.len() + 32);
    bytes.extend_from_slice(&(data.len() as u64).to_le_bytes());
    bytes.extend_from_slice(data);
    bytes.extend_from_slice(&crate::ID.to_bytes());

    let mut backing = vec![0u64; bytes.len().div_ceil(8)];
    unsafe {
        ptr::copy_nonoverlapping(bytes.as_ptr(), backing.as_mut_ptr() as *mut u8, bytes.len());
    }
    let start = backing.as_ptr() as usize + size_of::<u64>();
    (backing, start..start + data.len())
}

fn dispatch(data: &[u8]) -> (u64, Range<usize>) {
    // Zero declared accounts: the input region is just `num_accounts = 0`.
    let mut input = vec![0u64];
    let (ix_buf, ix_range) = build_ix_buffer(data);
    let result = unsafe {
        crate::__anchor_dispatch(
            input.as_mut_ptr() as *mut u8,
            ix_buf.as_ptr().cast::<u8>().add(size_of::<u64>()),
        )
    };
    (result, ix_range)
}

fn did_not_deserialize() -> u64 {
    ProgramError::from(ErrorCode::InstructionDidNotDeserialize).into()
}

fn assert_borrows_from(seen: &Seen, ix_range: &Range<usize>, offset: usize, payload: &[u8]) {
    assert_eq!(
        seen.data.start,
        ix_range.start + offset,
        "borrowed slice must point into the instruction data, not a copy"
    );
    assert_eq!(seen.data.len(), payload.len());
    assert!(seen.data.end <= ix_range.end);
}

#[test]
fn struct_arg_borrows_from_instruction_data() {
    let payload = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE];
    let data = instruction::Structured {
        ix: MyInstruction {
            a: 7,
            some_data: &payload,
        },
    }
    .data();

    let (result, ix_range) = dispatch(&data);
    assert_eq!(result, 0);

    let seen = take_seen();
    assert_eq!(seen.a, 7);
    // disc (8) + a: u32 (4) + slice length prefix: u32 (4)
    assert_borrows_from(&seen, &ix_range, 8 + 4 + 4, &payload);
}

#[test]
fn flat_args_borrow_from_instruction_data() {
    let payload = [1, 2, 3];
    let data = instruction::Flat {
        a: 42,
        some_data: &payload,
        label: "borrowed",
    }
    .data();

    let (result, ix_range) = dispatch(&data);
    assert_eq!(result, 0);

    let seen = take_seen();
    assert_eq!(seen.a, 42);
    assert_borrows_from(&seen, &ix_range, 8 + 4 + 4, &payload);
}

#[test]
fn accounts_instruction_attr_accepts_borrowed_struct() {
    let payload = [9; 16];
    let data = instruction::WithAccountsArgs {
        ix: MyInstruction {
            a: 3,
            some_data: &payload,
        },
    }
    .data();

    let (result, ix_range) = dispatch(&data);
    assert_eq!(result, 0);

    let seen = take_seen();
    assert_eq!(seen.a, 3);
    assert_borrows_from(&seen, &ix_range, 8 + 4 + 4, &payload);

    // `#[instruction(...)]` args handed back by `try_accounts` borrow too.
    let args = &data[8..];
    let (_, _, (ix,)) = WithBorrowedArgs::try_accounts(&crate::ID, &[], None, 0, args)
        .expect("accounts-side args should deserialize");
    assert_eq!(ix.a, 3);
    assert_eq!(ix.some_data.as_ptr(), args[8..].as_ptr());
}

#[test]
fn empty_borrowed_slice_is_accepted() {
    let data = instruction::Structured {
        ix: MyInstruction {
            a: 1,
            some_data: &[],
        },
    }
    .data();

    let (result, _) = dispatch(&data);
    assert_eq!(result, 0);
    assert_eq!(take_seen().data.len(), 0);
}

#[test]
fn truncated_borrowed_slice_is_rejected() {
    let payload = [0x11; 8];
    let mut data = instruction::Structured {
        ix: MyInstruction {
            a: 1,
            some_data: &payload,
        },
    }
    .data();
    data.truncate(data.len() - 1);

    let (result, _) = dispatch(&data);
    assert_eq!(result, did_not_deserialize());
    assert!(SEEN.with(|seen| seen.take()).is_none());
}

#[test]
fn oversized_length_prefix_is_rejected() {
    let mut data = instruction::Structured {
        ix: MyInstruction {
            a: 1,
            some_data: &[0x22; 4],
        },
    }
    .data();
    // Claim far more bytes than the instruction actually carries.
    data[12..16].copy_from_slice(&u32::MAX.to_le_bytes());

    let (result, _) = dispatch(&data);
    assert_eq!(result, did_not_deserialize());
    assert!(SEEN.with(|seen| seen.take()).is_none());
}

fn sample_entries() -> [Entry; 3] {
    [
        Entry {
            owner: Address::new_from_array([1; 32]),
            amount: 100u64.to_le_bytes(),
            flag: 0,
        },
        Entry {
            owner: Address::new_from_array([2; 32]),
            amount: 200u64.to_le_bytes(),
            flag: 1,
        },
        Entry {
            owner: Address::new_from_array([3; 32]),
            amount: u64::MAX.to_le_bytes(),
            flag: 0xFF,
        },
    ]
}

fn take_seen_entries() -> Vec<Entry> {
    SEEN_ENTRIES.with(|seen| core::mem::take(&mut *seen.borrow_mut()))
}

#[test]
fn struct_slice_field_borrows_from_instruction_data() {
    let entries = sample_entries();
    let data = instruction::SubmitBatch {
        ix: Batch {
            a: 5,
            entries: &entries,
        },
    }
    .data();

    let (result, ix_range) = dispatch(&data);
    assert_eq!(result, 0);

    let seen = take_seen();
    assert_eq!(seen.a, 5);
    // disc (8) + a: u32 (4) + slice length prefix: u32 (4)
    assert_eq!(seen.data.start, ix_range.start + 8 + 4 + 4);
    assert_eq!(seen.data.len(), entries.len() * size_of::<Entry>());
    assert!(seen.data.end <= ix_range.end);
    assert_eq!(take_seen_entries(), entries);
}

#[test]
fn flat_struct_slice_borrows_from_instruction_data() {
    let entries = sample_entries();
    let data = instruction::EntriesFlat { entries: &entries }.data();

    let (result, ix_range) = dispatch(&data);
    assert_eq!(result, 0);

    let seen = take_seen();
    // disc (8) + slice length prefix: u32 (4). Not 8-byte aligned, which is
    // fine for an alignment-1 element type.
    assert_eq!(seen.data.start, ix_range.start + 8 + 4);
    assert_eq!(seen.data.len(), entries.len() * size_of::<Entry>());
    assert_eq!(take_seen_entries(), entries);
}

#[test]
fn struct_slice_wire_format_matches_owned_vec() {
    let entries = sample_entries();
    let borrowed = instruction::EntriesFlat { entries: &entries }.data();

    let mut owned = instruction::EntriesFlat::DISCRIMINATOR.to_vec();
    owned.extend(
        anchor_lang::wincode::config::serialize(&entries.to_vec(), anchor_lang::BORSH_CONFIG)
            .unwrap(),
    );
    assert_eq!(borrowed, owned);
}

#[test]
fn truncated_struct_slice_is_rejected() {
    let entries = sample_entries();
    let mut data = instruction::EntriesFlat { entries: &entries }.data();
    // Drop half of the last element: length prefix still claims 3.
    data.truncate(data.len() - size_of::<Entry>() / 2);

    let (result, _) = dispatch(&data);
    assert_eq!(result, did_not_deserialize());
    assert!(SEEN.with(|seen| seen.take()).is_none());
}

#[test]
fn misaligned_wide_struct_slice_is_rejected() {
    let entries = [WideEntry {
        owner: Address::new_from_array([4; 32]),
        amount: 1,
    }];
    let data = instruction::Wide { entries: &entries }.data();

    // Elements land at disc (8) + length prefix (4) = 12, which is not a
    // multiple of `align_of::<u64>()`, so wincode refuses to cast in place.
    let (result, _) = dispatch(&data);
    assert_eq!(result, did_not_deserialize());
    assert!(SEEN.with(|seen| seen.take()).is_none());
}

#[test]
fn wide_struct_slice_decodes_when_preceding_args_align_it() {
    let entries = [
        WideEntry {
            owner: Address::new_from_array([5; 32]),
            amount: 1,
        },
        WideEntry {
            owner: Address::new_from_array([6; 32]),
            amount: u64::MAX,
        },
    ];
    let data = instruction::WideAfterU32 {
        a: 9,
        entries: &entries,
    }
    .data();

    // disc (8) + a: u32 (4) + length prefix (4) = 16: u64-aligned, so the
    // same element type that `misaligned_wide_struct_slice_is_rejected`
    // refuses decodes here. Alignment depends on the args before the slice.
    let (result, ix_range) = dispatch(&data);
    assert_eq!(result, 0);

    let seen = take_seen();
    assert_eq!(seen.a, 9);
    assert_eq!(seen.data.start, ix_range.start + 8 + 4 + 4);
    assert_eq!(seen.data.start % core::mem::align_of::<WideEntry>(), 0);
    assert_eq!(seen.data.len(), entries.len() * size_of::<WideEntry>());
}
