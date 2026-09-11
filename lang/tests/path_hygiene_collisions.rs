use anchor_lang::prelude::*;

mod mod_a {
    use super::*;
    #[derive(Accounts)]
    pub struct Shared<'info> {
        pub account: UncheckedAccount<'info>,
    }
}

mod mod_b {
    use super::*;
    #[derive(Accounts)]
    pub struct Shared<'info> {
        pub account: UncheckedAccount<'info>,
    }
}

// Test: two composite fields with the same struct name from different modules.
// The generated __client_accounts_main must NOT produce "name defined multiple times".
#[derive(Accounts)]
pub struct Main<'info> {
    pub a: mod_a::Shared<'info>,
    pub b: mod_b::Shared<'info>,
    // Test crate-relative path
    pub c: crate::mod_a::Shared<'info>,
}

#[test]
fn test_path_hygiene_collisions() {
    // Compilation verifies that our re-export deduplication works.
}

// Test: a module that defines a struct whose name matches an Anchor primitive.
// `nested::CustomAccounts` contains an `anchor_lang::prelude::Signer` field —
// it should be treated as a composite (Accounts) field, not as Ty::Signer.
mod nested {
    use super::*;

    #[derive(Accounts)]
    pub struct CustomAccounts<'info> {
        pub authority: Signer<'info>,
    }
}

#[derive(Accounts)]
pub struct UsesNested<'info> {
    // Must be parsed as composite — NOT a primitive type.
    pub inner: nested::CustomAccounts<'info>,
}

#[test]
fn test_nested_composite_accounts_parsed_correctly() {
    // Compilation verifies that nested::CustomAccounts is a composite field.
}

mod mod_c {
    use super::*;

    #[derive(Accounts)]
    pub struct Composite<'info> {
        pub a: mod_a::Shared<'info>,
    }

    #[test]
    pub fn test_visibility() {
        // Verifies helper modules are `pub` and accessible externally.
        let _ = crate::mod_a::__client_accounts_shared::Shared::default();

        fn _test_type(_: crate::mod_a::__cpi_client_accounts_shared::Shared) {}
    }
}
