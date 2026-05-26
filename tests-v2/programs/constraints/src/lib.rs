//! Test program covering the derive's account-constraint surface.
//!
//! One handler per constraint variant, keyed off a 1-byte discriminator.
//! Each `#[derive(Accounts)]` struct exercises one constraint in
//! isolation so the integration tests can trip it from a known state.
//!
//! Covered:
//!   - `address = expr` + `address = expr @ MyErr`
//!   - `has_one = field` + `has_one = field @ MyErr`
//!   - `owner = expr` + `owner = expr @ MyErr`
//!   - `constraint = expr` + `constraint = expr @ MyErr`
//!   - `executable`
//!   - `close = receiver`       (happy + self-close rejection)
//!   - `seeds::program = other` (cross-program PDA derivation)
//!   - `init_if_needed`         (create + reuse)
//!   - `zeroed`                 (pre-zeroed disc + non-zero rejection)
//!   - `#[account(signer)]` on `UncheckedAccount`

use anchor_lang_v2::prelude::*;

declare_id!("Con9ukTn9BRPXWcjS2UBbuN3NnCwy1hcaDNZ9Hb8QMNp");

/// Dummy program id used as the derivation domain for the
/// `seeds::program = OTHER_PROGRAM` override test. The PDA only has to be
/// verifiable under this key — it is never actually invoked.
pub const OTHER_PROGRAM: Address =
    Address::from_str_const("Gue5TpR6sstSyGhSvmVeH2TeKqBYYqmXpRCacB9jAk8u");

/// Expected address for the `address = PINNED_ADDRESS` check.
/// Pinned to a known off-curve pubkey — tests pass this exact address
/// on the happy path and a different one on the violation path.
pub const PINNED_ADDRESS: Address =
    Address::from_str_const("Pin1111111111111111111111111111111111111111");

// -- Custom error enum -------------------------------------------------------

#[error_code]
pub enum MyErr {
    #[msg("address did not match expected pinned value")]
    BadAddress,
    #[msg("has_one authority mismatch")]
    BadAuthority,
    #[msg("account is not owned by the expected program")]
    BadOwner,
    #[msg("arbitrary constraint expression was false")]
    BadConstraint,
    #[msg("first chained constraint was false")]
    BadFirst,
    #[msg("second chained constraint was false")]
    BadSecond,
}

// -- Account types -----------------------------------------------------------

#[account]
pub struct Data {
    pub authority: Address,
    pub value: u64,
}

#[account]
pub struct DataWithBump {
    pub value: u64,
    pub bump: u8,
    pub _padding: [u8; 7],
}

// -- Handlers ----------------------------------------------------------------

#[program]
pub mod constraints {
    use super::*;

    /// Create a `Data` PDA at `[b"data"]` with `authority = ctx.accounts.authority`.
    /// Used by has_one + close + constraint tests as a pre-existing account.
    #[discrim = 0]
    pub fn initialize(ctx: &mut Context<Initialize>) -> Result<()> {
        ctx.accounts.data.authority = *ctx.accounts.authority.account().address();
        ctx.accounts.data.value = 42;
        Ok(())
    }

    #[discrim = 1]
    pub fn check_address(_ctx: &mut Context<CheckAddress>) -> Result<()> {
        Ok(())
    }

    #[discrim = 2]
    pub fn check_address_custom_err(_ctx: &mut Context<CheckAddressCustomErr>) -> Result<()> {
        Ok(())
    }

    #[discrim = 3]
    pub fn check_has_one(_ctx: &mut Context<CheckHasOne>) -> Result<()> {
        Ok(())
    }

    #[discrim = 4]
    pub fn check_has_one_custom_err(_ctx: &mut Context<CheckHasOneCustomErr>) -> Result<()> {
        Ok(())
    }

    #[discrim = 5]
    pub fn check_owner(_ctx: &mut Context<CheckOwner>) -> Result<()> {
        Ok(())
    }

    #[discrim = 6]
    pub fn check_owner_custom_err(_ctx: &mut Context<CheckOwnerCustomErr>) -> Result<()> {
        Ok(())
    }

    #[discrim = 7]
    pub fn check_constraint(_ctx: &mut Context<CheckConstraint>) -> Result<()> {
        Ok(())
    }

    #[discrim = 8]
    pub fn check_constraint_custom_err(_ctx: &mut Context<CheckConstraintCustomErr>) -> Result<()> {
        Ok(())
    }

