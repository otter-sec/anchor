use anchor_lang::prelude::*;

declare_program!(external);
use external::types::GenericStruct;

#[test]
fn structs() {
    let _ = GenericStruct { field: [1; 2] };
}

#[test]
fn structs_can_clone() {
    let strct = GenericStruct { field: [1; 2] };
    let _ = strct.clone();
}

#[test]
fn structs_can_debug() {
    let strct = GenericStruct { field: [1; 2] };
    println!("{strct:?}");
}

#[test]
fn args() {
    let _ = external::client::args::TestCompilationGenericTypes {
        generic_struct: GenericStruct {
            field: Default::default(),
        },
    };
}

#[test]
fn args_can_clone() {
    let args = external::client::args::TestCompilationGenericTypes {
        generic_struct: GenericStruct {
            field: Default::default(),
        },
    };
    let _ = args.clone();
}

#[test]
fn args_can_debug() {
    let args = external::client::args::TestCompilationGenericTypes {
        generic_struct: GenericStruct {
            field: Default::default(),
        },
    };
    println!("{args:?}");
}
