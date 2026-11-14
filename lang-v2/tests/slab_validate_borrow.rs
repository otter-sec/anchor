use {
    anchor_lang::{
        accounts::{Account, SlabSchema},
        testing::AccountBuffer,
        AnchorAccount,
    },
    bytemuck::{Pod, Zeroable},
    pinocchio::account::AccountView,
    solana_program_error::ProgramError,
};

const DATA_OFFSET: usize = 8;
const DATA_LEN: usize = DATA_OFFSET + core::mem::size_of::<WritingHeader>();

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct WritingHeader {
    value: u64,
}

impl SlabSchema for WritingHeader {
    const DATA_OFFSET: usize = DATA_OFFSET;
    const MIN_DATA_LEN: usize = DATA_LEN;

    // Misbehaving validator: tries to write through an aliased view while the
    // slab's `data` slice is live.
    fn validate(view: &AccountView, _data: &[u8]) -> Result<(), ProgramError> {
        let mut alias = *view;
        let mut data = alias.try_borrow_mut()?;
        data[0] = 0xEE;
        Ok(())
    }
}

type WritingAccount = Account<WritingHeader>;

fn setup_account() -> AccountBuffer<128> {
    let buf = AccountBuffer::<128>::new();
    buf.init([0x44; 32], [0x55; 32], DATA_LEN, false, true, false);
    buf.write_data(&[0u8; DATA_LEN]);
    buf
}

fn assert_released_and_untouched(buf: &AccountBuffer<128>) {
    let mut view = unsafe { buf.view() };
    let data = view
        .try_borrow_mut()
        .expect("a failed load must release its validation borrow");
    assert_eq!(data[0], 0, "validate must not have written the account");
}

#[test]
fn load_blocks_mutable_borrow_during_validate() {
    let buf = setup_account();
    let err = WritingAccount::load(unsafe { buf.view() })
        .err()
        .expect("validate's mutable borrow must fail");
    assert_eq!(err, ProgramError::AccountBorrowFailed);
    assert_released_and_untouched(&buf);
}

#[test]
fn load_mut_blocks_mutable_borrow_during_validate() {
    let buf = setup_account();
    let err = WritingAccount::load_mut(unsafe { buf.view() })
        .err()
        .expect("validate's mutable borrow must fail");
    assert_eq!(err, ProgramError::AccountBorrowFailed);
    assert_released_and_untouched(&buf);
}