    #[discrim = 9]
    pub fn check_executable(_ctx: &mut Context<CheckExecutable>) -> Result<()> {
        Ok(())
    }

    /// Close `data`, sending its lamports to `receiver`.
    #[discrim = 10]
    pub fn do_close(_ctx: &mut Context<DoClose>) -> Result<()> {
        Ok(())
    }

    /// PDA derived against `OTHER_PROGRAM` rather than this program's id.
    #[discrim = 11]
    pub fn check_seeds_program(_ctx: &mut Context<CheckSeedsProgram>) -> Result<()> {
        Ok(())
    }

    /// First call creates the PDA; subsequent calls reuse it.
    #[discrim = 12]
    pub fn do_init_if_needed(ctx: &mut Context<DoInitIfNeeded>) -> Result<()> {
        ctx.accounts.data.value = ctx.accounts.data.value.wrapping_add(1);
        Ok(())
    }

    /// Expects a pre-allocated account whose first 8 bytes are zero.
    #[discrim = 13]
    pub fn check_zeroed(_ctx: &mut Context<CheckZeroed>) -> Result<()> {
        Ok(())
    }

    /// `signer` attribute on an `UncheckedAccount` — a distinct code path
    /// from the native `Signer` type check.
    #[discrim = 14]
    pub fn check_signer(_ctx: &mut Context<CheckSigner>) -> Result<()> {
        Ok(())
    }

    /// `#[account(address = <sibling>.<field>)]` — the v2 spelling that
    /// replaces `has_one` on the sibling account. Runtime check runs on
    /// the `authority` field; same semantics as handler 3's `has_one`
    /// path but reached via the `address` codegen branch.
    #[discrim = 15]
    pub fn check_address_field_path(_ctx: &mut Context<CheckAddressFieldPath>) -> Result<()> {
        Ok(())
    }

    /// Multiple `constraint`s on one field. Mixes the parenthesized
    /// `constraint(...)` form with the legacy `constraint = ...` form,
    /// with and without custom errors. Each check must fire in source
    /// order — the first violating check decides the surfaced error.
    #[discrim = 16]
    pub fn check_multiple_constraints(_ctx: &mut Context<CheckMultipleConstraints>) -> Result<()> {
        Ok(())
    }

    /// `address = <expr>` accepting an `Into<Address>` RHS. The fixture
    /// uses `[u8; 32]` (which has `From<[u8; 32]> for Address`) and a
    /// helper returning `&Address` (`From<&Address> for Address`) — both
    /// would have failed the prior `let __expected: Address = #expr`
    /// type-ascription path.
    #[discrim = 17]
    pub fn check_address_into(_ctx: &mut Context<CheckAddressInto>) -> Result<()> {
        Ok(())
    }

    /// Same but feeding `address` from a function returning `&Address`.
    #[discrim = 18]
    pub fn check_address_into_ref(_ctx: &mut Context<CheckAddressIntoRef>) -> Result<()> {
        Ok(())
    }

    /// First call creates the PDA with the canonical bump; subsequent
    /// calls verify against the bump stored in the account.
    #[discrim = 19]
    pub fn do_init_if_needed_explicit_bump(
        ctx: &mut Context<DoInitIfNeededExplicitBump>,
    ) -> Result<()> {
        ctx.accounts.data.bump = ctx.bumps.data;
        ctx.accounts.data.value = ctx.accounts.data.value.wrapping_add(1);
        Ok(())
    }

    /// `init` with an explicit `owner = ...` creates the account under
    /// that owner instead of this program id.
    #[discrim = 20]
    pub fn init_foreign_owned_unchecked(
        _ctx: &mut Context<InitForeignOwnedUnchecked>,
    ) -> Result<()> {
        Ok(())
    }
}

// -- Accounts structs --------------------------------------------------------

/// Init a Data PDA keyed by `[b"data"]`.
#[derive(Accounts)]
pub struct Initialize {
    #[account(mut)]
    pub payer: Signer,
    #[account(init, payer = payer, seeds = [b"data"], bump)]
    pub data: Account<Data>,
    /// Becomes the `authority` field of `Data`.
    pub authority: UncheckedAccount,
    pub system_program: Program<System>,
}

