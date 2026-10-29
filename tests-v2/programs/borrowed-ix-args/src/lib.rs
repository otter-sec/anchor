//! Instruction args borrowed straight from the SBF input region.

use anchor_lang::prelude::*;

declare_id!("3ZQUcsU4hkY7erFj7FMyvmXufuqrcK6AVPWSwz2bDM98");

/// Start of the VM's serialized-input region. The heap lives below it
/// (`0x3_0000_0000`), so a borrow decoded in place points in here.
const MM_INPUT_START: u64 = 0x4_0000_0000;
const MM_REGION_SIZE: u64 = 1 << 32;

#[repr(C)]
#[derive(AnchorSerialize, AnchorDeserialize, IdlType, Clone, Copy)]
pub struct Entry {
    pub owner: Address,
    pub amount: [u8; 8],
}

#[derive(AnchorSerialize, AnchorDeserialize, IdlType)]
pub struct BatchArgs<'a> {
    pub a: u32,
    pub some_data: &'a [u8],
    pub entries: &'a [Entry],
}

#[repr(C)]
#[derive(AnchorSerialize, AnchorDeserialize, IdlType, Clone, Copy)]
pub struct WideEntry {
    pub owner: Address,
    pub amount: u64,
}

#[error_code]
pub enum BorrowError {
    NotBorrowedFromInput,
}

fn require_in_input<T>(slice: &[T]) -> Result<()> {
    let addr = slice.as_ptr() as u64;
    require!(
        (MM_INPUT_START..MM_INPUT_START + MM_REGION_SIZE).contains(&addr),
        BorrowError::NotBorrowedFromInput
    );
    Ok(())
}

/// `a + len(some_data) + sum(entries.amount)`, so the client can check every
/// field made it through.
fn digest(a: u32, some_data: &[u8], entries: &[Entry]) -> u64 {
    entries
        .iter()
        .fold(a as u64 + some_data.len() as u64, |acc, entry| {
            acc.wrapping_add(u64::from_le_bytes(entry.amount))
        })
}

#[program]
pub mod borrowed_ix_args {
    use super::*;

    pub fn submit_batch(_ctx: &mut Context<Noop>, args: BatchArgs<'_>) -> Result<u64> {
        require_in_input(args.some_data)?;
        require_in_input(args.entries)?;
        Ok(digest(args.a, args.some_data, args.entries))
    }

    pub fn submit_flat(
        _ctx: &mut Context<Noop>,
        a: u32,
        some_data: &[u8],
        entries: &[Entry],
    ) -> Result<u64> {
        require_in_input(some_data)?;
        require_in_input(entries)?;
        Ok(digest(a, some_data, entries))
    }

    pub fn submit_wide(_ctx: &mut Context<Noop>, entries: &[WideEntry]) -> Result<u64> {
        Ok(entries.iter().map(|entry| entry.amount).sum())
    }
}

#[derive(Accounts)]
pub struct Noop {}
