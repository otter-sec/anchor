#![allow(dead_code)]

use anchor_lang::{prelude::*, TryAccounts};

declare_id!("11111111111111111111111111111111");

#[program]
pub mod prefix_only_instruction_args {
    use super::*;

    pub fn ix(ctx: &mut Context<PrefixOnly>, amount: u64, step: i32) -> Result<()> {
        let _ = ctx;
        let _ = amount;
        let _ = step;
        Ok(())
    }
}

#[derive(Accounts)]
pub struct NoArgs {
    pub account: UncheckedAccount,
}

#[derive(Accounts)]
#[instruction(amount: u64, step: i32)]
pub struct WithArgs {
    pub account: UncheckedAccount,
}

#[derive(Accounts)]
#[instruction(amount: u64)]
pub struct PrefixOnly {
    pub account: UncheckedAccount,
}

fn _no_args_maps_to_unit() {
    let _: <NoArgs as TryAccounts>::IxArgs<'static> = ();
}

#[test]
fn no_instruction_args_maps_to_unit() {
    _no_args_maps_to_unit();
}

fn _instruction_args_are_tuple<'a>(args: <WithArgs as TryAccounts>::IxArgs<'a>) -> (u64, i32) {
    args
}

fn _instruction_args_keep_declared_prefix<'a>(
    args: <PrefixOnly as TryAccounts>::IxArgs<'a>,
) -> (u64,) {
    args
}

#[test]
fn instruction_args_map_to_tuple() {
    let _: fn(_) -> _ = _instruction_args_are_tuple;
}

#[test]
fn instruction_attr_keeps_prefix_tuple() {
    let _: fn(_) -> _ = _instruction_args_keep_declared_prefix;
}