// 1. address = PINNED_ADDRESS
#[derive(Accounts)]
pub struct CheckAddress {
    #[account(address = PINNED_ADDRESS)]
    pub pinned: UncheckedAccount,
}

// 2. address = PINNED_ADDRESS @ MyErr::BadAddress
#[derive(Accounts)]
pub struct CheckAddressCustomErr {
    #[account(address = PINNED_ADDRESS @ MyErr::BadAddress)]
    pub pinned: UncheckedAccount,
}

// 3+4. has_one = authority [@ MyErr::BadAuthority]
//
// `has_one` is deprecated, so the derive emits a `deprecated` warning at
// each usage site. Wrapping the structs in a submodule gated by
// `#[expect(deprecated)]` both silences the warnings AND turns the
// expectation into a compile error if the derive ever stops emitting
// the deprecation — a free compile-time regression check. The
// `__client_accounts_*` helper modules the `#[program]` macro references
// are also inside the submodule, so the glob re-export below hoists
// everything back to crate root for the handler resolution.
#[expect(deprecated)]
mod has_one_structs {
    use super::*;

    #[derive(Accounts)]
    pub struct CheckHasOne {
        #[account(has_one = authority)]
        pub data: Account<Data>,
        pub authority: UncheckedAccount,
    }

    #[derive(Accounts)]
    pub struct CheckHasOneCustomErr {
        #[account(has_one = authority @ MyErr::BadAuthority)]
        pub data: Account<Data>,
        pub authority: UncheckedAccount,
    }
}
pub use has_one_structs::*;

// 5. owner = System
#[derive(Accounts)]
pub struct CheckOwner {
    #[account(owner = System::id())]
    pub target: UncheckedAccount,
}

// 6. owner = System @ MyErr::BadOwner
#[derive(Accounts)]
pub struct CheckOwnerCustomErr {
    #[account(owner = System::id() @ MyErr::BadOwner)]
    pub target: UncheckedAccount,
}

// 7. constraint = a.address() != b.address()
#[derive(Accounts)]
pub struct CheckConstraint {
    pub a: UncheckedAccount,
    #[account(constraint = a.address() != b.address())]
    pub b: UncheckedAccount,
}

// 8. constraint = ... @ MyErr::BadConstraint
#[derive(Accounts)]
pub struct CheckConstraintCustomErr {
    pub a: UncheckedAccount,
    #[account(constraint = a.address() != b.address() @ MyErr::BadConstraint)]
    pub b: UncheckedAccount,
}

// 9. executable
#[derive(Accounts)]
pub struct CheckExecutable {
    #[account(executable)]
    pub prog: UncheckedAccount,
}

// 10. close = receiver
#[derive(Accounts)]
pub struct DoClose {
    #[account(mut, seeds = [b"data"], bump, close = receiver)]
    pub data: Account<Data>,
    #[account(mut)]
    pub receiver: UncheckedAccount,
}

// 11. seeds::program = OTHER_PROGRAM
#[derive(Accounts)]
pub struct CheckSeedsProgram {
    #[account(seeds = [b"other"], bump, seeds::program = OTHER_PROGRAM)]
    pub pda: UncheckedAccount,
}

// 12. init_if_needed
#[derive(Accounts)]
pub struct DoInitIfNeeded {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init_if_needed,
        payer = payer,
        seeds = [b"maybe"],
        bump,
    )]
    pub data: Account<Data>,
    pub system_program: Program<System>,
}

#[derive(Accounts)]
pub struct DoInitIfNeededExplicitBump {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init_if_needed,
        payer = payer,
        seeds = [b"maybe-explicit"],
        bump = data.bump,
    )]
    pub data: Account<DataWithBump>,
    pub system_program: Program<System>,
}

// 13. zeroed
#[derive(Accounts)]
pub struct CheckZeroed {
    #[account(zeroed)]
    pub data: Account<Data>,
}

// 14. signer on UncheckedAccount
#[derive(Accounts)]
pub struct CheckSigner {
    #[account(signer)]
    pub user: UncheckedAccount,
}

/// `[u8; 32]` form of `PINNED_ADDRESS`. Used in the `Into<Address>` test
/// fixture so the RHS goes through `From<[u8; 32]> for Address` rather
/// than the trivial identity coercion.
pub const PINNED_BYTES: [u8; 32] = PINNED_ADDRESS.to_bytes();

