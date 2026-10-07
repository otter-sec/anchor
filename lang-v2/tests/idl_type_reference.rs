#![cfg(feature = "idl-build")]
#![allow(dead_code)]
use anchor_lang::{idl_build::IdlGenericArg, prelude::*};

type Count = u64;
type Text = String;
type Rows = Vec<Option<Text>>;
const COUNT: usize = 3;

#[derive(IdlType)]
struct Value {
    count: Count,
    rows: Rows,
}
#[derive(IdlType)]
struct Pair<A: IdlAccountType, B: IdlAccountType> {
    first: A,
    second: B,
}
#[derive(IdlType)]
struct Template<T: IdlAccountType, const N: usize> {
    items: Vec<T>,
    pair: Pair<T, u64>,
    fixed: Count,
    array: [u8; N],
}
type TemplateAlias = Template<u16, COUNT>;

#[test]
fn resolved_aliases_preserve_wire_type_metadata() {
    assert_eq!(Count::__idl_type_reference(&[]), "\"u64\"");
    assert_eq!(
        Rows::__idl_type_reference(&[]),
        "{\"vec\":{\"option\":\"string\"}}"
    );
    let definition = Value::__idl_type_def().unwrap();
    assert!(definition.contains("\"type\":\"u64\""));
    assert!(definition.contains("\"type\":{\"vec\":{\"option\":\"string\"}}"));
    assert!(!definition.contains("\"name\":\"Count\""));
}

#[test]
fn generic_definitions_keep_symbolic_parameters_and_resolved_fixed_types() {
    let reference = TemplateAlias::__idl_type_reference(&[]);
    assert_eq!(
        reference,
        r#"{"defined":{"name":"Template","generics":[{"kind":"type","type":"u16"},{"kind":"const","value":"3"}]}}"#
    );
    let definition = TemplateAlias::__idl_type_def().unwrap();
    assert!(definition.contains("\"vec\":{\"generic\":\"T\"}"));
    assert!(definition.contains("\"array\":[\"u8\",{\"generic\":\"N\"}]"));
    assert!(definition.contains(
        r#""generics":[{"kind":"type","type":{"generic":"T"}},{"kind":"type","type":"u64"}]"#
    ));
}

#[test]
fn collection_and_tuple_metadata_register_complete_definitions() {
    type Map = std::collections::BTreeMap<u8, u16>;
    assert!(Map::__idl_type_reference(&[])
        .contains("\"vec\":{\"defined\":{\"name\":\"__anchor_tuple_2\""));
    let mut accounts = Vec::new();
    let mut types = Vec::new();
    Map::__register_idl_deps(&mut accounts, &mut types);
    assert!(types
        .iter()
        .any(|ty| ty.contains("\"name\":\"__anchor_tuple_2\"")));
    assert_eq!(<&[u8]>::__idl_type_reference(&[]), "\"bytes\"");
    assert_eq!(<&str>::__idl_type_reference(&[]), "\"string\"");
    assert_eq!(
        <Vec<u8>>::__idl_type_reference(&[Some(IdlGenericArg::Type("{\"generic\":\"T\"}".into()))]),
        "{\"vec\":{\"generic\":\"T\"}}"
    );
}

#[test]
fn bounded_pod_vector_aliases_keep_their_generic_layout_reference() {
    type Items = PodVec<PodU64, COUNT>;
    let reference = Items::__idl_type_reference(&[]);
    assert!(reference.contains("\"name\":\"PodVec\""));
    assert!(reference.contains("\"type\":{\"defined\":{\"name\":\"PodU64\"}}"));
    assert!(reference.contains("\"kind\":\"const\",\"value\":\"3\""));
}
