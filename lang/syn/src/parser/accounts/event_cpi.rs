use quote::quote;

/// This struct is used to keep the authority account information in sync.
pub struct EventAuthority {
    /// Account name of the event authority
    pub name: &'static str,
    /// Seeds expression of the event authority
    pub seeds: proc_macro2::TokenStream,
}

impl EventAuthority {
    /// Returns the account name and the seeds expression of the event authority.
    pub fn get() -> Self {
        Self {
            name: "event_authority",
            seeds: quote! {b"__event_authority"},
        }
    }

    /// Returns the name without surrounding quotes.
    pub fn name_token_stream(&self) -> proc_macro2::TokenStream {
        let name_ident = syn::Ident::new(self.name, proc_macro2::Span::call_site());
        quote! {#name_ident}
    }
}

/// Add necessary event CPI accounts to the given accounts struct.
pub fn add_event_cpi_accounts(
    accounts_struct: &syn::ItemStruct,
) -> syn::parse::Result<syn::ItemStruct> {
    let syn::ItemStruct {
        attrs,
        vis,
        struct_token,
        ident,
        generics,
        fields,
        ..
    } = accounts_struct;

    let fields = fields.into_iter().collect::<Vec<_>>();

    let info_lifetime = generics
        .lifetimes()
        .next()
        .map(|lifetime| quote! {#lifetime})
        .unwrap_or(quote! {'info});
    let generics = generics
        .lt_token
        .map(|_| quote! {#generics})
        .unwrap_or(quote! {<'info>});

    let authority = EventAuthority::get();
    let authority_name = authority.name_token_stream();

    let accounts_struct = quote! {
        #(#attrs)*
        #vis #struct_token #ident #generics {
            #(#fields,)*

            /// CHECK: Only the event authority can invoke self-CPI
            #[account(address = crate::EVENT_AUTHORITY_AND_BUMP.0)]
            pub #authority_name: UncheckedAccount<#info_lifetime>,
            /// CHECK: Self-CPI will fail if the program is not the current program
            pub program: UncheckedAccount<#info_lifetime>,
        }
    };
    syn::parse2(accounts_struct)
}
