#[test]
fn fallback_accepts_qualified_type_paths() {
    let program = syn::parse_str::<anchor_syn::Program>(
        r#"
        pub mod example {
            pub fn default<'info>(
                _program_id: &anchor_lang::prelude::Pubkey,
                _accounts: &[anchor_lang::prelude::AccountInfo<'info>],
                _data: &[u8],
            ) -> anchor_lang::Result<()> {
                Ok(())
            }
        }
        "#,
    )
    .unwrap();

    assert!(program.fallback_fn.is_some());
}

#[test]
fn fallback_accepts_type_aliases() {
    let program = syn::parse_str::<anchor_syn::Program>(
        r#"
        pub mod example {
            type ProgramId = Pubkey;
            type AccountInfos<'info> = [AccountInfo<'info>];
            type InstructionData = [u8];

            pub fn default<'info>(
                _program_id: &ProgramId,
                _accounts: &AccountInfos<'info>,
                _data: &InstructionData,
            ) -> Result<()> {
                Ok(())
            }
        }
        "#,
    )
    .unwrap();

    assert!(program.fallback_fn.is_some());
}

#[test]
fn underscore_instruction_arg_is_rejected() {
    let program = syn::parse_str::<anchor_syn::Program>(
        r#"
        pub mod example {
            pub fn initialize(ctx: Context<Initialize>, _: u8) -> Result<()> {
                Ok(())
            }
        }
        "#,
    );

    let err = program.unwrap_err().to_string();
    assert_eq!(err, "expected named argument");
}

#[test]
fn context_accepts_module_qualified_accounts_paths() {
    let program = syn::parse_str::<anchor_syn::Program>(
        r#"
        pub mod example {
            pub fn init(ctx: Context<instructions::init::Init>) -> Result<()> {
                Ok(())
            }

            pub fn update(ctx: Context<crate::instructions::Update<'info>>) -> Result<()> {
                Ok(())
            }
        }
        "#,
    )
    .unwrap();

    let mut ixs = program.ixs.iter();
    // `anchor_ident` is the last segment of the path, even for
    // module-qualified (and `crate::`-prefixed, generic) paths.
    assert_eq!(ixs.next().unwrap().anchor_ident, "Init");
    assert_eq!(ixs.next().unwrap().anchor_ident, "Update");
}

#[test]
fn context_rejects_same_named_accounts_structs_in_different_modules() {
    let program = syn::parse_str::<anchor_syn::Program>(
        r#"
        pub mod example {
            pub fn foo(ctx: Context<a::Init>) -> Result<()> {
                Ok(())
            }

            pub fn bar(ctx: Context<b::Init>) -> Result<()> {
                Ok(())
            }
        }
        "#,
    );

    let err = program.unwrap_err().to_string();
    assert!(err.contains("two `Accounts` structs named `Init`"), "{err}");
}

#[test]
fn context_rejects_super_qualified_accounts_paths() {
    let program = syn::parse_str::<anchor_syn::Program>(
        r#"
        pub mod example {
            pub fn init(ctx: Context<super::Init>) -> Result<()> {
                Ok(())
            }
        }
        "#,
    );

    assert!(program.is_err());
}
