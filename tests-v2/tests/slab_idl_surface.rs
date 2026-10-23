use {
    anchor_lang::{
        accounts::Slab,
        bytemuck::{Pod, Zeroable},
        Discriminator, IdlAccountType, Owner,
    },
    pinocchio::address::Address,
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Header {
    count: u64,
}

impl Owner for Header {
    const OWNER: Address = Address::new_from_array([0x11; 32]);
}

impl Discriminator for Header {
    const DISCRIMINATOR: &'static [u8] = &[1, 2, 3, 4, 5, 6, 7, 8];
}

impl IdlAccountType for Header {
    const __IDL_ACCOUNT_ENTRY: Option<&'static str> = Some("header-account");
    const __IDL_TYPE_DEF: Option<&'static str> = Some("header-type");

    fn __register_idl_deps(accounts: &mut Vec<&'static str>, types: &mut Vec<&'static str>) {
        accounts.push("header-account");
        types.push("header-type");
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct Item {
    value: u64,
}

impl IdlAccountType for Item {
    const __IDL_TYPE_DEF: Option<&'static str> = Some("item-type");

    fn __register_idl_deps(_accounts: &mut Vec<&'static str>, types: &mut Vec<&'static str>) {
        types.push("item-type");
    }
}

#[test]
fn slab_idl_surface_forwards_header_metadata() {
    assert_eq!(
        <Slab<Header, Item> as IdlAccountType>::__IDL_ACCOUNT_ENTRY,
        Header::__IDL_ACCOUNT_ENTRY
    );
    assert_eq!(
        <Slab<Header, Item> as IdlAccountType>::__IDL_TYPE_DEF,
        Header::__IDL_TYPE_DEF
    );
}

#[test]
fn slab_idl_surface_remains_header_only() {
    let mut accounts = Vec::new();
    let mut types = Vec::new();
    <Slab<Header, Item> as IdlAccountType>::__register_idl_deps(&mut accounts, &mut types);

    assert_eq!(accounts, vec!["header-account"]);
    assert_eq!(types, vec!["header-type"]);
    assert!(!types.iter().any(|entry| entry.contains("item-type")));
}
