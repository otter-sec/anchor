//! Box<T> must forward IdlAccountType signer/address metadata to T.
//!
//! Run: `cargo test -p anchor-lang --test boxed_idl_metadata`

extern crate alloc;

use {
    alloc::boxed::Box,
    anchor_lang::{
        accounts::{Program, Signer, Sysvar, UncheckedAccount},
        prelude::{Clock, Token},
        IdlAccountType,
    },
};

#[test]
fn boxed_signer_forwards_idl_is_signer() {
    assert!(<Signer as IdlAccountType>::__IDL_IS_SIGNER);
    assert!(
        <Box<Signer> as IdlAccountType>::__IDL_IS_SIGNER,
        "Box<Signer> must keep signer:true in the IDL"
    );
}

#[test]
fn boxed_program_forwards_idl_address() {
    assert_eq!(
        <Program<Token> as IdlAccountType>::__IDL_ADDRESS,
        Some("TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA")
    );
    assert_eq!(
        <Box<Program<Token>> as IdlAccountType>::__IDL_ADDRESS,
        <Program<Token> as IdlAccountType>::__IDL_ADDRESS
    );
}

#[test]
fn boxed_sysvar_forwards_idl_address() {
    assert_eq!(
        <Box<Sysvar<Clock>> as IdlAccountType>::__IDL_ADDRESS,
        <Sysvar<Clock> as IdlAccountType>::__IDL_ADDRESS
    );
}

#[test]
fn boxed_unchecked_does_not_invent_metadata() {
    assert!(!<Box<UncheckedAccount> as IdlAccountType>::__IDL_IS_SIGNER);
    assert_eq!(
        <Box<UncheckedAccount> as IdlAccountType>::__IDL_ADDRESS,
        None
    );
}
