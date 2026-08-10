//! Verifies that no two `ErrorCode` variants map to the same
//! `ProgramError::Custom(u32)` code.
//!
//! Collision between two custom codes would make on-chain error
//! reporting ambiguous — a client decoding a `Custom(2003)` error
//! can't tell which constraint failed.
//!
//! Note: some variants intentionally map to built-in `ProgramError`s
//! (for example `ConstraintSigner` → `MissingRequiredSignature`).
//! The invariant is specifically "no two variants share a
//! `Custom(u32)` code".
//!
//! Exhaustiveness: the helper below uses an `#[allow(dead_code)]`
//! match on a fresh `ErrorCode` parameter with explicit arms for
//! every variant. If a new `ErrorCode` variant is added without
//! being listed there, the match goes non-exhaustive and the test
//! crate fails to compile — forcing a review.

use anchor_lang_v2::ErrorCode;
use solana_program_error::ProgramError;

// Exhaustive variant list. The inner `exhaustiveness_check` fn uses a
// non-wildcard match that the compiler verifies covers every variant
// of `ErrorCode` — adding a new variant without updating the match
// is a compile error, and we keep the labeled vec in the same order
// so reviewers see both halves together.
fn all_labeled_variants() -> Vec<(&'static str, ErrorCode)> {
    // Compile-time exhaustiveness guard. The match must cover every
    // `ErrorCode` variant; a missing arm is a compile error. Actually
    // called below (with a fixed sentinel) so the fn isn't dead code.
    fn exhaustiveness_check(code: ErrorCode) {
        match code {
            ErrorCode::AccountNotEnoughKeys
            | ErrorCode::ConstraintMut
            | ErrorCode::ConstraintSigner
            | ErrorCode::ConstraintSeeds
            | ErrorCode::ConstraintHasOne
            | ErrorCode::ConstraintAddress
            | ErrorCode::ConstraintAccountIsNone
            | ErrorCode::ConstraintClose
            | ErrorCode::ConstraintOwner
            | ErrorCode::ConstraintSpace
            | ErrorCode::ConstraintRentExempt
            | ErrorCode::ConstraintRaw
            | ErrorCode::ConstraintExecutable
            | ErrorCode::ConstraintZero
            | ErrorCode::InstructionDidNotDeserialize
            | ErrorCode::DeclaredProgramIdMismatch
            | ErrorCode::InstructionFallbackNotFound
            | ErrorCode::RequireViolated
            | ErrorCode::RequireEqViolated
            | ErrorCode::RequireNeqViolated
            | ErrorCode::RequireKeysEqViolated
            | ErrorCode::RequireKeysNeqViolated
            | ErrorCode::RequireGtViolated
            | ErrorCode::RequireGteViolated
            | ErrorCode::ConstraintDuplicateMutableAccount => (),
        }
    }
    exhaustiveness_check(ErrorCode::AccountNotEnoughKeys);

    vec![
        ("AccountNotEnoughKeys", ErrorCode::AccountNotEnoughKeys),
        ("ConstraintMut", ErrorCode::ConstraintMut),
        ("ConstraintSigner", ErrorCode::ConstraintSigner),
        ("ConstraintSeeds", ErrorCode::ConstraintSeeds),
        ("ConstraintHasOne", ErrorCode::ConstraintHasOne),
        ("ConstraintAddress", ErrorCode::ConstraintAddress),
        (
            "ConstraintAccountIsNone",
            ErrorCode::ConstraintAccountIsNone,
        ),
        ("ConstraintClose", ErrorCode::ConstraintClose),
        ("ConstraintOwner", ErrorCode::ConstraintOwner),
        ("ConstraintSpace", ErrorCode::ConstraintSpace),
        ("ConstraintRentExempt", ErrorCode::ConstraintRentExempt),
        ("ConstraintRaw", ErrorCode::ConstraintRaw),
        ("ConstraintExecutable", ErrorCode::ConstraintExecutable),
        ("ConstraintZero", ErrorCode::ConstraintZero),
        (
            "InstructionDidNotDeserialize",
            ErrorCode::InstructionDidNotDeserialize,
        ),
        (
            "DeclaredProgramIdMismatch",
            ErrorCode::DeclaredProgramIdMismatch,
        ),
        (
            "InstructionFallbackNotFound",
            ErrorCode::InstructionFallbackNotFound,
        ),
        ("RequireViolated", ErrorCode::RequireViolated),
        ("RequireEqViolated", ErrorCode::RequireEqViolated),
        ("RequireNeqViolated", ErrorCode::RequireNeqViolated),
        ("RequireKeysEqViolated", ErrorCode::RequireKeysEqViolated),
        ("RequireKeysNeqViolated", ErrorCode::RequireKeysNeqViolated),
        ("RequireGtViolated", ErrorCode::RequireGtViolated),
        ("RequireGteViolated", ErrorCode::RequireGteViolated),
        (
            "ConstraintDuplicateMutableAccount",
            ErrorCode::ConstraintDuplicateMutableAccount,
        ),
    ]
}