/// Helper returning `&Address`. Exercises `From<&Address> for Address` on
/// the constraint RHS — different impl from the `[u8; 32]` form.
pub fn pinned_address_ref() -> &'static Address {
    &PINNED_ADDRESS
}

// 16. Multiple `constraint`s on a single field, mixing both spellings.
//
// Order of evaluation matters: the first failing check is the one whose
// error surfaces. The integration tests trip each entry in turn to
// confirm that.
// 17. address = <[u8; 32] expr> — RHS converts via `Into<Address>`.
#[derive(Accounts)]
pub struct CheckAddressInto {
    #[account(address = PINNED_BYTES)]
    pub pinned: UncheckedAccount,
}

// 18. address = <&Address expr> — RHS converts via `Into<Address>`.
#[derive(Accounts)]
pub struct CheckAddressIntoRef {
    #[account(address = pinned_address_ref())]
    pub pinned: UncheckedAccount,
}

#[derive(Accounts)]
pub struct CheckMultipleConstraints {
    pub a: UncheckedAccount,
    pub b: UncheckedAccount,
    #[account(
        constraint(a.address() != b.address() @ MyErr::BadFirst),
        constraint = a.address() != c.address() @ MyErr::BadSecond,
        constraint(b.address() != c.address()),
    )]
    pub c: UncheckedAccount,
}

// -- IDL-only fixtures -------------------------------------------------------
//
// The structs below exist purely to exercise IDL emission — they are not
// wired into `#[program]`, so they contribute no runtime surface. They
// cover spellings the runtime fixtures above don't need (or don't reach):
//
//   - `address = <dotted field path>` — IDL-only here; the runtime
//     fixture `CheckAddress` uses a const RHS.
//   - `init_if_needed` without `seeds` — in the IDL this path sets
//     `signer:true` on the fresh-keypair account (there is no PDA to
//     derive). The runtime `DoInitIfNeeded` uses `seeds = [b"maybe"]`, so
//     a dedicated struct is needed to hit the no-seeds branch.

/// Fixture for the v1-encodable `address = <sibling>.<self_name>` shape:
/// the subfield name (`authority`) matches the field holding the
/// constraint (`authority`), so v1's `has_one = authority` on `data`
/// expresses exactly the same check. IDL surfaces it as `relations`, not
/// `address`.
#[derive(Accounts)]
pub struct CheckAddressFieldPath {
    pub data: Account<Data>,
    #[account(address = data.authority)]
    pub authority: UncheckedAccount,
}

/// Fixture for the non-v1-encodable dotted-path shape: the subfield name
/// (`authority`) differs from the field holding the constraint
/// (`owner`), so no `has_one` spelling can express the check. IDL
/// surfaces the path verbatim under `address`.
#[derive(Accounts)]
pub struct CheckAddressFieldPathRenamed {
    pub data: Account<Data>,
    #[account(address = data.authority)]
    pub owner: UncheckedAccount,
}

/// Fixture for the `init_if_needed` no-seeds signer emission. The account
/// is a fresh keypair rather than a PDA, so the IDL marks it `signer:true`.
#[derive(Accounts)]
pub struct InitIfNeededNoSeeds {
    #[account(mut)]
    pub payer: Signer,
    #[account(init_if_needed, payer = payer)]
    pub data: Account<Data>,
    pub system_program: Program<System>,
}

/// Regression fixture for `#[account(init, owner = ...)]` on unchecked
/// accounts. The account is only allocated here; the foreign program owns
/// any later initialization of its bytes.
#[derive(Accounts)]
pub struct InitForeignOwnedUnchecked {
    #[account(mut)]
    pub payer: Signer,
    #[account(
        init,
        payer = payer,
        space = 8,
        owner = OTHER_PROGRAM,
        seeds = [b"foreign"],
        bump,
    )]
    pub foreign: UncheckedAccount,
    pub system_program: Program<System>,
}

