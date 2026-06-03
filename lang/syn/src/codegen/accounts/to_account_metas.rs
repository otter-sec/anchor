use {
    crate::{AccountField, AccountsStruct},
    quote::quote,
};

// Generates the `ToAccountMetas` trait implementation.
pub fn generate(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let name = &accs.ident;

    let to_acc_metas: Vec<proc_macro2::TokenStream> = accs
        .fields
        .iter()
        .map(|f: &AccountField| {
            let (name, is_signer, is_optional) = match f {
                AccountField::CompositeField(s) => (&s.ident, quote! {None}, false),
                AccountField::Field(f) => {
                    let is_signer = match f.constraints.is_signer() {
                        false => quote! {None},
                        true => quote! {Some(true)},
                    };
                    (&f.ident, is_signer, f.is_optional)
                }
            };
            if is_optional {
                quote! {
                    if let Some(#name) = &self.#name {
                        account_metas.extend(#name.to_account_metas(#is_signer));
                    } else {
                        account_metas.push(AccountMeta::new_readonly(crate::ID, false));
                    }
                }
            } else {
                quote! {
                    account_metas.extend(self.#name.to_account_metas(#is_signer));
                }
            }
        })
        .collect();

    let (impl_gen, ty_gen, where_clause) = accs.generics.split_for_impl();

    quote! {
        #[automatically_derived]
        impl #impl_gen anchor_lang::ToAccountMetas for #name #ty_gen #where_clause{
            fn to_account_metas(&self, is_signer: Option<bool>) -> Vec<anchor_lang::solana_program::instruction::AccountMeta> {
                let mut account_metas = vec![];

                #(#to_acc_metas)*

                account_metas
            }
        }
    }
}
