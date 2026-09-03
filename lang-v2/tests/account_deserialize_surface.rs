#![allow(dead_code)]

use anchor_lang::{
    account, declare_id, declare_program, AccountDeserialize, Discriminator, BORSH_CONFIG,
};

declare_id!("11111111111111111111111111111111");
declare_program!(deserialize_surface);

#[account]
#[derive(Debug, PartialEq)]
pub struct PodCounter {
    pub value: u64,
}

#[account(borsh)]
#[derive(Debug, PartialEq)]
pub struct BorshCounter {
    pub value: u64,
}

fn full_account_bytes<T>(payload: &[u8]) -> Vec<u8>
where
    T: Discriminator,
{
    [T::DISCRIMINATOR, payload].concat()
}

#[test]
fn pod_account_deserializes_full_bytes_in_checked_and_unchecked_modes() {
    let full = full_account_bytes::<PodCounter>(&7u64.to_le_bytes());

    let mut checked = full.as_slice();
    let checked_value = PodCounter::try_deserialize(&mut checked).unwrap();
    assert_eq!(checked_value.value, 7);
    assert!(checked.is_empty());

    let mut unchecked = full.as_slice();
    let unchecked_value = PodCounter::try_deserialize_unchecked(&mut unchecked).unwrap();
    assert_eq!(unchecked_value.value, 7);
    assert!(unchecked.is_empty());
}

#[test]
fn borsh_account_deserializes_full_bytes_in_checked_and_unchecked_modes() {
    let payload =
        anchor_lang::wincode::config::serialize(&BorshCounter { value: 11 }, BORSH_CONFIG)
            .unwrap();
    let full = full_account_bytes::<BorshCounter>(&payload);

    let mut checked = full.as_slice();
    let checked_value = BorshCounter::try_deserialize(&mut checked).unwrap();
    assert_eq!(checked_value.value, 11);
    assert!(checked.is_empty());

    let mut unchecked = full.as_slice();
    let unchecked_value = BorshCounter::try_deserialize_unchecked(&mut unchecked).unwrap();
    assert_eq!(unchecked_value.value, 11);
    assert!(unchecked.is_empty());
}

#[test]
fn declared_program_accounts_implement_account_deserialize() {
    let payload = anchor_lang::wincode::config::serialize(
        &deserialize_surface::DeclaredCounter { value: 19 },
        BORSH_CONFIG,
    )
    .unwrap();
    let full = full_account_bytes::<deserialize_surface::DeclaredCounter>(&payload);

    let mut checked = full.as_slice();
    let checked_value =
        <deserialize_surface::DeclaredCounter as AccountDeserialize>::try_deserialize(&mut checked)
            .unwrap();
    assert_eq!(checked_value.value, 19);
    assert!(checked.is_empty());

    let mut unchecked = full.as_slice();
    let unchecked_value =
        <deserialize_surface::DeclaredCounter as AccountDeserialize>::try_deserialize_unchecked(
            &mut unchecked,
        )
        .unwrap();
    assert_eq!(unchecked_value.value, 19);
    assert!(unchecked.is_empty());
}
