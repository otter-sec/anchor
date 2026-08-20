use {
    anchor_syn::AccountsStruct,
    quote::ToTokens,
};

#[test]
fn test_realloc_zero_codegen() {
    let accs: AccountsStruct = syn::parse_str(
        r#"
        pub struct ReallocTest<'info> {
            #[account(mut, realloc = 100, realloc::payer = payer, realloc::zero = true)]
            pub data: Account<'info, MyData>,
            #[account(mut)]
            pub payer: Signer<'info>,
            pub system_program: Program<'info, System>,
        }
        "#,
    )
    .unwrap();

    let tokens = accs.to_token_stream().to_string();
    assert!(tokens.contains("realloc (100 , true)"));
}

#[test]
fn test_realloc_zero_false_codegen() {
    let accs: AccountsStruct = syn::parse_str(
        r#"
        pub struct ReallocTest<'info> {
            #[account(mut, realloc = 100, realloc::payer = payer, realloc::zero = false)]
            pub data: Account<'info, MyData>,
            #[account(mut)]
            pub payer: Signer<'info>,
            pub system_program: Program<'info, System>,
        }
        "#,
    )
    .unwrap();

    let tokens = accs.to_token_stream().to_string();
    assert!(tokens.contains("realloc (100 , false)"));
}
