#![no_std]
#![cfg(feature = "metadata")]

extern crate std;

use {
    anchor_lang::AnchorAccount,
    anchor_spl::metadata::{
        self, Collection, Creator, DataV2, MetadataAccount, MetadataData, UseMethod, Uses,
    },
    solana_pubkey::Pubkey,
};

fn assert_metadata_data_is_owned_by_anchor<T: AnchorAccount<Data = MetadataData>>() {}

#[test]
fn metadata_public_api_does_not_require_allocator_types() {
    let creators = [Creator {
        address: Pubkey::from([1u8; 32]),
        verified: false,
        share: 100,
    }];
    let data = DataV2 {
        name: "name",
        symbol: "SYMBOL",
        uri: "https://example.invalid/metadata.json",
        seller_fee_basis_points: 500,
        creators: Some(&creators),
        collection: Some(Collection {
            verified: false,
            key: Pubkey::from([2u8; 32]),
        }),
        uses: Some(Uses {
            use_method: UseMethod::Single,
            remaining: 1,
            total: 1,
        }),
    };

    assert_metadata_data_is_owned_by_anchor::<MetadataAccount>();
    assert_eq!(data.name, "name");
    assert_eq!(
        metadata::MetadataName::try_from_str(data.name)
            .unwrap()
            .as_str(),
        "name"
    );
}
