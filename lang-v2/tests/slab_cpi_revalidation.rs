use {
    anchor_lang::{
        accounts::{Account, Slab},
        testing::AccountBuffer,
        AnchorAccount, Discriminator, Owner,
    },
    bytemuck::{Pod, Zeroable},
    pinocchio::{account::RuntimeAccount, address::Address},
    solana_program_error::ProgramError,
};

const PROGRAM_ID: [u8; 32] = [0x42; 32];
const FOREIGN_PROGRAM_ID: [u8; 32] = [0x24; 32];

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Counter {
    value: u64,
    bump: u8,
    _pad: [u8; 7],
}

impl Owner for Counter {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}

impl Discriminator for Counter {
    const DISCRIMINATOR: &'static [u8] = &[0xff, 0xb0, 0x04, 0xf5, 0xbc, 0xfd, 0x7c, 0x19];
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ForeignCounter {
    value: u64,
    bump: u8,
    _pad: [u8; 7],
}

impl Owner for ForeignCounter {
    const OWNER: Address = Address::new_from_array(FOREIGN_PROGRAM_ID);
}

impl Discriminator for ForeignCounter {
    const DISCRIMINATOR: &'static [u8] = &[0x23, 0xaa, 0x41, 0x17, 0x83, 0x62, 0xdd, 0x09];
}

type CounterAccount = Account<Counter>;
type CounterLedger = Slab<Counter, [u8; 8]>;

const HEADER_OFFSET: usize = 8;
const LEN_OFFSET: usize = HEADER_OFFSET + core::mem::size_of::<Counter>();
const ITEMS_OFFSET: usize = LEN_OFFSET + 4;
const ITEM_SIZE: usize = core::mem::size_of::<[u8; 8]>();

fn setup_counter_account() -> AccountBuffer<256> {
    let buf = AccountBuffer::<256>::new();
    let data_len = HEADER_OFFSET + core::mem::size_of::<Counter>();
    buf.init([0xAB; 32], PROGRAM_ID, data_len, false, true, false);
    let mut data = [0u8; 256];
    data[..8].copy_from_slice(Counter::DISCRIMINATOR);
    buf.write_data(&data[..data_len]);
    buf
}

fn setup_ledger(capacity: usize, populated_len: u32) -> AccountBuffer<256> {
    let buf = AccountBuffer::<256>::new();
    let data_len = ITEMS_OFFSET + capacity * ITEM_SIZE;
    buf.init([0xAA; 32], PROGRAM_ID, data_len, false, true, false);
    let mut data = [0u8; 256];
    data[..8].copy_from_slice(Counter::DISCRIMINATOR);
    data[LEN_OFFSET..LEN_OFFSET + 4].copy_from_slice(&populated_len.to_le_bytes());
    buf.write_data(&data[..data_len]);
    buf
}

fn set_data_bytes(buf: &AccountBuffer<256>, offset: usize, bytes: &[u8]) {
    let header = core::mem::size_of::<RuntimeAccount>();
    let start = header + offset;
    unsafe {
        let base = buf.raw() as *mut u8;
        core::ptr::copy_nonoverlapping(bytes.as_ptr(), base.add(start), bytes.len());
    }
}

#[test]
fn revalidate_after_cpi_rejects_owner_change() {
    let buf = setup_counter_account();

    let view = unsafe { buf.view() };
    let mut account = unsafe { CounterAccount::load_mut(view) }.unwrap();

    buf.set_owner(FOREIGN_PROGRAM_ID);

    let err = account
        .revalidate_after_cpi()
        .expect_err("owner change must be rejected after CPI");
    assert_eq!(err, ProgramError::IllegalOwner);
}

#[test]
fn revalidate_after_cpi_rejects_discriminator_change() {
    let buf = setup_counter_account();

    let view = unsafe { buf.view() };
    let mut account = unsafe { CounterAccount::load_mut(view) }.unwrap();

    set_data_bytes(&buf, 0, ForeignCounter::DISCRIMINATOR);

    let err = account
        .revalidate_after_cpi()
        .expect_err("discriminator change must be rejected after CPI");
    assert_eq!(err, ProgramError::InvalidAccountData);
}

#[test]
fn revalidate_after_cpi_rejects_tail_len_exceeding_live_capacity() {
    let buf = setup_ledger(/*capacity*/ 4, /*len*/ 3);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CounterLedger::load_mut(view) }.unwrap();

    buf.set_data_len((ITEMS_OFFSET + ITEM_SIZE) as u64);

    let err = slab
        .revalidate_after_cpi()
        .expect_err("tail len > capacity must be rejected after CPI");
    assert_eq!(err, ProgramError::InvalidAccountData);
}

#[test]
fn revalidate_after_cpi_accepts_schema_preserving_tail_mutation() {
    let buf = setup_ledger(/*capacity*/ 2, /*len*/ 1);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CounterLedger::load_mut(view) }.unwrap();

    set_data_bytes(&buf, ITEMS_OFFSET, &[0x11; 8]);

    slab.revalidate_after_cpi().unwrap();
    assert_eq!(slab.len(), 1);
    assert_eq!(slab.capacity(), 2);
}
