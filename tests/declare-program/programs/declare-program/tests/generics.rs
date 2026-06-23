use anchor_lang::prelude::*;

declare_program!(external);
use external::types::GenericStruct;

#[test]
fn structs() {
    let _ = GenericStruct { field: [1; 2] };
}

#[test]
fn args() {
    let _ = external::client::args::TestCompilationGenericTypes {
        generic_struct: GenericStruct {
            field: Default::default(),
        },
    };
}
