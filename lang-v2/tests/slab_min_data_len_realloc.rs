use {
    anchor_lang::{
        accounts::{Account, Slab, SlabSchema},
        testing::AccountBuffer,
        AccountRealloc, AnchorAccount, Space,
    },
    bytemuck::{Pod, Zeroable},
    pinocchio::account::AccountView,
    solana_program_error::ProgramError,
};

const DATA_OFFSET: usize = 8;
const PHYSICAL_MIN_DATA_LEN: usize = DATA_OFFSET + core::mem::size_of::<CustomHeader>();
const DECLARED_MIN_DATA_LEN: usize = PHYSICAL_MIN_DATA_LEN + 16;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct CustomHeader {
    counter: u64,
    bump: u64,
}

impl SlabSchema for CustomHeader {
    const DATA_OFFSET: usize = DATA_OFFSET;
    const MIN_DATA_LEN: usize = DECLARED_MIN_DATA_LEN;

    fn validate(_view: &AccountView, data: &[u8]) -> Result<(), ProgramError> {
        if data.len() < Self::MIN_DATA_LEN {
            return Err(ProgramError::AccountDataTooSmall);
        }
        Ok(())
    }
}

type CustomAccount = Account<CustomHeader>;

fn setup_account(data_len: usize) -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    buf.init([0x44; 32], [0x55; 32], data_len, false, true, false);
    let data = [0u8; 128];
    buf.write_data(&data[..data_len]);
    buf
}

#[test]
fn custom_schema_minimum_exceeds_physical_layout() {
    assert_eq!(PHYSICAL_MIN_DATA_LEN, 24);
    assert_eq!(
        <CustomAccount as AnchorAccount>::MIN_DATA_LEN,
        DECLARED_MIN_DATA_LEN
    );
    assert!(DECLARED_MIN_DATA_LEN > PHYSICAL_MIN_DATA_LEN);
}

#[test]
fn realloc_rejects_shrink_below_custom_schema_minimum() {
    let buf = setup_account(DECLARED_MIN_DATA_LEN);
    let payer = AccountBuffer::<128>::new();
    payer.init([0x99; 32], [0x11; 32], 0, true, true, false);

    let view = unsafe { buf.view() };
    let mut account = unsafe { CustomAccount::load_mut(view) }.unwrap();
    let payer_view = unsafe { payer.view() };

    let err = account
        .realloc_account(PHYSICAL_MIN_DATA_LEN, payer_view, false)
        .expect_err("realloc must reject spaces below the schema minimum");
    assert_eq!(err, ProgramError::AccountDataTooSmall);
    assert_eq!(account.current_space(), DECLARED_MIN_DATA_LEN);
    drop(account);

    CustomAccount::load(unsafe { buf.view() }).expect("account should stay reloadable");
}

type CustomLedger = Slab<CustomHeader, u8>;

// `Slab<CustomHeader, u8>` layout:
//   [disc:8][H (16 bytes)][len:u32]  → ITEMS_OFFSET = 28
// Schema minimum is `DECLARED_MIN_DATA_LEN = 40` — well past ITEMS_OFFSET.
const TAIL_ITEMS_OFFSET: usize = DATA_OFFSET + core::mem::size_of::<CustomHeader>() + 4;

fn setup_ledger(data_len: usize, populated_len: u32) -> AccountBuffer<256> {
    let buf = AccountBuffer::<256>::new();
    buf.init([0x66; 32], [0x55; 32], data_len, false, true, false);
    let mut data = [0u8; 256];
    if data_len >= TAIL_ITEMS_OFFSET {
        let len_offset = DATA_OFFSET + core::mem::size_of::<CustomHeader>();
        data[len_offset..len_offset + 4].copy_from_slice(&populated_len.to_le_bytes());
    }
    buf.write_data(&data[..data_len]);
    buf
}

#[test]
fn tail_slab_min_data_len_floors_at_schema_minimum() {
    // Structural layout on its own is smaller than the schema floor.
    assert!(TAIL_ITEMS_OFFSET < DECLARED_MIN_DATA_LEN);
    // Slab must expose the max of the two, not the raw structural offset.
    assert_eq!(
        <CustomLedger as AnchorAccount>::MIN_DATA_LEN,
        DECLARED_MIN_DATA_LEN
    );
    assert_eq!(<CustomLedger as Space>::INIT_SPACE, DECLARED_MIN_DATA_LEN);
}

#[test]
fn tail_slab_space_for_zero_capacity_floors_at_schema_minimum() {
    // `space_for(0)` used to return `ITEMS_OFFSET`, which is below the
    // schema floor for our custom header. The floor now applies so
    // `#[account(init, space = Slab::<H,T>::space_for(0))]` yields a
    // buffer that H::validate will still accept on the next load.
    assert_eq!(CustomLedger::space_for(0), DECLARED_MIN_DATA_LEN);
    // Small non-zero capacities that would still fall under the schema
    // minimum are floored too.
    assert_eq!(CustomLedger::space_for(4), DECLARED_MIN_DATA_LEN);
    // Once the structural size exceeds the schema minimum, `space_for`
    // tracks the item count as before.
    let big = DECLARED_MIN_DATA_LEN - TAIL_ITEMS_OFFSET + 8;
    assert_eq!(CustomLedger::space_for(big as u32), TAIL_ITEMS_OFFSET + big);
}

#[test]
fn tail_slab_resize_to_zero_capacity_keeps_account_loadable() {
    // Start over-provisioned so the resize is a real shrink.
    let start_len = DECLARED_MIN_DATA_LEN + 32;
    let buf = setup_ledger(start_len, 0);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CustomLedger::load_mut(view) }.unwrap();

    slab.resize_to_capacity(0)
        .expect("resize to 0 must succeed");
    // Shrunk exactly to the schema floor — not to ITEMS_OFFSET.
    assert_eq!(slab.current_space(), DECLARED_MIN_DATA_LEN);
    drop(slab);

    // The whole point: after the resize the account still validates.
    CustomLedger::load(unsafe { buf.view() })
        .expect("tail Slab must stay reloadable after resize_to_capacity(0)");
}

#[test]
fn tail_slab_realloc_rejects_shrink_below_schema_minimum() {
    let buf = setup_ledger(DECLARED_MIN_DATA_LEN, 0);
    let payer = AccountBuffer::<128>::new();
    payer.init([0x99; 32], [0x11; 32], 0, true, true, false);

    let view = unsafe { buf.view() };
    let mut slab = unsafe { CustomLedger::load_mut(view) }.unwrap();
    let payer_view = unsafe { payer.view() };

    // ITEMS_OFFSET is a valid physical layout but under the schema floor.
    let err = slab
        .realloc_account(TAIL_ITEMS_OFFSET, payer_view, false)
        .expect_err("realloc must reject shrink below H::MIN_DATA_LEN");
    assert_eq!(err, ProgramError::AccountDataTooSmall);
    assert_eq!(slab.current_space(), DECLARED_MIN_DATA_LEN);
    drop(slab);

    CustomLedger::load(unsafe { buf.view() })
        .expect("tail Slab must stay reloadable after a rejected realloc");
}
