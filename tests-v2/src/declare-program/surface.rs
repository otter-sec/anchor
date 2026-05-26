use {
    anchor_lang_v2::{
        solana_program::instruction::AccountMeta, Discriminator, Id, InstructionData,
        ToAccountMetas,
    },
    declare_program_surface::surface,
    solana_pubkey::Pubkey,
};

fn surface_id() -> Pubkey {
    "D9t6cEFPTDWmTZfcikokLbnuuyeJT6oXnpEbyXB45LU2"
        .parse()
        .unwrap()
}

#[test]
fn declared_surface_exports_program_marker_and_id() {
    assert_eq!(surface::ID, surface_id());
    assert_eq!(surface::program::Surface::id(), surface_id());
}

#[test]
fn declared_surface_instruction_builder_uses_declared_program_id() {
    let ix = surface::instruction::NoAccounts {}.to_instruction(surface::accounts::NoAccounts {});
    assert_eq!(ix.program_id, surface_id());
}

#[test]
fn declared_surface_explicit_discriminators_are_preserved() {
    assert_eq!(
        surface::instruction::ComplexArgs::DISCRIMINATOR,
        &[40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55]
    );
}

#[test]
fn declared_surface_legacy_flags_and_optional_accounts_are_client_buildable() {
    let legacy_signer = Pubkey::new_unique();
    let legacy_writable = Pubkey::new_unique();
    let metas = surface::accounts::LegacyFlags {
        legacy_signer,
        legacy_writable,
    }
    .to_account_metas(None);

    assert_eq!(metas.len(), 2);
    assert_eq!(metas[0], AccountMeta::new_readonly(legacy_signer, true));
    assert_eq!(metas[1], AccountMeta::new(legacy_writable, false));

    let required = Pubkey::new_unique();
    let maybe_data = Pubkey::new_unique();
    let metas = surface::accounts::OptionalAccounts {
        required,
        maybe_signer: None,
        maybe_data: Some(maybe_data),
    }
    .to_account_metas(None);

    assert_eq!(metas.len(), 3);
    assert_eq!(metas[0], AccountMeta::new(required, false));
    assert_eq!(metas[1], AccountMeta::new_readonly(surface_id(), false));
    assert_eq!(metas[2], AccountMeta::new(maybe_data, false));
}

#[test]
fn declared_surface_deep_nested_accounts_flatten_in_idl_order() {
    let leaf_signer = Pubkey::new_unique();
    let leaf_writable = Pubkey::new_unique();
    let payer = Pubkey::new_unique();
    let metas = surface::accounts::NestedLevels {
        outer: surface::__client_accounts_outer::Outer {
            middle: surface::__client_accounts_middle::Middle {
                leaf_signer,
                leaf_writable,
            }
            .into(),
        }
        .into(),
        payer,
    }
    .to_account_metas(None);

    assert_eq!(metas.len(), 3);
    assert_eq!(metas[0], AccountMeta::new_readonly(leaf_signer, true));
    assert_eq!(metas[1], AccountMeta::new(leaf_writable, false));
    assert_eq!(metas[2], AccountMeta::new(payer, true));
}

#[test]
fn declared_surface_complex_types_compile_and_serialize() {
    let owner = Pubkey::new_unique();
    let args = surface::SurfaceArgs {
        enabled: true,
        small: -7,
        count: 99,
        maybe_label: Some("surface".to_string()),
        keys: vec![owner],
        pair: [-1, 2],
    };
    let data = surface::instruction::ComplexArgs {
        amounts: vec![1, 2, 3],
        maybe_owner: Some(owner),
        fixed: [4, 5, 6, 7],
        by_object: args.clone(),
        by_string: args.clone(),
        by_enum: surface::SurfaceMode::Limit(5, surface::TupleFlag(false)),
        by_tuple: surface::TupleFlag(true),
    }
    .data();

    assert_eq!(
        &data[..16],
        &[40, 41, 42, 43, 44, 45, 46, 47, 48, 49, 50, 51, 52, 53, 54, 55]
    );
    assert!(data.len() > 16);

    let legacy_data = surface::instruction::LegacyFlags {
        enabled: true,
        small: -1,
        medium: -2,
        large: -3,
        wide: u128::MAX,
    }
    .data();
    assert_eq!(&legacy_data[..5], &[10, 11, 12, 13, 14]);
    assert!(legacy_data.len() > 5);
}