// -- IDL emission tests ------------------------------------------------------
//
// Tests are parsed into the typed `anchor-lang-idl-spec` structs and
// asserted structurally — no substring matching — so a shape change in
// the derive's emitter fails the test at the impacted field rather than
// with a "got {full-json-blob}" dump.
//
// Only constraints that influence the IDL surface get tests here:
//   - `address = <expr>`        → `IdlInstructionAccount.address`
//   - `has_one = <field>`       → `IdlInstructionAccount.relations` (on the target)
//   - `init_if_needed` w/o seeds → `IdlInstructionAccount.signer`
//
// Constraints considered and intentionally skipped (runtime-only, no IDL
// surface): `zeroed`, `owner`, `constraint`, `executable`, `close`.
// `seeds` / `seeds::program` are already covered structurally
// by the `seeds` program fixture; adding a duplicate here would only
// retest the shared `classify_seed` path.
#[cfg(all(test, feature = "idl-build"))]
mod idl_tests {
    use {
        super::*,
        anchor_lang_idl_spec::{IdlInstructionAccount, IdlInstructionAccountItem},
    };

    /// Parse an `__idl_accounts()` JSON blob into the typed spec enum.
    /// Panics with the raw JSON on parse failure so regressions in the
    /// emitter surface as readable diagnostics.
    fn parse(json: &str) -> Vec<IdlInstructionAccountItem> {
        serde_json::from_str(json)
            .unwrap_or_else(|e| panic!("failed to parse accounts JSON: {e}\njson: {json}"))
    }

    /// Unwrap a `Single` account at the given index or panic. `Composite`
    /// would only appear for a `Nested<Inner>` field, which none of the
    /// fixtures below use.
    fn single(items: &[IdlInstructionAccountItem], idx: usize) -> &IdlInstructionAccount {
        match &items[idx] {
            IdlInstructionAccountItem::Single(a) => a,
            IdlInstructionAccountItem::Composite(_) => {
                panic!("expected Single at index {idx}, got Composite")
            }
        }
    }

    mod idl_address {
        use super::*;

        #[test]
        fn const_path_rhs_emits_address_verbatim() {
            let items = parse(&CheckAddress::__idl_accounts());
            let account = single(&items, 0);
            assert_eq!(account.name, "pinned");
            assert_eq!(account.address.as_deref(), Some("PINNED_ADDRESS"));
        }

        #[test]
        fn const_path_rhs_with_custom_err_emits_address_verbatim() {
            // The `@ MyErr::BadAddress` error override is a runtime
            // concern; the IDL output for this spelling must match the
            // plain `address = PINNED_ADDRESS` variant exactly.
            let items = parse(&CheckAddressCustomErr::__idl_accounts());
            let account = single(&items, 0);
            assert_eq!(account.name, "pinned");
            assert_eq!(account.address.as_deref(), Some("PINNED_ADDRESS"));
        }

        #[test]
        fn v1_encodable_field_path_emits_relation_not_address() {
            // `address = data.authority` on field `authority` — the
            // subfield name matches the field's own ident, so v1's
            // `has_one = authority` on `data` expresses the same check.
            // Emitter routes it through `relations` for shape parity.
            let items = parse(&CheckAddressFieldPath::__idl_accounts());
            let data = single(&items, 0);
            assert_eq!(data.name, "data");
            // The source of the relation carries no `relations` entry.
            assert!(data.relations.is_empty());
            let authority = single(&items, 1);
            assert_eq!(authority.name, "authority");
            assert_eq!(authority.address, None);
            assert_eq!(
                authority.relations,
                vec!["data".to_string()],
                "expected v1-encodable address to surface as a relation"
            );
        }

        #[test]
        fn non_v1_encodable_field_path_emits_dotted_address() {
            // `address = data.authority` on field `owner` — the subfield
            // name (`authority`) doesn't match the field's ident, which
            // no `has_one` spelling in v1 can express. The path falls
            // through to the `address` key verbatim for client-side
            // resolution.
            let items = parse(&CheckAddressFieldPathRenamed::__idl_accounts());
            let data = single(&items, 0);
            assert_eq!(data.name, "data");
            assert!(data.relations.is_empty());
            let owner = single(&items, 1);
            assert_eq!(owner.name, "owner");
            assert!(owner.relations.is_empty());
            assert_eq!(owner.address.as_deref(), Some("data.authority"));
        }
    }

    mod idl_has_one {
        use super::*;

