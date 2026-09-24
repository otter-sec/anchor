use {anchor_lang::solana_program::pubkey::Pubkey, core::str::FromStr};

mod id {
    anchor_lang::declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");
}

#[test]
fn test_declare_id() {
    let good = Pubkey::from_str("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS").unwrap();
    let bad = Pubkey::from_str("A7yUYJNEVYRLE4QWsnc9rE9JRsm7DfqEmLscQVwkffAk").unwrap();
    assert_eq!(good, id::ID);
    assert_eq!(good, id::id());
    assert!(id::check_id(&good));
    assert!(!id::check_id(&bad));
}

mod pk {
    pub(super) const PUBKEY: anchor_lang::solana_program::pubkey::Pubkey =
        anchor_lang::pubkey!("A7yUYJNEVYRLE4QWsnc9rE9JRsm7DfqEmLscQVwkffAk");
}

#[test]
fn test_pubkey() {
    let want = Pubkey::from_str("A7yUYJNEVYRLE4QWsnc9rE9JRsm7DfqEmLscQVwkffAk");
    assert_eq!(want.unwrap(), pk::PUBKEY);
}

#[test]
fn test_slot_hashes_stays_exported_from_prelude() {
    fn accepts_slot_hashes(_: Option<anchor_lang::prelude::SlotHashes>) {}

    accepts_slot_hashes(None);
}
