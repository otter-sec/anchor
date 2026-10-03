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
fn context_boxed_accounts_is_accepted() {
    let program = syn::parse_str::<anchor_syn::Program>(
        r#"
        pub mod example {
            pub fn initialize(ctx: Context<Box<Initialize>>) -> Result<()> {
                Ok(())
            }
        }
        "#,
    )
    .unwrap();

    assert_eq!(program.ixs.len(), 1);
    assert_eq!(program.ixs[0].anchor_ident.to_string(), "Initialize");
}

