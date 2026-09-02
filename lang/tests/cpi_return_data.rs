use anchor_lang::{__private::CpiReturnData, solana_program::pubkey::Pubkey};

#[test]
fn test_cpi_return_data_snapshot_keeps_original_bytes() {
    let program_id = Pubkey::new_unique();
    let original = 10u64;
    let original_bytes = borsh::to_vec(&original).unwrap();
    let snapshot = CpiReturnData::new(Some((program_id, original_bytes.clone())));

    let spoofed = 999u64;
    let spoofed_bytes = borsh::to_vec(&spoofed).unwrap();
    let later = CpiReturnData::new(Some((program_id, spoofed_bytes.clone())));

    let (snapshot_program_id, snapshot_bytes) = snapshot.return_data().unwrap();
    assert_eq!(snapshot_program_id, program_id);
    assert_eq!(snapshot_bytes, original_bytes.as_slice());

    let (later_program_id, later_bytes) = later.return_data().unwrap();
    assert_eq!(later_program_id, program_id);
    assert_eq!(later_bytes, spoofed_bytes.as_slice());

    assert_eq!(snapshot.get::<u64>(program_id), original);
    assert_eq!(later.get::<u64>(program_id), spoofed);
}

#[test]
#[should_panic]
fn test_cpi_return_data_snapshot_rejects_program_id_mismatch() {
    let program_id = Pubkey::new_unique();
    let other_program_id = Pubkey::new_unique();
    let value = 10u64;
    let snapshot = CpiReturnData::new(Some((other_program_id, borsh::to_vec(&value).unwrap())));

    let _ = snapshot.get::<u64>(program_id);
}
