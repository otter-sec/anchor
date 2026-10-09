use {
    crate::{basic_2, composite, events},
    anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas},
    solana_sdk::pubkey::Pubkey,
};

#[test]
fn idl_instruction_matches_v1_program() {
    let authority = Pubkey::new_unique();
    let generated = basic_2::instruction::Create { authority }.data();
    let original =
        anchor_lang_v1::InstructionData::data(&legacy_basic_2::instruction::Create { authority });
    assert_eq!(generated, original);
}

#[test]
fn idl_account_deserializes_v1_program_data() {
    let authority = Pubkey::new_unique();
    let mut bytes = Vec::new();
    anchor_lang_v1::AccountSerialize::try_serialize(
        &legacy_basic_2::Counter {
            authority,
            count: 42,
        },
        &mut bytes,
    )
    .unwrap();
    let counter = basic_2::Counter::try_deserialize(&mut bytes.as_slice()).unwrap();
    assert_eq!(counter.authority, authority);
    assert_eq!(counter.count, 42);
}

#[test]
fn idl_event_matches_v1_program() {
    let original = legacy_events::MyEvent {
        data: 5,
        label: "hello".into(),
    };
    let generated = events::MyEvent {
        data: 5,
        label: "hello".into(),
    };
    assert_eq!(
        anchor_lang::Event::data(&generated),
        anchor_lang_v1::Event::data(&original),
    );
}

#[test]
fn idl_nested_account_metas_match_v1_program() {
    let dummy_a = Pubkey::new_unique();
    let dummy_b = Pubkey::new_unique();
    let generated = composite::accounts::CompositeUpdate {
        foo: composite::__client_accounts_foo::Foo { dummy_a },
        bar: composite::__client_accounts_bar::Bar { dummy_b },
    };
    let original = legacy_composite::accounts::CompositeUpdate {
        foo: legacy_composite::accounts::Foo { dummy_a },
        bar: legacy_composite::accounts::Bar { dummy_b },
    };
    assert_eq!(
        generated.to_account_metas(None),
        anchor_lang_v1::ToAccountMetas::to_account_metas(&original, None),
    );
}
