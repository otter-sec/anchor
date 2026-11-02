//! Regression tests for otter-sec/anchor#2942.

#![allow(dead_code, deprecated)]

use {anchor_lang::prelude::*, std::collections::BTreeSet};

declare_id!("Fg6PaFpoGXkYsidMpWTK6W2BeZ7FEfcYkg476zPFsLnS");

const EXPECTED_A: u64 = 7;
const EXPECTED_B: u32 = 9;

#[error_code]
pub enum TestError {
    #[msg("child saw an unexpected `a`")]
    ChildWrongA,
    #[msg("child saw an unexpected `b`")]
    ChildWrongB,
    #[msg("parent saw an unexpected `a`")]
    ParentWrongA,
}

#[derive(Accounts)]
#[instruction(a: u64, b: u32)]
pub struct Child<'info> {
    /// CHECK: test only
    #[account(
        constraint = a == EXPECTED_A @ TestError::ChildWrongA,
        constraint = b == EXPECTED_B @ TestError::ChildWrongB,
    )]
    pub target: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(a: u64, b: u32)]
pub struct Parent<'info> {
    /// CHECK: test only
    #[account(constraint = a == EXPECTED_A @ TestError::ParentWrongA)]
    pub first: UncheckedAccount<'info>,
    pub child: Child<'info>,
}

#[derive(Accounts)]
#[instruction(a: u64)]
pub struct PrefixParent<'info> {
    /// CHECK: test only
    #[account(constraint = a == EXPECTED_A @ TestError::ParentWrongA)]
    pub first: UncheckedAccount<'info>,
    pub child: Child<'info>,
}

#[derive(Accounts)]
#[instruction(b: u32)]
pub struct SuffixChild<'info> {
    /// CHECK: test only
    #[account(constraint = b == EXPECTED_B @ TestError::ChildWrongB)]
    pub target: UncheckedAccount<'info>,
}

#[derive(Accounts)]
#[instruction(a: u64)]
pub struct SuffixParent<'info> {
    /// CHECK: test only
    #[account(constraint = a == EXPECTED_A @ TestError::ParentWrongA)]
    pub first: UncheckedAccount<'info>,
    pub child: SuffixChild<'info>,
}

#[derive(Accounts)]
#[instruction(a: u64, b: u32)]
pub struct FullParentSuffixChild<'info> {
    /// CHECK: test only
    #[account(constraint = a == EXPECTED_A @ TestError::ParentWrongA)]
    pub first: UncheckedAccount<'info>,
    pub child: SuffixChild<'info>,
}

#[derive(Accounts)]
#[instruction(a: u64, b: u32)]
pub struct Mid<'info> {
    /// CHECK: test only
    #[account(
        constraint = a == EXPECTED_A @ TestError::ChildWrongA,
        constraint = b == EXPECTED_B @ TestError::ChildWrongB,
    )]
    pub target: UncheckedAccount<'info>,
    pub inner: SuffixChild<'info>,
}

#[derive(Accounts)]
#[instruction(a: u64)]
pub struct GrandParent<'info> {
    /// CHECK: test only
    #[account(constraint = a == EXPECTED_A @ TestError::ParentWrongA)]
    pub first: UncheckedAccount<'info>,
    pub mid: Mid<'info>,
}

#[derive(Accounts)]
pub struct NoArgsParent<'info> {
    /// CHECK: test only
    pub first: UncheckedAccount<'info>,
    pub child: Child<'info>,
}

fn ix_data(a: u64, b: u32) -> Vec<u8> {
    let mut v = Vec::new();
    a.serialize(&mut v).unwrap();
    b.serialize(&mut v).unwrap();
    v
}

fn error_code(err: anchor_lang::error::Error) -> u32 {
    match err {
        anchor_lang::error::Error::AnchorError(e) => e.error_code_number,
        other => panic!("unexpected error kind: {other:?}"),
    }
}

