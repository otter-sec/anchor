//! Two-arg `require_*` arms delegate to the three-arg forms.
//!
//! Run: `cargo test -p anchor-lang --test require_macros`

use {
    anchor_lang::{prelude::*, ErrorCode},
    pinocchio::address::Address,
    solana_program_error::ProgramError,
};

fn require_neq_hit() -> Result<()> {
    require_neq!(1u64, 1u64);
    Ok(())
}

fn require_gt_hit() -> Result<()> {
    require_gt!(1u64, 1u64);
    Ok(())
}

fn require_gte_hit() -> Result<()> {
    require_gte!(0u64, 1u64);
    Ok(())
}

fn require_eq_hit() -> Result<()> {
    require_eq!(1u64, 2u64);
    Ok(())
}

fn require_keys_eq_hit() -> Result<()> {
    require_keys_eq!(
        Address::new_from_array([1; 32]),
        Address::new_from_array([2; 32])
    );
    Ok(())
}

fn require_keys_neq_hit() -> Result<()> {
    let key = Address::new_from_array([1; 32]);
    require_keys_neq!(key, key);
    Ok(())
}

#[test]
fn two_arg_arms_use_default_error_codes() {
    assert_eq!(
        require_neq_hit().unwrap_err(),
        ErrorCode::RequireNeqViolated.into()
    );
    assert_eq!(
        require_gt_hit().unwrap_err(),
        ErrorCode::RequireGtViolated.into()
    );
    assert_eq!(
        require_gte_hit().unwrap_err(),
        ErrorCode::RequireGteViolated.into()
    );
    assert_eq!(
        require_eq_hit().unwrap_err(),
        ErrorCode::RequireEqViolated.into()
    );
    assert_eq!(
        require_keys_eq_hit().unwrap_err(),
        ErrorCode::RequireKeysEqViolated.into()
    );
    assert_eq!(
        require_keys_neq_hit().unwrap_err(),
        ErrorCode::RequireKeysNeqViolated.into()
    );
}

#[test]
fn two_arg_arms_succeed_when_the_comparison_holds() {
    fn ok() -> Result<()> {
        require_neq!(1u64, 2u64);
        require_gt!(2u64, 1u64);
        require_gte!(1u64, 1u64);
        require_eq!(1u64, 1u64);
        require_keys_eq!(
            Address::new_from_array([1; 32]),
            Address::new_from_array([1; 32])
        );
        require_keys_neq!(
            Address::new_from_array([1; 32]),
            Address::new_from_array([2; 32])
        );
        Ok(())
    }
    ok().unwrap();
}

#[test]
fn two_arg_trailing_comma_still_uses_the_default_error() {
    fn hit() -> Result<()> {
        require_neq!(1u64, 1u64,);
        Ok(())
    }
    assert_eq!(hit().unwrap_err(), ErrorCode::RequireNeqViolated.into());
}

#[test]
fn three_arg_custom_errors_still_work() {
    fn error_code_ident() -> Result<()> {
        require_neq!(1u64, 1u64, RequireNeqViolated);
        Ok(())
    }
    assert_eq!(
        error_code_ident().unwrap_err(),
        ErrorCode::RequireNeqViolated.into()
    );
}
