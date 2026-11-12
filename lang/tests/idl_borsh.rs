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
fn test_borsh_bound_is_accepted_by_idl_build() {
    use anchor_lang::idl::{
        types::{IdlDefinedFields, IdlTypeDefTy},
        IdlBuild,
    };

    #[allow(dead_code)]
    #[derive(AnchorSerialize, AnchorDeserialize)]
    pub struct BoundField {
        #[borsh(bound(
            serialize = "u64: borsh::ser::BorshSerialize",
            deserialize = "u64: borsh::de::BorshDeserialize"
        ))]
        pub value: u64,
    }

    let ty = BoundField::create_type().unwrap();
    let IdlTypeDefTy::Struct {
        fields: Some(IdlDefinedFields::Named(fields)),
    } = ty.ty
    else {
        panic!("expected named struct fields in IDL");
    };

    assert_eq!(fields.len(), 1);
    assert_eq!(fields[0].name, "value");
}

#[test]
fn test_borsh_init_is_accepted_by_idl_build_and_runs_on_deserialize() {
    use {
        anchor_lang::idl::{types::IdlTypeDefTy, IdlBuild},
        borsh::{from_slice, to_vec},
    };

    #[derive(AnchorSerialize, AnchorDeserialize, Debug, PartialEq)]
    #[borsh(init=initialization_method)]
    pub enum Animal {
        Cat,
        Dog,
    }

    impl Animal {
        fn initialization_method(&mut self) {
            *self = Self::Cat;
        }
    }

    let ty = Animal::create_type().unwrap();
    let IdlTypeDefTy::Enum { variants } = ty.ty else {
        panic!("expected enum variants in IDL");
    };

    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].name, "Cat");
    assert_eq!(variants[1].name, "Dog");

    let encoded = to_vec(&Animal::Dog).unwrap();
    let decoded = from_slice::<Animal>(&encoded).unwrap();
    assert_eq!(decoded, Animal::Cat);
}

#[test]
fn test_borsh_use_discriminant_false_keeps_default_idl_enum_layout() {
    use anchor_lang::idl::{types::IdlTypeDefTy, IdlBuild};

    #[derive(AnchorSerialize, AnchorDeserialize)]
    #[borsh(use_discriminant = false)]
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

    assert_eq!(variants.len(), 3);
    assert_eq!(variants[0].name, "Cat");
    assert_eq!(variants[1].name, "Dog");
    assert_eq!(variants[2].name, "Mouse");
}
