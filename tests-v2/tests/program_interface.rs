use {
    anchor_lang_v2::{Discriminator, InstructionData, ToAccountMetas},
    program_interface::{accounts, cpi_account_type_is_generated, instruction, ComplexArgs},
    sha2::{Digest, Sha256},
    solana_pubkey::Pubkey,
};

fn program_id() -> Pubkey {
    "Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp"
        .parse()
        .unwrap()
}

fn disc_hash(name: &str) -> [u8; 8] {
    let hash = Sha256::digest(format!("global:{name}").as_bytes());
    hash[..8].try_into().unwrap()
}

#[test]
fn interface_builders_use_declared_program_id() {
    let ix = instruction::DefaultDisc {}.to_instruction(accounts::Empty {});

    assert_eq!(ix.program_id, program_id());
}

#[test]
fn default_discriminator_uses_anchor_global_hash() {
    let ix = instruction::DefaultDisc {}.to_instruction(accounts::Empty {});

    assert_eq!(
        instruction::DefaultDisc::DISCRIMINATOR,
        &disc_hash("default_disc")
    );
    assert_eq!(ix.data, disc_hash("default_disc"));
}

#[test]
fn explicit_discriminator_widths_are_preserved() {
    assert_eq!(instruction::OneByte::DISCRIMINATOR, &[7]);
    assert_eq!(instruction::ShortDisc::DISCRIMINATOR, &[1, 2, 3, 4]);
    assert_eq!(
        instruction::EightByte::DISCRIMINATOR,
        &[10, 11, 12, 13, 14, 15, 16, 17]
    );
    assert_eq!(
        instruction::LongDisc::DISCRIMINATOR,
        &[21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36]
    );
}

#[test]
fn scalar_and_struct_args_serialize_after_discriminator() {
    let one = instruction::OneByte {
        amount: 0x1122_3344_5566_7788,
    };
    let one_data = one.data();
    assert_eq!(&one_data[..1], &[7]);
    assert_eq!(&one_data[1..], &0x1122_3344_5566_7788u64.to_le_bytes());

    let long = instruction::LongDisc {
        args: ComplexArgs {
            amount: 0x0102_0304_0506_0708,
            tag: *b"idl",
        },
    };
    let long_data = long.data();
    assert_eq!(
        &long_data[..16],
        &[21, 22, 23, 24, 25, 26, 27, 28, 29, 30, 31, 32, 33, 34, 35, 36]
    );
    assert_eq!(&long_data[16..24], &0x0102_0304_0506_0708u64.to_le_bytes());
    assert_eq!(&long_data[24..], b"idl");
}

#[test]
fn generated_account_metas_preserve_writable_and_signer_flags() {
    let data = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let spectator = Pubkey::new_unique();
    let metas = accounts::Mixed {
        data,
        authority,
        spectator,
    }
    .to_account_metas(None);

    assert_eq!(metas.len(), 3);
    assert_eq!(metas[0].pubkey, data);
    assert!(metas[0].is_writable);
    assert!(!metas[0].is_signer);
    assert_eq!(metas[1].pubkey, authority);
    assert!(!metas[1].is_writable);
    assert!(metas[1].is_signer);
    assert_eq!(metas[2].pubkey, spectator);
    assert!(!metas[2].is_writable);
    assert!(!metas[2].is_signer);
}

#[test]
fn nested_account_metas_are_flattened_in_idl_order() {
    let authority = Pubkey::new_unique();
    let vault = Pubkey::new_unique();
    let metas = accounts::NestedOuter {
        nested: accounts::AuthorityOnly { authority }.into(),
        vault,
    }
    .to_account_metas(None);

    assert_eq!(metas.len(), 2);
    assert_eq!(metas[0].pubkey, authority);
    assert!(metas[0].is_signer);
    assert!(!metas[0].is_writable);
    assert_eq!(metas[1].pubkey, vault);
    assert!(!metas[1].is_signer);
    assert!(metas[1].is_writable);
}

#[test]
fn repeated_account_struct_reexports_are_deduped() {
    let data = Pubkey::new_unique();
    let authority = Pubkey::new_unique();
    let spectator = Pubkey::new_unique();
    let accounts = accounts::Mixed {
        data,
        authority,
        spectator,
    };
    let ix = instruction::ReuseAccounts {}.to_instruction(accounts);

    assert_eq!(ix.program_id, program_id());
    assert_eq!(ix.data, &[50, 51, 52, 53]);
}

#[test]
fn cpi_account_surface_is_generated_for_nested_accounts() {
    let _ = cpi_account_type_is_generated;
}
