use {
    anchor_lang::{
        accounts::{Account, UncheckedAccount},
        testing::{AccountRecord, SbfInputBuffer},
        AccountCursor, Accounts, Discriminator, ErrorCode, Nested, Owner, TryAccounts,
    },
    bytemuck::{Pod, Zeroable},
    core::mem::{size_of, MaybeUninit},
    pinocchio::{account::AccountView, address::Address},
    solana_program_error::ProgramError,
};

const PROGRAM_ID: [u8; 32] = [0xAA; 32];
anchor_lang::declare_id!("11111111111111111111111111111111");

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Counter {
    value: u64,
    _pad: [u8; 8],
}
impl Owner for Counter {
    const OWNER: Address = Address::new_from_array(PROGRAM_ID);
}
impl Discriminator for Counter {
    const DISCRIMINATOR: &'static [u8] = &[1; 8];
}

#[derive(Accounts)]
struct TwoMutable {
    #[account(mut)]
    first: Account<Counter>,
    #[account(mut)]
    second: Account<Counter>,
}
#[derive(Accounts)]
struct InnerMutable {
    #[account(mut)]
    first: Account<Counter>,
    #[account(mut)]
    second: Account<Counter>,
}
#[derive(Accounts)]
#[allow(dead_code)]
struct OuterNested {
    prefix: UncheckedAccount,
    inner: Nested<InnerMutable>,
}

fn account(address: [u8; 32], writable: bool) -> AccountRecord {
    AccountRecord::NonDup {
        address,
        owner: PROGRAM_ID,
        lamports: 100,
        is_signer: false,
        is_writable: writable,
        executable: false,
        data_len: 8 + size_of::<Counter>(),
    }
}
fn duplicate_input() -> SbfInputBuffer {
    SbfInputBuffer::build(&[account([1; 32], true), AccountRecord::Dup { index: 0 }])
}
fn nested_input() -> SbfInputBuffer {
    SbfInputBuffer::build(&[
        AccountRecord::NonDup {
            address: [9; 32],
            owner: [0xBB; 32],
            lamports: 100,
            is_signer: false,
            is_writable: false,
            executable: false,
            data_len: 0,
        },
        account([1; 32], true),
        AccountRecord::Dup { index: 1 },
    ])
}

fn parse_at<T: TryAccounts>(
    input: &mut SbfInputBuffer,
    walk: usize,
    start: usize,
    base: usize,
) -> Result<(), ProgramError> {
    let mut lookup = [const { MaybeUninit::<AccountView>::uninit() }; 256];
    let mut cursor = unsafe { AccountCursor::new(input.as_mut_ptr(), lookup.as_mut_ptr().cast()) };
    let (views, duplicates) = unsafe { cursor.walk_n(walk) };
    T::try_accounts(
        &Address::new_from_array(PROGRAM_ID),
        &views[start..start + T::HEADER_SIZE],
        duplicates,
        base,
        &[],
    )
    .map(|_| ())
}
fn duplicate_error() -> ProgramError {
    ErrorCode::ConstraintDuplicateMutableAccount.into()
}

#[test]
fn direct_try_accounts_rejects_duplicate_mutables_before_loading() {
    // The input has zeroed data, while Counter requires a nonzero discriminator;
    // duplicate rejection winning proves it precedes unsafe `load_mut`.
    assert_eq!(
        parse_at::<TwoMutable>(&mut duplicate_input(), 2, 0, 0),
        Err(duplicate_error())
    );
}

#[test]
fn direct_nested_try_accounts_shifts_the_global_duplicate_mask() {
    assert_eq!(
        parse_at::<OuterNested>(&mut nested_input(), 3, 0, 0),
        Err(duplicate_error())
    );
    assert_eq!(
        parse_at::<InnerMutable>(&mut nested_input(), 3, 1, 1),
        Err(duplicate_error())
    );
}
