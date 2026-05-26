use {anchor_lang_v2::Id, declare_program_errors::errors, solana_pubkey::Pubkey};

fn errors_id() -> Pubkey {
    "6EF8rrecthR5Dkzon8Nwu78hRvfCKubJ14M5uBEwF6P"
        .parse()
        .unwrap()
}

#[test]
fn declared_errors_exports_program_marker_and_id() {
    assert_eq!(errors::ID, errors_id());
    assert_eq!(errors::program::Errors::id(), errors_id());
}

#[test]
fn declared_errors_generate_error_enum_with_declared_codes() {
    assert_eq!(errors::error::ErrorsError::NotAllowed as u32, 7000);
    assert_eq!(errors::error::ErrorsError::LegacyShapeFailed as u32, 7001);
}

#[test]
fn declared_errors_convert_to_program_error_custom_codes() {
    let not_allowed: anchor_lang_v2::Error = errors::error::ErrorsError::NotAllowed.into();
    let legacy_shape_failed: anchor_lang_v2::Error =
        errors::error::ErrorsError::LegacyShapeFailed.into();

    assert_eq!(not_allowed, anchor_lang_v2::Error::Custom(7000));
    assert_eq!(legacy_shape_failed, anchor_lang_v2::Error::Custom(7001));
}
