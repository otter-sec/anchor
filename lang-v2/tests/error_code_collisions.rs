//! Verifies that no two `ErrorCode` variants map to the same
//! `ProgramError::Custom(u32)` code.
//!
//! Collision between two custom codes would make on-chain error
//! reporting ambiguous — a client decoding a `Custom(2003)` error
//! can't tell which constraint failed.
//!
//! Note: multiple variants mapping to the same *built-in*
//! `ProgramError` is acceptable only when the variants mean the same
//! thing to a caller (e.g., `InstructionDidNotDeserialize` and
//! `InstructionFallbackNotFound` both → `InvalidInstructionData`:
//! either way the instruction data was wrong). Account-relation
//! constraints (`has_one`, `address`, `close`) instead get distinct
//! v1-aligned `Custom` codes — sharing `InvalidAccountData` made
//! their failures indistinguishable from each other and from
//! discriminator/deserialization errors (#4849). The invariant is
//! specifically "no two variants share a `Custom(u32)` code".
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
            | ErrorCode::ConstraintClose
            | ErrorCode::ConstraintOwner
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
        ("ConstraintClose", ErrorCode::ConstraintClose),
        ("ConstraintOwner", ErrorCode::ConstraintOwner),
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
        custom_count, 15,
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
        "ConstraintOwner",
        ErrorCode::ConstraintOwner.into(),
        IllegalOwner,
    );
    check(
        "DeclaredProgramIdMismatch",
        ErrorCode::DeclaredProgramIdMismatch.into(),
        IncorrectProgramId,
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

#[test]
fn custom_codes_match_v1_numbering() {
    // Every `Custom` code must equal the discriminant of the same-named
    // variant in v1's `ErrorCode` (lang/src/error.rs), so clients and
    // explorers that already decode v1 codes read v2 errors correctly.
    // Snapshot test — a mismatch here is a breaking API change.
    let expected: Vec<(&'static str, ErrorCode, u32)> = vec![
        ("ConstraintMut", ErrorCode::ConstraintMut, 2000),
        ("ConstraintHasOne", ErrorCode::ConstraintHasOne, 2001),
        ("ConstraintRaw", ErrorCode::ConstraintRaw, 2003),
        (
            "ConstraintExecutable",
            ErrorCode::ConstraintExecutable,
            2007,
        ),
        ("ConstraintClose", ErrorCode::ConstraintClose, 2011),
        ("ConstraintAddress", ErrorCode::ConstraintAddress, 2012),
        ("ConstraintZero", ErrorCode::ConstraintZero, 2013),
        (
            "ConstraintDuplicateMutableAccount",
            ErrorCode::ConstraintDuplicateMutableAccount,
            2040,
        ),
        ("RequireViolated", ErrorCode::RequireViolated, 2500),
        ("RequireEqViolated", ErrorCode::RequireEqViolated, 2501),
        ("RequireNeqViolated", ErrorCode::RequireNeqViolated, 2502),
        (
            "RequireKeysEqViolated",
            ErrorCode::RequireKeysEqViolated,
            2503,
        ),
        (
            "RequireKeysNeqViolated",
            ErrorCode::RequireKeysNeqViolated,
            2504,
        ),
        ("RequireGtViolated", ErrorCode::RequireGtViolated, 2505),
        ("RequireGteViolated", ErrorCode::RequireGteViolated, 2506),
    ];
    // Keep this list in sync with the Custom-code count snapshot above.
    assert_eq!(expected.len(), 15);
    for (label, code, expected_code) in expected {
        let err: ProgramError = code.into();
        assert_eq!(
            err,
            ProgramError::Custom(expected_code),
            "ErrorCode::{} should map to Custom({}) to match v1 numbering",
            label,
            expected_code
        );
    }
}
