#![allow(dead_code)]

mod accounts;
mod address;
mod common;
mod constant;
mod defined;
mod error;
mod event;
mod external;
mod program;

pub use {
    accounts::gen_idl_build_impl_accounts_struct,
    address::gen_idl_print_fn_address,
    constant::gen_idl_print_fn_constant,
    defined::{impl_idl_build_enum, impl_idl_build_struct, impl_idl_build_union},
    error::gen_idl_print_fn_error,
    event::gen_idl_print_fn_event,
    program::gen_idl_print_fn_program,
};
