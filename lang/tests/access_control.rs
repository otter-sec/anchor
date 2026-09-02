//! `#[access_control]` arguments are parsed as expressions: nested calls and
//! string literals must survive intact, and comma-separated checks run in
//! order before the handler body.

use {
    anchor_lang::{access_control, error::ErrorCode, Result},
    std::sync::atomic::{AtomicUsize, Ordering},
};

static CALLS: AtomicUsize = AtomicUsize::new(0);

fn admin_key() -> u8 {
    7
}

fn check_key(key: u8) -> Result<()> {
    assert_eq!(key, 7);
    Ok(())
}

fn check_msg(msg: &str) -> Result<()> {
    assert_eq!(msg, "some message");
    Ok(())
}

fn first() -> Result<()> {
    assert_eq!(CALLS.fetch_add(1, Ordering::SeqCst), 0);
    Ok(())
}

fn second() -> Result<()> {
    assert_eq!(CALLS.fetch_add(1, Ordering::SeqCst), 1);
    Ok(())
}

fn fail() -> Result<()> {
    Err(ErrorCode::InstructionMissing.into())
}

#[access_control(check_key(admin_key()))]
fn nested_call() -> Result<()> {
    Ok(())
}

#[access_control(check_msg("some message"))]
fn string_literal() -> Result<()> {
    Ok(())
}

#[access_control(first(), second())]
fn multiple_checks() -> Result<()> {
    assert_eq!(CALLS.fetch_add(1, Ordering::SeqCst), 2);
    Ok(())
}

#[access_control(fail())]
fn guarded() -> Result<()> {
    panic!("body ran after a failed check");
}

#[access_control()]
fn no_checks() -> Result<()> {
    Ok(())
}

#[test]
fn nested_calls_parse() {
    nested_call().unwrap();
}

#[test]
fn string_literals_survive() {
    string_literal().unwrap();
}

#[test]
fn checks_run_in_order() {
    multiple_checks().unwrap();
    assert_eq!(CALLS.load(Ordering::SeqCst), 3);
}

#[test]
fn failing_check_short_circuits() {
    assert!(guarded().is_err());
}

#[test]
fn empty_args_compile() {
    no_checks().unwrap();
}
