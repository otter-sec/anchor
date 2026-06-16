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

#[derive(Accounts)]
pub struct Main<'info> {
    pub a: mod_a::Shared<'info>,
    pub b: mod_b::Shared<'info>,
    // Test crate-relative path
    pub c: crate::mod_a::Shared<'info>,
}

#[test]
fn test_path_hygiene_collisions() {
    // This test primarily checks for compilation.
    // If there's a name collision in the generated __client_accounts_main module,
    // it will fail to compile.
}

mod mod_c {
    use super::*;

    #[derive(Accounts)]
    pub struct Composite<'info> {
        pub a: mod_a::Shared<'info>,
    }

    #[test]
    pub fn test_visibility() {
        // This should compile because we made the helper module `pub`
        let _ = crate::mod_a::__client_accounts_shared::Shared::default();

        // We just need to check the type exists and is accessible.
        // We use a dummy function to check type without instantiating it with invalid values.
        fn _test_type(_: crate::mod_a::__cpi_client_accounts_shared::Shared) {}
    }
}