        #[test]
        fn has_one_emits_relation_on_target_field() {
            // `has_one = authority` on the `data` field produces an
            // inverse mapping: the `authority` account lists `data` in
            // its `relations[]`. Clients use this to know which sibling
            // to look up the authority pubkey on.
            let items = parse(&CheckHasOne::__idl_accounts());
            let data = single(&items, 0);
            assert_eq!(data.name, "data");
            // The source of the relation carries no `relations` entry.
            assert!(
                data.relations.is_empty(),
                "expected no relations on source field `data`, got {:?}",
                data.relations
            );
            let authority = single(&items, 1);
            assert_eq!(authority.name, "authority");
            assert_eq!(
                authority.relations,
                vec!["data".to_string()],
                "expected `relations:[\"data\"]` on the has_one target",
            );
        }

        #[test]
        fn has_one_with_custom_err_emits_same_relation() {
            // The `@ MyErr::BadAuthority` override is a runtime-only
            // concern; it must not change the IDL shape.
            let items = parse(&CheckHasOneCustomErr::__idl_accounts());
            let authority = single(&items, 1);
            assert_eq!(authority.name, "authority");
            assert_eq!(authority.relations, vec!["data".to_string()]);
        }
    }

    mod idl_init_if_needed {
        use super::*;

        #[test]
        fn init_if_needed_without_seeds_marks_account_as_signer() {
            // With no `seeds`, the fresh account must be signed by a
            // keypair the client generates at request time. The IDL
            // surfaces that as `signer:true` so the client builder
            // knows to add the signature.
            let items = parse(&InitIfNeededNoSeeds::__idl_accounts());
            let data = single(&items, 1);
            assert_eq!(data.name, "data");
            assert!(
                data.signer,
                "expected `signer:true` on fresh-keypair init_if_needed account",
            );
            assert!(
                data.pda.is_none(),
                "expected no `pda` entry on seedless init_if_needed account",
            );
        }

        #[test]
        fn init_if_needed_with_seeds_does_not_mark_signer() {
            // Control case: when `seeds` is present the account is a
            // PDA, and PDAs cannot sign. The IDL must leave `signer`
            // unset so the client doesn't attempt to add a signature.
            let items = parse(&DoInitIfNeeded::__idl_accounts());
            let data = single(&items, 1);
            assert_eq!(data.name, "data");
            assert!(
                !data.signer,
                "expected `signer:false` on PDA init_if_needed account",
            );
            assert!(
                data.pda.is_some(),
                "expected `pda` entry to be populated from seeds",
            );
        }
    }

    /// `#[error_code]` IDL surface. Every enum exposes
    /// `pub fn __idl_errors() -> String` (mirroring `__idl_accounts()`),
    /// so the tests parse the JSON with the typed `IdlErrorCode` spec
    /// and assert structurally rather than against literal strings.
    /// Each test that exercises auto-numbering also cross-checks the
    /// runtime `From<E> for Error` path, since the IDL `code` and
    /// `Error::Custom(code)` must agree for clients to surface the
    /// right error name.
    mod idl_errors {
        use {super::*, anchor_lang_idl_spec::IdlErrorCode};

        fn parse_errors(json: &str) -> Vec<IdlErrorCode> {
            serde_json::from_str(json)
                .unwrap_or_else(|e| panic!("failed to parse errors JSON: {e}\njson: {json}"))
        }

        fn custom_code(e: Error) -> u32 {
            match e {
                Error::Custom(c) => c,
                other => panic!("expected Error::Custom, got {other:?}"),
            }
        }

        #[test]
        fn fixture_my_err_default_offset_with_msgs() {
            // `MyErr` is the program's actual error enum — every variant
            // has a `#[msg("...")]` and uses default sequential numbering
            // from the v1-compatible 6000 offset.
            let errors = parse_errors(&MyErr::__idl_errors());
            let names: Vec<&str> = errors.iter().map(|e| e.name.as_str()).collect();
            assert_eq!(
                names,
                vec![
                    "BadAddress",
                    "BadAuthority",
                    "BadOwner",
                    "BadConstraint",
                    "BadFirst",
                    "BadSecond",
                ]
            );
            for (i, e) in errors.iter().enumerate() {
                assert_eq!(e.code, 6000 + i as u32);
                assert!(e.msg.is_some(), "{} should carry a msg", e.name);
            }
            assert_eq!(
                errors[0].msg.as_deref(),
                Some("address did not match expected pinned value")
            );
        }

