use {
    anchor_lang::{
        cpi::{realloc_account, rent_exempt_lamports},
        testing::AccountBuffer,
    },
    solana_program_error::ProgramError,
};

const PROGRAM_ID: [u8; 32] = [0x42; 32];

#[test]
fn realloc_rejects_underfunded_growth_with_target_as_payer() {
    let buf = AccountBuffer::<256>::new();
    buf.init([0xAB; 32], PROGRAM_ID, 24, false, true, false);
    buf.set_lamports(rent_exempt_lamports(24).unwrap());

    let mut account = unsafe { buf.view() };
    let payer = account;
    let old_space = account.data_len();
    let old_lamports = account.lamports();

    let err = realloc_account(&mut account, 64, &payer, false)
        .expect_err("realloc must reject using the target account as the payer");

    assert_eq!(err, ProgramError::InvalidArgument);
    assert_eq!(account.data_len(), old_space);
    assert_eq!(account.lamports(), old_lamports);
}

#[test]
fn realloc_allows_funded_growth_with_target_as_payer() {
    let buf = AccountBuffer::<256>::new();
    buf.init([0xAB; 32], PROGRAM_ID, 24, false, true, false);
    let new_space = 64;
    let funded_lamports = rent_exempt_lamports(new_space).unwrap();
    buf.set_lamports(funded_lamports);

    let mut account = unsafe { buf.view() };
    let payer = account;

    realloc_account(&mut account, new_space, &payer, false).unwrap();

    assert_eq!(account.data_len(), new_space);
    assert_eq!(account.lamports(), funded_lamports);
}
