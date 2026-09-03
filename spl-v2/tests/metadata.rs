#![cfg(feature = "metadata")]

use {
    anchor_lang::{testing::AccountBuffer, AccountDeserialize, AnchorAccount},
    anchor_spl::metadata::{self, MasterEditionAccount, MetadataAccount, TokenRecordAccount},
    borsh::{to_vec, BorshSerialize},
    solana_program_error::ProgramError,
    solana_pubkey::Pubkey,
};

#[derive(BorshSerialize)]
struct LegacyMetadataAccount {
    key: mpl_token_metadata::types::Key,
    update_authority: Pubkey,
    mint: Pubkey,
    data: mpl_token_metadata::types::Data,
    primary_sale_happened: bool,
    is_mutable: bool,
    edition_nonce: Option<u8>,
}

fn sample_metadata() -> mpl_token_metadata::accounts::Metadata {
    let creator = Pubkey::from([7u8; 32]);
    mpl_token_metadata::accounts::Metadata {
        key: mpl_token_metadata::types::Key::MetadataV1,
        update_authority: Pubkey::from([1u8; 32]),
        mint: Pubkey::from([2u8; 32]),
        name: "Pump AMM".to_string(),
        symbol: "PUMP".to_string(),
        uri: "https://example.invalid/pump.json".to_string(),
        seller_fee_basis_points: 250,
        creators: Some(vec![mpl_token_metadata::types::Creator {
            address: creator,
            verified: true,
            share: 100,
        }]),
        primary_sale_happened: false,
        is_mutable: true,
        edition_nonce: Some(255),
        token_standard: None,
        collection: None,
        uses: None,
        collection_details: None,
        programmable_config: None,
    }
}

fn sample_metadata_with_v1_2_fields() -> mpl_token_metadata::accounts::Metadata {
    let mut metadata = sample_metadata();
    metadata.token_standard = Some(mpl_token_metadata::types::TokenStandard::NonFungible);
    metadata.collection = Some(mpl_token_metadata::types::Collection {
        verified: true,
        key: Pubkey::from([11u8; 32]),
    });
    metadata
}

fn sample_metadata_with_programmable_config() -> mpl_token_metadata::accounts::Metadata {
    let mut metadata = sample_metadata_with_v1_2_fields();
    metadata.token_standard =
        Some(mpl_token_metadata::types::TokenStandard::ProgrammableNonFungible);
    metadata.programmable_config = Some(mpl_token_metadata::types::ProgrammableConfig::V1 {
        rule_set: Some(Pubkey::from([12u8; 32])),
    });
    metadata
}

fn sample_metadata_with_collection_without_uses_bytes() -> Vec<u8> {
    let metadata = sample_metadata_with_v1_2_fields();
    let data = mpl_token_metadata::types::Data {
        name: metadata.name,
        symbol: metadata.symbol,
        uri: metadata.uri,
        seller_fee_basis_points: metadata.seller_fee_basis_points,
        creators: metadata.creators,
    };

    let mut bytes = Vec::new();
    bytes.extend_from_slice(&to_vec(&metadata.key).unwrap());
    bytes.extend_from_slice(&to_vec(&metadata.update_authority).unwrap());
    bytes.extend_from_slice(&to_vec(&metadata.mint).unwrap());
    bytes.extend_from_slice(&to_vec(&data).unwrap());
    bytes.extend_from_slice(&to_vec(&metadata.primary_sale_happened).unwrap());
    bytes.extend_from_slice(&to_vec(&metadata.is_mutable).unwrap());
    bytes.extend_from_slice(&to_vec(&metadata.edition_nonce).unwrap());
    bytes.extend_from_slice(&to_vec(&metadata.token_standard).unwrap());
    bytes.extend_from_slice(&to_vec(&metadata.collection).unwrap());
    bytes
}

fn sample_legacy_metadata() -> LegacyMetadataAccount {
    let creator = Pubkey::from([7u8; 32]);
    LegacyMetadataAccount {
        key: mpl_token_metadata::types::Key::MetadataV1,
        update_authority: Pubkey::from([1u8; 32]),
        mint: Pubkey::from([2u8; 32]),
        data: mpl_token_metadata::types::Data {
            name: "Pump AMM".to_string(),
            symbol: "PUMP".to_string(),
            uri: "https://example.invalid/pump.json".to_string(),
            seller_fee_basis_points: 250,
            creators: Some(vec![mpl_token_metadata::types::Creator {
                address: creator,
                verified: true,
                share: 100,
            }]),
        },
        primary_sale_happened: false,
        is_mutable: true,
        edition_nonce: Some(255),
    }
}

fn sample_master_edition() -> mpl_token_metadata::accounts::MasterEdition {
    mpl_token_metadata::accounts::MasterEdition {
        key: mpl_token_metadata::types::Key::MasterEditionV2,
        supply: 42,
        max_supply: Some(100),
    }
}
fn sample_token_record() -> mpl_token_metadata::accounts::TokenRecord {
    mpl_token_metadata::accounts::TokenRecord {
        key: mpl_token_metadata::types::Key::TokenRecord,
        bump: 7,
        state: mpl_token_metadata::types::TokenState::Unlocked,
        rule_set_revision: Some(9),
        delegate: Some(Pubkey::from([3u8; 32])),
        delegate_role: Some(mpl_token_metadata::types::TokenDelegateRole::Transfer),
        locked_transfer: Some(Pubkey::from([4u8; 32])),
    }
}

