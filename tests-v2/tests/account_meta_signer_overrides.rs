use {anchor_lang::ToAccountMetas, solana_pubkey::Pubkey};

#[test]
fn signer_override_changes_signer_fields_only() {
    let authority = Pubkey::new_unique();
    let accounts = account_meta_signer_overrides::accounts::RequireSigner { authority };

    let default = accounts.to_account_metas(None);
    assert_eq!(default.len(), 1);
    assert_eq!(default[0].pubkey, authority);
    assert!(default[0].is_signer);
    assert!(!default[0].is_writable);

    let overridden = accounts.to_account_metas(Some(false));
    assert_eq!(overridden[0].pubkey, authority);
    assert!(!overridden[0].is_signer);
    assert_eq!(overridden[0].is_writable, default[0].is_writable);

    let forced = accounts.to_account_metas(Some(true));
    assert!(forced[0].is_signer);
}

#[test]
fn signer_override_propagates_through_nested_accounts() {
    let writable = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let spectator = Pubkey::new_unique();
    let accounts = account_meta_signer_overrides::accounts::NestedSignerOuter {
        inner: account_meta_signer_overrides::accounts::NestedSignerInner {
            writable,
            authority,
        },
        spectator,
    };

    let default = accounts.to_account_metas(None);
    assert_eq!(default.len(), 3);
    assert_eq!(default[0].pubkey, writable);
    assert!(default[0].is_writable);
    assert!(!default[0].is_signer);
    assert_eq!(default[1].pubkey, authority);
    assert!(default[1].is_signer);
    assert!(!default[1].is_writable);
    assert_eq!(default[2].pubkey, spectator);
    assert!(!default[2].is_signer);

    let overridden = accounts.to_account_metas(Some(false));
    assert_eq!(overridden.len(), 3);
    assert_eq!(overridden[0].pubkey, writable);
    assert_eq!(overridden[0].is_writable, default[0].is_writable);
    assert!(!overridden[0].is_signer);
    assert_eq!(overridden[1].pubkey, authority);
    assert!(!overridden[1].is_signer);
    assert_eq!(overridden[1].is_writable, default[1].is_writable);
    assert_eq!(overridden[2].pubkey, spectator);
    assert!(!overridden[2].is_signer);
}

#[test]
fn init_keypair_signer_flag_also_respects_override() {
    let payer = Pubkey::new_unique();
    let fresh = Pubkey::new_unique();
    let accounts = account_meta_signer_overrides::accounts::InitKeypair {
        payer,
        fresh,
        system_program: solana_sdk_ids::system_program::ID,
    };

    let default = accounts.to_account_metas(None);
    assert_eq!(default.len(), 3);
    assert!(default[0].is_signer);
    assert!(default[1].is_signer);
    assert!(!default[2].is_signer);

    let overridden = accounts.to_account_metas(Some(false));
    assert!(!overridden[0].is_signer);
    assert!(!overridden[1].is_signer);
    assert!(!overridden[2].is_signer);
    assert_eq!(
        default.iter().map(|m| m.is_writable).collect::<Vec<_>>(),
        overridden.iter().map(|m| m.is_writable).collect::<Vec<_>>()
    );
}
