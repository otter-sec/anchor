use {
    crate::{
        codegen::accounts::{generics, ParsedGenerics},
        AccountField, AccountsStruct,
    },
    quote::quote,
};

// Generates the `ToAccountInfos` trait implementation.
pub fn generate(accs: &AccountsStruct) -> proc_macro2::TokenStream {
    let name = &accs.ident;
    let ParsedGenerics {
        combined_generics,
        trait_generics,
        struct_generics,
        where_clause,
    } = generics(accs);

    let to_acc_infos: Vec<proc_macro2::TokenStream> = accs
        .fields
        .iter()
        .map(|f: &AccountField| {
            let name = &f.ident();
            quote! { account_infos.extend(self.#name.to_account_infos()); }
        })
        .collect();
    quote! {
        #[automatically_derived]
        impl<#combined_generics> anchor_lang::ToAccountInfos<#trait_generics> for #name <#struct_generics> #where_clause{
            fn to_account_infos(&self) -> Vec<anchor_lang::solana_program::account_info::AccountInfo<#trait_generics>> {
                let mut account_infos = vec![];

                #(#to_acc_infos)*

                account_infos
            }
        }
    }
}