fn sample_legacy_token_record_bytes() -> Vec<u8> {
    let record = sample_token_record();
    let mut data = Vec::new();
    data.extend_from_slice(&to_vec(&record.key).unwrap());
    data.extend_from_slice(&to_vec(&record.bump).unwrap());
    data.extend_from_slice(&to_vec(&record.state).unwrap());
    data.extend_from_slice(&to_vec(&record.rule_set_revision).unwrap());
    data.extend_from_slice(&to_vec(&record.delegate).unwrap());
    data.extend_from_slice(&to_vec(&record.delegate_role).unwrap());
    data
}
#[test]
fn fixture_is_real_metadata_program_elf() {
    let fixture = include_bytes!("fixtures/metaplex_token_metadata.so");
    assert_eq!(fixture.len(), 283_512);
    assert_eq!(&fixture[..4], b"\x7fELF");
}

#[test]
fn metadata_account_deserializes_raw_metaplex_bytes() {
    let expected = sample_metadata();
    let data = to_vec(&expected).unwrap();
    let account = MetadataAccount::try_deserialize(&mut data.as_slice()).unwrap();

    assert_eq!(&*account, &expected);
}

#[test]
fn metadata_account_deserialize_advances_cursor() {
    let data = to_vec(&sample_metadata()).unwrap();
    let mut cursor = data.as_slice();
    let account = MetadataAccount::try_deserialize(&mut cursor).unwrap();

    assert_eq!(account.key, mpl_token_metadata::types::Key::MetadataV1);
    assert!(cursor.is_empty());
}

#[test]
fn metadata_account_legacy_bytes_leave_v1_2_fields_absent() {
    let data = to_vec(&sample_legacy_metadata()).unwrap();
    let mut cursor = data.as_slice();
    let account = MetadataAccount::try_deserialize(&mut cursor).unwrap();

    assert_eq!(account.token_standard, None);
    assert_eq!(account.collection, None);
    assert_eq!(account.uses, None);
    assert!(cursor.is_empty());
}

#[test]
fn metadata_account_preserves_v1_2_fields_when_uses_is_none() {
    let expected = sample_metadata_with_v1_2_fields();
    let data = to_vec(&expected).unwrap();
    let mut cursor = data.as_slice();
    let account = MetadataAccount::try_deserialize(&mut cursor).unwrap();

    assert_eq!(account.token_standard, expected.token_standard);
    assert_eq!(account.collection, expected.collection);
    assert_eq!(account.uses, None);
    assert!(cursor.is_empty());
}

#[test]
fn metadata_account_preserves_v1_2_fields_when_uses_field_is_absent() {
    let expected = sample_metadata_with_v1_2_fields();
    let data = sample_metadata_with_collection_without_uses_bytes();
    let mut cursor = data.as_slice();
    let account = MetadataAccount::try_deserialize(&mut cursor).unwrap();

    assert_eq!(account.token_standard, expected.token_standard);
    assert_eq!(account.collection, expected.collection);
    assert_eq!(account.uses, None);
    assert_eq!(account.collection_details, None);
    assert_eq!(account.programmable_config, None);
    assert!(cursor.is_empty());
}

#[test]
fn metadata_account_preserves_programmable_config_fields() {
    let expected = sample_metadata_with_programmable_config();
    let data = to_vec(&expected).unwrap();
    let mut cursor = data.as_slice();
    let account = MetadataAccount::try_deserialize(&mut cursor).unwrap();

    assert_eq!(account.token_standard, expected.token_standard);
    assert_eq!(account.collection, expected.collection);
    assert_eq!(account.programmable_config, expected.programmable_config);
    assert!(cursor.is_empty());
}

#[test]
fn master_edition_account_deserialize_advances_cursor() {
    let data = to_vec(&sample_master_edition()).unwrap();
    let mut cursor = data.as_slice();
    let account = MasterEditionAccount::try_deserialize(&mut cursor).unwrap();

    assert_eq!(account.key, mpl_token_metadata::types::Key::MasterEditionV2);
    assert!(cursor.is_empty());
}

#[test]
fn master_edition_account_load_accepts_padded_accounts() {
    let data = to_vec(&sample_master_edition()).unwrap();
    let account = AccountBuffer::<4096>::new();
    account.init(
        [6u8; 32],
        metadata::ID.to_bytes(),
        data.len() + 8,
        false,
        false,
        false,
    );
    account.write_data(&data);

    let loaded = MasterEditionAccount::load(unsafe { account.view() }).unwrap();
    assert_eq!(loaded.key, mpl_token_metadata::types::Key::MasterEditionV2);
    assert_eq!(loaded.supply, 42);
    assert_eq!(loaded.max_supply, Some(100));
}

