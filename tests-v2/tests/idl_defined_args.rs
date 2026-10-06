//! Plain `#[derive(IdlType)]` structs used as instruction arguments must
//! resolve through `IdlAccountType` and land in the IDL's `types[]`.
//! Regression coverage for otter-sec/anchor#4850 — an arg struct deriving
//! only `AnchorDeserialize` / `AnchorSerialize` used to fail `--features idl-build`
//! compilation with an unsatisfied `IdlAccountType` bound.

use {
    anchor_lang_idl_spec::{IdlArrayLen, IdlDefinedFields, IdlType, IdlTypeDef, IdlTypeDefTy},
    anchor_lang::IdlAccountType,
};

#[test]
fn plain_arg_struct_emits_type_def() {
    let json = <accounts_test::BumpArgs as IdlAccountType>::__idl_type_def()
        .expect("IdlType derive should set __IDL_TYPE_DEF");
    let type_def: IdlTypeDef = serde_json::from_str(json)
        .unwrap_or_else(|err| panic!("failed to parse type def JSON: {err}\njson: {json}"));

    assert_eq!(type_def.name, "BumpArgs");
    let IdlTypeDefTy::Struct {
        fields: Some(IdlDefinedFields::Named(fields)),
    } = &type_def.ty
    else {
        panic!(
            "expected named-field struct type def, got {:?}",
            type_def.ty
        );
    };
    assert_eq!(fields.len(), 2);
    assert_eq!(fields[0].name, "amount");
    assert_eq!(fields[0].ty, IdlType::U64);
    assert_eq!(fields[1].name, "tag");
    assert_eq!(
        fields[1].ty,
        IdlType::Array(Box::new(IdlType::U8), IdlArrayLen::Value(4))
    );
}

#[test]
fn plain_enum_emits_variants() {
    let json = <accounts_test::DepositType as IdlAccountType>::__idl_type_def()
        .expect("IdlType derive should set __IDL_TYPE_DEF");
    let type_def: IdlTypeDef = serde_json::from_str(json)
        .unwrap_or_else(|err| panic!("failed to parse type def JSON: {err}\njson: {json}"));

    assert_eq!(type_def.name, "DepositType");
    let IdlTypeDefTy::Enum { variants } = &type_def.ty else {
        panic!("expected enum type def, got {:?}", type_def.ty);
    };
    assert_eq!(variants.len(), 2);
    assert_eq!(variants[0].name, "Protected");
    assert_eq!(variants[1].name, "Boosted");
}

#[test]
fn plain_arg_struct_registers_as_type_not_account() {
    let mut accounts: Vec<&'static str> = Vec::new();
    let mut types: Vec<&'static str> = Vec::new();
    <accounts_test::BumpArgs as IdlAccountType>::__register_idl_deps(&mut accounts, &mut types);

    assert!(
        accounts.is_empty(),
        "plain IdlType structs must not appear in accounts[]"
    );
    assert_eq!(types.len(), 1);
    let type_def: IdlTypeDef = serde_json::from_str(types[0]).unwrap();
    assert_eq!(type_def.name, "BumpArgs");
}
