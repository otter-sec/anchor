use anchor_syn::AccountsStruct;

#[test]
fn test_migration_requires_mut_constraint() {
    let result = syn::parse_str::<AccountsStruct>(
        r#"
        pub struct MigrateTest<'info> {
            pub state: Migration<'info, V1, V2>,
        }
        "#,
    );

    assert!(result.is_err());
    let err = result.unwrap_err().to_string();
    assert!(err.contains("Migration accounts must be mutable"));
}

#[test]
fn test_migration_with_mut_constraint_succeeds() {
    let result = syn::parse_str::<AccountsStruct>(
        r#"
        pub struct MigrateTest<'info> {
            #[account(mut)]
            pub state: Migration<'info, V1, V2>,
        }
        "#,
    );

    assert!(result.is_ok());
}