#[test]
fn token_record_account_deserialize_advances_cursor() {
    let data = to_vec(&sample_token_record()).unwrap();
    let mut cursor = data.as_slice();
    let account = TokenRecordAccount::try_deserialize(&mut cursor).unwrap();

    assert_eq!(account.key, mpl_token_metadata::types::Key::TokenRecord);
    assert!(cursor.is_empty());
}

#[test]
fn token_record_account_load_accepts_padded_accounts() {
    let data = to_vec(&sample_token_record()).unwrap();
    let account = AccountBuffer::<4096>::new();
    account.init(
        [8u8; 32],
        metadata::ID.to_bytes(),
        data.len() + 8,
        false,
        false,
        false,
    );
    account.write_data(&data);

    let loaded = TokenRecordAccount::load(unsafe { account.view() }).unwrap();
    assert_eq!(loaded.key, mpl_token_metadata::types::Key::TokenRecord);
    assert_eq!(loaded.bump, 7);
    assert_eq!(loaded.locked_transfer, Some(Pubkey::from([4u8; 32])));
}

#[test]
fn token_record_account_load_accepts_legacy_padded_accounts() {
    let data = sample_legacy_token_record_bytes();
    let account = AccountBuffer::<4096>::new();
    account.init(
        [5u8; 32],
        metadata::ID.to_bytes(),
        data.len() + 8,
        false,
        false,
        false,
    );
    account.write_data(&data);

    let loaded = TokenRecordAccount::load(unsafe { account.view() }).unwrap();
    assert_eq!(loaded.key, mpl_token_metadata::types::Key::TokenRecord);
    assert_eq!(loaded.bump, 7);
    assert_eq!(loaded.locked_transfer, None);
}

#[test]
fn metadata_account_load_validates_owner_and_raw_data() {
    let expected = sample_metadata();
    let data = to_vec(&expected).unwrap();
    let account = AccountBuffer::<4096>::new();
    account.init(
        [9u8; 32],
        metadata::ID.to_bytes(),
        data.len(),
        false,
        false,
        false,
    );
    account.write_data(&data);

    let loaded = MetadataAccount::load(unsafe { account.view() }).unwrap();
    assert_eq!(loaded.update_authority, expected.update_authority);
    assert_eq!(loaded.seller_fee_basis_points, 250);
}

#[test]
fn metadata_account_load_accepts_padded_accounts() {
    let expected = sample_metadata_with_programmable_config();
    let data = to_vec(&expected).unwrap();
    let account = AccountBuffer::<4096>::new();
    account.init(
        [4u8; 32],
        metadata::ID.to_bytes(),
        data.len() + 8,
        false,
        false,
        false,
    );
    account.write_data(&data);

    let loaded = MetadataAccount::load(unsafe { account.view() }).unwrap();
    assert_eq!(loaded.token_standard, expected.token_standard);
    assert_eq!(loaded.collection, expected.collection);
    assert_eq!(loaded.programmable_config, expected.programmable_config);
}

#[test]
fn metadata_account_rejects_wrong_owner() {
    let data = to_vec(&sample_metadata()).unwrap();
    let account = AccountBuffer::<4096>::new();
    account.init([9u8; 32], [3u8; 32], data.len(), false, false, false);
    account.write_data(&data);

    let err = MetadataAccount::load(unsafe { account.view() }).unwrap_err();
    assert_eq!(err, ProgramError::IllegalOwner);
}

#[test]
fn metadata_account_rejects_non_metadata_key_without_anchor_discriminator() {
    let mut data = to_vec(&sample_metadata()).unwrap();
    data[0] = mpl_token_metadata::types::Key::MasterEditionV2 as u8;

    let err = MetadataAccount::try_deserialize(&mut data.as_slice()).unwrap_err();
    assert_eq!(err, ProgramError::InvalidAccountData);
}

#[test]
fn token_record_account_deserialize_consumes_full_prefix_with_trailing_bytes() {
    let mut data = to_vec(&sample_token_record()).unwrap();
    let trailing = [0xAA, 0xBB, 0xCC];
    data.extend_from_slice(&trailing);

    let mut cursor = data.as_slice();
    let account = TokenRecordAccount::try_deserialize(&mut cursor).unwrap();

    assert_eq!(account.key, mpl_token_metadata::types::Key::TokenRecord);
    assert_eq!(account.bump, 7);
    assert_eq!(account.locked_transfer, Some(Pubkey::from([4u8; 32])));
    assert_eq!(cursor, trailing.as_slice());
}

#[test]
fn token_record_account_deserialize_consumes_legacy_prefix_with_trailing_bytes() {
    let mut data = sample_legacy_token_record_bytes();
    let trailing = [0xFF, 0xEE, 0xDD];
    data.extend_from_slice(&trailing);

    let mut cursor = data.as_slice();
    let account = TokenRecordAccount::try_deserialize(&mut cursor).unwrap();

    assert_eq!(account.key, mpl_token_metadata::types::Key::TokenRecord);
    assert_eq!(account.bump, 7);
    assert_eq!(account.locked_transfer, None);
    assert_eq!(cursor, trailing.as_slice());
}