macro_rules! try_accounts {
    ($ty:ident, $data:expr) => {{
        let program_id = crate::ID;
        let owner = crate::ID;
        let key_first = Pubkey::new_unique();
        let key_target = Pubkey::new_unique();
        let key_extra = Pubkey::new_unique();
        let mut lamports_first = 0u64;
        let mut lamports_target = 0u64;
        let mut lamports_extra = 0u64;
        let mut data_first: Vec<u8> = Vec::new();
        let mut data_target: Vec<u8> = Vec::new();
        let mut data_extra: Vec<u8> = Vec::new();
        let infos = [
            AccountInfo::new(
                &key_first,
                false,
                false,
                &mut lamports_first,
                &mut data_first,
                &owner,
                false,
            ),
            AccountInfo::new(
                &key_target,
                false,
                false,
                &mut lamports_target,
                &mut data_target,
                &owner,
                false,
            ),
            AccountInfo::new(
                &key_extra,
                false,
                false,
                &mut lamports_extra,
                &mut data_extra,
                &owner,
                false,
            ),
        ];
        let mut accounts: &[AccountInfo] = &infos;
        let data: Vec<u8> = $data;
        let result = <$ty as Accounts<_>>::try_accounts(
            &program_id,
            &mut accounts,
            &data,
            &mut Default::default(),
            &mut BTreeSet::new(),
        );
        result.map(|_| ())
    }};
}

#[test]
fn child_sees_same_args_as_parent() {
    try_accounts!(Parent, ix_data(EXPECTED_A, EXPECTED_B)).unwrap();
}

#[test]
fn child_sees_args_when_parent_declares_prefix() {
    try_accounts!(PrefixParent, ix_data(EXPECTED_A, EXPECTED_B)).unwrap();
}

#[test]
fn child_sees_args_when_parent_declares_none() {
    try_accounts!(NoArgsParent, ix_data(EXPECTED_A, EXPECTED_B)).unwrap();
}

#[test]
fn trailing_bytes_are_ignored() {
    let mut data = ix_data(EXPECTED_A, EXPECTED_B);
    data.extend_from_slice(&[0xAA; 16]);
    try_accounts!(Parent, data).unwrap();
}

#[test]
fn child_constraint_fails_on_wrong_a() {
    let err = try_accounts!(NoArgsParent, ix_data(EXPECTED_A + 1, EXPECTED_B)).unwrap_err();
    assert_eq!(error_code(err), TestError::ChildWrongA as u32 + 6000);
}

#[test]
fn child_constraint_fails_on_wrong_b() {
    let err = try_accounts!(NoArgsParent, ix_data(EXPECTED_A, EXPECTED_B + 1)).unwrap_err();
    assert_eq!(error_code(err), TestError::ChildWrongB as u32 + 6000);
}

#[test]
fn composite_constraints_run_before_parent_constraints() {
    let err = try_accounts!(Parent, ix_data(EXPECTED_A + 1, EXPECTED_B)).unwrap_err();
    assert_eq!(error_code(err), TestError::ChildWrongA as u32 + 6000);
}

#[test]
fn child_declaring_only_trailing_args_continues_after_parent() {
    try_accounts!(SuffixParent, ix_data(EXPECTED_A, EXPECTED_B)).unwrap();
    let err = try_accounts!(SuffixParent, ix_data(EXPECTED_A, EXPECTED_B + 1)).unwrap_err();
    assert_eq!(error_code(err), TestError::ChildWrongB as u32 + 6000);
}

#[test]
fn child_starts_at_named_parent_arg() {
    try_accounts!(FullParentSuffixChild, ix_data(EXPECTED_A, EXPECTED_B)).unwrap();
    let err =
        try_accounts!(FullParentSuffixChild, ix_data(EXPECTED_A, EXPECTED_B + 1)).unwrap_err();
    assert_eq!(error_code(err), TestError::ChildWrongB as u32 + 6000);
}

#[test]
fn three_level_chain_resolves_each_level() {
    try_accounts!(GrandParent, ix_data(EXPECTED_A, EXPECTED_B)).unwrap();
    let err = try_accounts!(GrandParent, ix_data(EXPECTED_A, EXPECTED_B + 1)).unwrap_err();
    assert_eq!(error_code(err), TestError::ChildWrongB as u32 + 6000);
}

#[test]
fn truncated_data_fails_in_parent() {
    let data = EXPECTED_A.to_le_bytes().to_vec();
    let err = try_accounts!(Parent, data).unwrap_err();
    assert_eq!(
        error_code(err),
        anchor_lang::error::ErrorCode::InstructionDidNotDeserialize as u32
    );
}

#[test]
fn truncated_data_fails_in_child() {
    let data = EXPECTED_A.to_le_bytes().to_vec();
    let err = try_accounts!(PrefixParent, data).unwrap_err();
    assert_eq!(
        error_code(err),
        anchor_lang::error::ErrorCode::InstructionDidNotDeserialize as u32
    );
}

#[test]
fn empty_data_fails() {
    let err = try_accounts!(Parent, Vec::new()).unwrap_err();
    assert_eq!(
        error_code(err),
        anchor_lang::error::ErrorCode::InstructionDidNotDeserialize as u32
    );
}
