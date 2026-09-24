//! `AnchorDeserialize` / `AnchorSerialize` are traits as well as derives.
//!
//! This file checks that both uses work. `#[event]` (and `#[derive]` on a
//! plain struct) implement the traits, so a type can appear in a bound
//! (`T: Event + AnchorDeserialize`) with no extra derive. It also encodes a
//! derived struct to bytes with `BORSH_CONFIG` and decodes those bytes back
//! to the same value.

use anchor_lang::{
    event, prelude::Address, AnchorDeserialize, AnchorSerialize, Discriminator, Event, BORSH_CONFIG,
};

#[event]
#[derive(Debug, PartialEq)]
pub struct Uploaded {
    pub sender: Address,
    pub len: u16,
    pub memo: String,
    pub tags: Vec<u8>,
}

#[derive(Debug, PartialEq, AnchorSerialize, AnchorDeserialize)]
pub struct Plain {
    pub a: u64,
    pub b: Option<[u8; 32]>,
}

#[derive(AnchorDeserialize)]
pub struct ReadOnly {
    pub a: u64,
}

/// The shape of `anchor_client::handle_program_log`: only the two public
/// trait names appear in the bound.
fn decode_event<T: Event + AnchorDeserialize>(bytes: &[u8]) -> Option<T> {
    let payload = bytes.strip_prefix(T::DISCRIMINATOR)?;
    anchor_lang::wincode::config::deserialize::<T, _>(payload, BORSH_CONFIG).ok()
}

fn roundtrip<T: AnchorSerialize + AnchorDeserialize>(value: &T) -> T {
    let bytes = anchor_lang::wincode::config::serialize(value, BORSH_CONFIG).unwrap();
    anchor_lang::wincode::config::deserialize::<T, _>(&bytes, BORSH_CONFIG).unwrap()
}

fn assert_deserialize<T: AnchorDeserialize>() {}
fn assert_serialize<T: AnchorSerialize>() {}

#[test]
fn default_event_decodes_with_only_the_public_bound() {
    let event = Uploaded {
        sender: Address::new_from_array([7; 32]),
        len: 3,
        memo: "hi".into(),
        tags: vec![1, 2, 3],
    };
    let bytes = event.data();
    assert_eq!(decode_event::<Uploaded>(&bytes), Some(event));
}

#[test]
fn event_data_is_borsh_shaped() {
    // disc(8) + sender(32) + len(2) + memo(u32 len + 2) + tags(u32 len + 3)
    let event = Uploaded {
        sender: Address::new_from_array([7; 32]),
        len: 3,
        memo: "hi".into(),
        tags: vec![1, 2, 3],
    };
    let bytes = event.data();
    assert_eq!(bytes.len(), 8 + 32 + 2 + 4 + 2 + 4 + 3);
    assert_eq!(&bytes[..8], Uploaded::DISCRIMINATOR);
    assert_eq!(&bytes[40..42], &3u16.to_le_bytes());
    assert_eq!(&bytes[42..46], &2u32.to_le_bytes());
    assert_eq!(&bytes[46..48], b"hi");
}

#[test]
fn wrong_discriminator_is_not_decoded() {
    let mut bytes = Uploaded {
        sender: Address::new_from_array([0; 32]),
        len: 0,
        memo: String::new(),
        tags: Vec::new(),
    }
    .data();
    bytes[0] ^= 0xff;
    assert_eq!(decode_event::<Uploaded>(&bytes), None);
}

#[test]
fn derives_satisfy_the_traits() {
    assert_serialize::<Uploaded>();
    assert_deserialize::<Uploaded>();
    assert_serialize::<Plain>();
    assert_deserialize::<Plain>();
    assert_deserialize::<ReadOnly>();
    // Primitives and containers come for free through the blanket impl.
    assert_deserialize::<u64>();
    assert_deserialize::<Vec<Address>>();
    assert_serialize::<Option<String>>();
}

#[test]
fn plain_struct_round_trips() {
    let value = Plain {
        a: 9,
        b: Some([1; 32]),
    };
    assert_eq!(roundtrip(&value), value);
}