#[test]
fn no_two_variants_share_a_custom_code() {
    let mut seen: Vec<(u32, &'static str)> = Vec::new();
    let mut custom_count = 0;
    for (label, code) in all_labeled_variants() {
        let err: ProgramError = code.into();
        if let ProgramError::Custom(n) = err {
            custom_count += 1;
            if let Some((_, prior)) = seen.iter().find(|(k, _)| *k == n) {
                panic!(
                    "Custom error code collision: {} maps to Custom({}) — \
                     already claimed by {}",
                    label, n, prior
                );
            }
            seen.push((n, label));
        }
    }
    // Snapshot — adding a new Custom variant forces a review of this number.
    assert_eq!(
        custom_count, 18,
        "Number of Custom error codes changed; update this snapshot after review"
    );
}

#[test]
fn custom_codes_are_in_reserved_ranges() {
    // Documented reserved ranges:
    //   2000-2499 — constraint failures
    //   2500-2999 — require! macro violations
    //
    // Codes outside these ranges may collide with v1 or downstream programs.
    for (label, code) in all_labeled_variants() {
        let err: ProgramError = code.into();
        if let ProgramError::Custom(n) = err {
            let in_constraints = (2000..=2499).contains(&n);
            let in_requires = (2500..=2999).contains(&n);
            assert!(
                in_constraints || in_requires,
                "ErrorCode::{} maps to Custom({}), outside the documented \
                 ranges [2000-2499] (constraints) and [2500-2999] (require!)",
                label,
                n
            );
        }
    }
}

#[test]
fn builtin_groupings_are_stable() {
    // These groupings are documented — changing them is a breaking
    // change for clients decoding errors. Snapshot test.
    use ProgramError::*;

    fn check(label: &str, actual: ProgramError, expected: ProgramError) {
        assert_eq!(
            actual, expected,
            "ErrorCode::{} mapping changed — this is a breaking API change",
            label
        );
    }

    check(
        "AccountNotEnoughKeys",
        ErrorCode::AccountNotEnoughKeys.into(),
        NotEnoughAccountKeys,
    );
    check(
        "ConstraintSigner",
        ErrorCode::ConstraintSigner.into(),
        MissingRequiredSignature,
    );
    check(
        "ConstraintSeeds",
        ErrorCode::ConstraintSeeds.into(),
        InvalidSeeds,
    );
    check(
        "ConstraintAccountIsNone",
        ErrorCode::ConstraintAccountIsNone.into(),
        Custom(2020),
    );
    check(
        "ConstraintOwner",
        ErrorCode::ConstraintOwner.into(),
        IllegalOwner,
    );
    check(
        "DeclaredProgramIdMismatch",
        ErrorCode::DeclaredProgramIdMismatch.into(),
        IncorrectProgramId,
    );
    check(
        "RequireViolated",
        ErrorCode::RequireViolated.into(),
        Custom(2500),
    );
    check(
        "RequireEqViolated",
        ErrorCode::RequireEqViolated.into(),
        Custom(2501),
    );
    check(
        "RequireKeysEqViolated",
        ErrorCode::RequireKeysEqViolated.into(),
        Custom(2502),
    );
    check(
        "RequireNeqViolated",
        ErrorCode::RequireNeqViolated.into(),
        Custom(2503),
    );
    check(
        "RequireKeysNeqViolated",
        ErrorCode::RequireKeysNeqViolated.into(),
        Custom(2504),
    );
    check(
        "RequireGtViolated",
        ErrorCode::RequireGtViolated.into(),
        Custom(2505),
    );
    check(
        "RequireGteViolated",
        ErrorCode::RequireGteViolated.into(),
        Custom(2506),
    );

    check(
        "ConstraintHasOne",
        ErrorCode::ConstraintHasOne.into(),
        Custom(2001),
    );
    check(
        "ConstraintAddress",
        ErrorCode::ConstraintAddress.into(),
        Custom(2012),
    );
    check(
        "ConstraintClose",
        ErrorCode::ConstraintClose.into(),
        Custom(2011),
    );

    // Dedicated constraint custom code
    check(
        "ConstraintSpace",
        ErrorCode::ConstraintSpace.into(),
        Custom(2019),
    );

    // Grouped under InvalidInstructionData
    check(
        "InstructionDidNotDeserialize",
        ErrorCode::InstructionDidNotDeserialize.into(),
        InvalidInstructionData,
    );
    check(
        "InstructionFallbackNotFound",
        ErrorCode::InstructionFallbackNotFound.into(),
        InvalidInstructionData,
    );
}
