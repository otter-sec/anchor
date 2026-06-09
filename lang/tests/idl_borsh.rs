#![cfg(all(feature = "idl-build", not(feature = "lazy-account")))]

use anchor_lang::{AnchorDeserialize, AnchorSerialize};

#[test]
fn test_borsh_skip_is_excluded_from_idl() {
    use anchor_lang::idl::{
        types::{IdlDefinedFields, IdlType, IdlTypeDefTy},
        IdlBuild,
    };

    #[allow(dead_code)]
    #[derive(AnchorSerialize, AnchorDeserialize)]
    pub struct SkipField {
        pub head: u8,
        #[borsh(skip)]
        pub skipped: u64,
        pub tail: u16,
    }

    let ty = SkipField::create_type().unwrap();
    let IdlTypeDefTy::Struct {
        fields: Some(IdlDefinedFields::Named(fields)),
    } = ty.ty
    else {
        panic!("expected named struct fields in IDL");
    };

    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "head");
    assert_eq!(fields[0].ty, IdlType::U8);
    assert_eq!(fields[1].name, "tail");
    assert_eq!(fields[1].ty, IdlType::U16);
}

#[test]
fn test_borsh_discriminants_are_recorded_in_idl() {
    use anchor_lang::idl::{types::IdlTypeDefTy, IdlBuild};

    #[derive(AnchorSerialize, AnchorDeserialize)]
    #[borsh(use_discriminant = true)]
    #[repr(u8)]
    pub enum Animal {
        Cat = 0,
        Dog = 1,
        Mouse = 5,
    }

    let ty = Animal::create_type().unwrap();
    let IdlTypeDefTy::Enum { variants } = ty.ty else {
        panic!("expected enum variants in IDL");
    };

    assert_eq!(variants[0].name, "Cat");
    assert_eq!(variants[0].discriminant, Some(0));
    assert_eq!(variants[1].name, "Dog");
    assert_eq!(variants[1].discriminant, Some(1));
    assert_eq!(variants[2].name, "Mouse");
    assert_eq!(variants[2].discriminant, Some(5));
}
