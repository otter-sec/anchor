fn main() {
    let src = r#"
        pub struct FuncOne<'info> {
            /// CHECK: This account is checked!
            #[account(mut)]
            pub my_account: UncheckedAccount<'info>,
        }
    "#;
    let file = syn::parse_file(src).unwrap();
    let s = match &file.items[0] {
        syn::Item::Struct(s) => s,
        _ => panic!(),
    };
    let f = &s.fields.iter().next().unwrap();
    let is_documented = f.attrs.iter().any(|attr| {
        if let syn::Meta::NameValue(syn::MetaNameValue {
            value:
                syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(s),
                    ..
                }),
            ..
        }) = &attr.meta
        {
            s.value().contains("CHECK")
        } else {
            false
        }
    });
    println!("is_documented: {}", is_documented);
}
