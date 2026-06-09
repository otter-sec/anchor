use anchor_lang::prelude::*;

declare_program!(external);

#[test]
fn client_to_account_metas() {
    let authority = Pubkey::new_unique();
    let accounts = external::client::accounts::Init {
        authority,
        my_account: Pubkey::find_program_address(&[authority.as_ref()], &external::ID).0,
        system_program: system_program::ID,
    };

    // Keep signer
    assert!(accounts.to_account_metas(None)[0].is_signer);
    assert!(accounts.to_account_metas(Some(true))[0].is_signer);

    // Remove signer (test https://github.com/otter-sec/anchor/pull/3322)
    assert!(!accounts.to_account_metas(Some(false))[0].is_signer);
}
