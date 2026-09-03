//! Hand-rolled pinocchio counter — the perf floor for this bench.
//! Borrows `find_program_address` and `create_account_signed` from
//! anchor-lang as plain pinocchio utilities.

use {
    anchor_lang::{create_account_signed, find_program_address},
    pinocchio::{
        account::AccountView, address::Address, no_allocator, program_entrypoint, ProgramResult,
    },
};

// Shared helloworld id — base58 `B7ihZyo...`.
// The line below is scanned by `anchor debugger`'s program discovery to map
// this .so's runtime id without depending on anchor's `declare_id!` macro.
// declare_id!("B7ihZyoXZ1fwAY3TugkiFJ6SXkzJwMuQrxrekBaSmn32")
pub const ID: Address = Address::new_from_array([
    150, 77, 128, 252, 18, 209, 27, 135, 162, 60, 31, 212, 195, 62, 83, 235,
    169, 66, 250, 246, 54, 206, 179, 35, 44, 128, 206, 68, 111, 175, 179, 217,
]);

const COUNTER_DISC: u8 = 1;
const COUNTER_SPACE: usize = 1 + 8 + 1 + 7;

#[cfg(not(feature = "no-entrypoint"))]
program_entrypoint!(process_instruction);
no_allocator!();

pub fn process_instruction(
    program_id: &Address,
    accounts: &mut [AccountView],
    _instruction_data: &[u8],
) -> ProgramResult {
    let payer = unsafe { accounts.get_unchecked(0) };
    let counter = unsafe { accounts.get_unchecked(1) };

    // CreateAccount CPI verifies the passed counter matches the derived PDA.
    let (_pda, bump) = find_program_address(&[b"counter"], program_id);

    let bump_slice = [bump];
    create_account_signed(
        payer,
        counter,
        COUNTER_SPACE,
        program_id,
        &[b"counter", &bump_slice],
    )?;

    // Data layout: [disc: u8][value: u64 LE][bump: u8][_pad: [u8; 7]]
    unsafe {
        let mut counter_mut = *counter;
        let data = counter_mut.borrow_unchecked_mut();
        *data.get_unchecked_mut(0) = COUNTER_DISC;
        data.get_unchecked_mut(1..9).copy_from_slice(&42u64.to_le_bytes());
        *data.get_unchecked_mut(9) = bump;
    }

    Ok(())
}