        #[test]
        fn from_impl_matches_idl_code_for_fixture() {
            // The runtime path emits `Error::Custom(e as u32 + offset)`;
            // the IDL advertises `offset + discriminator`. They must
            // agree for every variant.
            let errors = parse_errors(&MyErr::__idl_errors());
            assert_eq!(custom_code(MyErr::BadAddress.into()), errors[0].code);
            assert_eq!(custom_code(MyErr::BadAuthority.into()), errors[1].code);
            assert_eq!(custom_code(MyErr::BadSecond.into()), errors[5].code);
        }

        #[error_code(offset = 7000)]
        pub enum CustomOffset {
            A,
            B,
        }

        #[test]
        fn custom_offset_shifts_codes() {
            let errors = parse_errors(&CustomOffset::__idl_errors());
            assert_eq!(
                errors.iter().map(|e| e.code).collect::<Vec<_>>(),
                vec![7000, 7001]
            );
            assert!(errors.iter().all(|e| e.msg.is_none()));
            assert_eq!(custom_code(CustomOffset::A.into()), 7000);
            assert_eq!(custom_code(CustomOffset::B.into()), 7001);
        }

        #[error_code]
        pub enum AllExplicit {
            Foo = 10,
            Bar = 20,
            Baz = 30,
        }

        #[test]
        fn all_explicit_discriminators() {
            let errors = parse_errors(&AllExplicit::__idl_errors());
            assert_eq!(
                errors.iter().map(|e| e.code).collect::<Vec<_>>(),
                vec![6010, 6020, 6030]
            );
            assert_eq!(custom_code(AllExplicit::Foo.into()), 6010);
            assert_eq!(custom_code(AllExplicit::Bar.into()), 6020);
            assert_eq!(custom_code(AllExplicit::Baz.into()), 6030);
        }

        #[error_code]
        pub enum MixedDiscrim {
            Alpha = 100,
            Beta,
            Gamma = 200,
            Delta,
            Epsilon,
        }

        #[test]
        fn explicit_then_implicit_auto_increments() {
            // After an explicit discriminant, subsequent unannotated
            // variants pick up at `prev + 1`. The macro replicates Rust's
            // own enum semantics, so `as u32` and the IDL agree.
            let errors = parse_errors(&MixedDiscrim::__idl_errors());
            assert_eq!(
                errors.iter().map(|e| e.code).collect::<Vec<_>>(),
                vec![6100, 6101, 6200, 6201, 6202]
            );
            assert_eq!(custom_code(MixedDiscrim::Beta.into()), 6101);
            assert_eq!(custom_code(MixedDiscrim::Delta.into()), 6201);
        }

        #[error_code]
        pub enum PartialMsg {
            Plain,
            #[msg("hello")]
            Greeted,
            Silent,
        }

        #[test]
        fn variants_without_msg_omit_field() {
            // `IdlErrorCode.msg` is `Option<String>` with
            // `skip_serializing_if`; the absence of `#[msg]` must round-
            // trip back through serde as `None`, not `Some("")`.
            let errors = parse_errors(&PartialMsg::__idl_errors());
            assert_eq!(errors[0].msg, None);
            assert_eq!(errors[1].msg.as_deref(), Some("hello"));
            assert_eq!(errors[2].msg, None);
        }

        #[error_code]
        pub enum Escapes {
            #[msg("a \"quoted\" word")]
            Quoted,
            #[msg("a backslash \\ here")]
            Backslash,
            #[msg("both \\\" together")]
            Both,
        }

        #[test]
        fn json_escaping_round_trips_through_serde() {
            // The macro escapes `"` and `\`; serde unescapes them on the
            // way back. A regression where the macro emitted invalid JSON
            // would surface as a parse failure here, not a string compare.
            let errors = parse_errors(&Escapes::__idl_errors());
            assert_eq!(errors[0].msg.as_deref(), Some(r#"a "quoted" word"#));
            assert_eq!(errors[1].msg.as_deref(), Some(r"a backslash \ here"));
            assert_eq!(errors[2].msg.as_deref(), Some(r#"both \" together"#));
        }

        #[error_code(offset = 0)]
        pub enum ZeroOffset {
            Zero,
            One,
        }

        #[test]
        fn zero_offset_boundary() {
            let errors = parse_errors(&ZeroOffset::__idl_errors());
            assert_eq!(
                errors.iter().map(|e| e.code).collect::<Vec<_>>(),
                vec![0, 1]
            );
            assert_eq!(custom_code(ZeroOffset::Zero.into()), 0);
        }
    }
}
