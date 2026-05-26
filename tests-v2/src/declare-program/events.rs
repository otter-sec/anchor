use {
    anchor_lang_v2::{Discriminator, Event as _, Id, IdlAccountType},
    declare_program_events::evented,
    solana_pubkey::Pubkey,
};

fn evented_id() -> Pubkey {
    "11111111111111111111111111111111".parse().unwrap()
}

#[test]
fn declared_events_exports_program_marker_and_id() {
    assert_eq!(evented::ID, evented_id());
    assert_eq!(evented::program::Evented::id(), evented_id());
}

#[test]
fn declared_events_module_reexports_event_types_and_traits() {
    fn assert_idl_type<T: IdlAccountType>() {}
    fn assert_event<T: anchor_lang_v2::Event>() {}

    assert_idl_type::<evented::events::BorshEvent>();
    assert_idl_type::<evented::events::PodEvent>();
    assert_event::<evented::events::BorshEvent>();
    assert_event::<evented::events::PodEvent>();

    assert_eq!(evented::events::BorshEvent::DISCRIMINATOR, &[1, 2, 3, 4]);
    assert_eq!(evented::events::PodEvent::DISCRIMINATOR, &[9, 8, 7, 6, 5]);
}

#[test]
fn declared_borsh_event_serializes_and_parses() {
    let event = evented::events::BorshEvent {
        amount: 42,
        label: "declared-event".to_string(),
        flag: Some(true),
    };
    let data = event.data();

    assert_eq!(&data[..4], &[1, 2, 3, 4]);
    assert!(data.len() > 4);

    let parsed = evented::parsers::Event::parse(&data).expect("parse declared borsh event");
    let evented::parsers::Event::BorshEvent(parsed) = parsed else {
        panic!("expected BorshEvent parser variant");
    };
    assert_eq!(parsed.amount, 42);
    assert_eq!(parsed.label, "declared-event");
    assert_eq!(parsed.flag, Some(true));

    let mut trailing = data;
    trailing.push(0);
    assert!(matches!(
        evented::parsers::Event::parse(&trailing),
        Err(anchor_lang_v2::Error::InvalidInstructionData)
    ));
}

#[test]
fn declared_bytemuck_event_copies_repr_c_bytes_and_parses() {
    let event = evented::events::PodEvent {
        wide: 0x0102_0304_0506_0708,
        tag: 9,
        padding: [0; 7],
    };
    let data = event.data();

    assert_eq!(&data[..5], &[9, 8, 7, 6, 5]);
    assert_eq!(
        data.len(),
        evented::events::PodEvent::DISCRIMINATOR.len()
            + core::mem::size_of::<evented::events::PodEvent>()
    );
    assert_eq!(&data[5..13], &0x0102_0304_0506_0708u64.to_le_bytes());
    assert_eq!(data[13], 9);

    let parsed =
        evented::parsers::Event::try_from(data.as_slice()).expect("parse declared bytemuck event");
    let evented::parsers::Event::PodEvent(parsed) = parsed else {
        panic!("expected PodEvent parser variant");
    };
    assert_eq!(parsed.wide, 0x0102_0304_0506_0708);
    assert_eq!(parsed.tag, 9);
    assert_eq!(parsed.padding, [0; 7]);

    let truncated = &data[..data.len() - 1];
    assert!(matches!(
        evented::parsers::Event::parse(truncated),
        Err(anchor_lang_v2::Error::InvalidInstructionData)
    ));
}

#[test]
fn declared_event_parser_rejects_unknown_discriminator() {
    assert!(matches!(
        evented::parsers::Event::parse(&[0, 0, 0]),
        Err(anchor_lang_v2::Error::InvalidArgument)
    ));
}
