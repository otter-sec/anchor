use {
    proc_macro2::{Literal, TokenStream},
    quote::{format_ident, quote, ToTokens},
};

pub fn gen_lazy(strct: &syn::ItemStruct) -> syn::Result<TokenStream> {
    let ident = &strct.ident;
    let lazy_ident = format_ident!("Lazy{}", ident);
    let load_common_ident = to_private_ident("load_common");
    let initialize_fields = to_private_ident("initialize_fields");
    let lazy_acc_ty = quote! { anchor_lang::accounts::lazy_account::LazyAccount };
    let disc_len = quote! { <#ident as anchor_lang::Discriminator>::DISCRIMINATOR.len() };

    let load_common_docs = quote! {
        /// The deserialized value is cached for future uses i.e. all subsequent calls to this
        /// method do not deserialize the data again, instead, they return the cached value.
        ///
        /// To reload the data from the underlying account info (e.g. after a CPI call), run
        /// [`LazyAccount::unload`] before running this method.
        ///
        /// See [`LazyAccount`]'s documentation for more information.
    };
    let load_panic_docs = quote! {
        /// # Panics
        ///
        /// If there is an existing mutable reference created by any of the `load_mut` methods.
    };
    let load_mut_panic_docs = quote! {
        /// # Panics
        ///
        /// If there is an existing reference (mutable or not) created by any of the `load` methods.
    };

    let (loader_signatures, loader_impls) = strct
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| {
            let field_ident = to_field_ident(field, i);
            let load_ident = format_ident!("load_{field_ident}");
            let load_mut_ident = format_ident!("load_mut_{field_ident}");
            let load_common_ident = to_private_ident(format!("load_common_{field_ident}"));
            let offset_of_ident = to_private_ident(format!("offset_of_{field_ident}"));
            let size_of_ident = to_private_ident(format!("size_of_{field_ident}"));

            #[allow(
                clippy::unwrap_used,
                reason = "field i-1 always exists when iterating field i"
            )]
            let offset = i.eq(&0).then(|| quote!(#disc_len)).unwrap_or_else(|| {
                // Current offset is the previous field's offset + size
                strct
                    .fields
                    .iter()
                    .nth(i - 1)
                    .map(|field| {
                        let field_ident = to_field_ident(field, i - 1);
                        let offset_of_ident = to_private_ident(format!("offset_of_{field_ident}"));
                        let size_of_ident = to_private_ident(format!("size_of_{field_ident}"));
                        quote! { self.#offset_of_ident() + self.#size_of_ident() }
                    })
                    .expect("Previous field should always exist when i > 0")
            });

            let ty = &field.ty;
            let ty_as_lazy = quote! { <#ty as anchor_lang::__private::Lazy> };
            let size = quote! {
                // Calculating the offset is highly wasteful if the type is sized.
                if #ty_as_lazy::SIZED {
                    #ty_as_lazy::size_of(&[])
                } else {
                    #ty_as_lazy::size_of(&self.__info.data.borrow()[self.#offset_of_ident()..])
                }
            };

            let signatures = quote! {
                /// Load a reference to the field.
                ///
                #load_common_docs
                ///
                #load_panic_docs
                fn #load_ident(&self) -> anchor_lang::Result<::core::cell::Ref<'_, #ty>>;

                /// Load a mutable reference to the field.
                ///
                #load_common_docs
                ///
                #load_mut_panic_docs
                fn #load_mut_ident(&self) -> anchor_lang::Result<::core::cell::RefMut<'_, #ty>>;

                 #[doc(hidden)]
                fn #load_common_ident<R>(&self, f: impl FnOnce() -> R) -> anchor_lang::Result<R>;

                 #[doc(hidden)]
                fn #offset_of_ident(&self) -> usize;

                 #[doc(hidden)]
                fn #size_of_ident(&self) -> usize;
            };

            let impls = quote! {
                fn #load_ident(&self) -> anchor_lang::Result<::core::cell::Ref<'_, #ty>> {
                    self.#load_common_ident(|| {
                        // SAFETY: The common load method makes sure the field is initialized.
                        ::core::cell::Ref::map(self.__account.borrow(), |acc| unsafe {
                            &*::core::ptr::addr_of!((*acc.as_ptr()).#field_ident)
                        })
                    })
                }

                fn #load_mut_ident(&self) -> anchor_lang::Result<::core::cell::RefMut<'_, #ty>> {
                    self.#load_common_ident(|| {
                        // SAFETY: The common load method makes sure the field is initialized.
                        ::core::cell::RefMut::map(self.__account.borrow_mut(), |acc| unsafe {
                            &mut *::core::ptr::addr_of_mut!((*acc.as_mut_ptr()).#field_ident)
                        })
                    })
                }

                #[inline(never)]
                fn #load_common_ident<R>(&self, f: impl FnOnce() -> R) -> anchor_lang::Result<R> {
                    self.#initialize_fields();

                    // Return early if initialized
                    if self.__fields.borrow().as_ref().unwrap()[#i] {
                        return Ok(f());
                    }

                    // Deserialize and write
                    let offset = self.#offset_of_ident();
                    let size = self.#size_of_ident();
                    let data = self.__info.data.borrow();
                    let val = anchor_lang::AnchorDeserialize::try_from_slice(
                        &data[offset..offset + size]
                    )?;
                    unsafe {
                        ::core::ptr::addr_of_mut!(
                            (*self.__account.borrow_mut().as_mut_ptr()).#field_ident
                        ).write(val)
                     };

                    // Set initialized
                    self.__fields.borrow_mut().as_mut().unwrap()[#i] = true;

                    Ok(f())
                }

                // If this method gets inlined when there are >= 12 fields, compilation breaks with
                // `LLVM ERROR: Branch target out of insn range`
                #[inline(never)]
                fn #offset_of_ident(&self) -> usize {
                    #offset
                }

                #[inline(always)]
                fn #size_of_ident(&self) -> usize {
                    #size
                }
            };

            Ok((signatures, impls))
        })
        .collect::<syn::Result<Vec<_>>>()?
        .into_iter()
        .unzip::<_, _, Vec<_>, Vec<_>>();

    let load_idents = strct
        .fields
        .iter()
        .enumerate()
        .map(|(i, field)| to_field_ident(field, i))
        .map(|field| format_ident!("load_{field}"));
    let total_fields = strct.fields.len();

    Ok(quote! {
        pub trait #lazy_ident {
            /// Load a reference to the entire account.
            ///
            #load_common_docs
            ///
            #load_panic_docs
            fn load(&self) -> anchor_lang::Result<::core::cell::Ref<'_, #ident>>;

            /// Load a mutable reference to the entire account.
            ///
            #load_common_docs
            ///
            #load_mut_panic_docs
            fn load_mut(&self) -> anchor_lang::Result<::core::cell::RefMut<'_, #ident>>;

            #[doc(hidden)]
            fn #load_common_ident<R>(&self, f: impl FnOnce() -> R) -> anchor_lang::Result<R>;

            #(#loader_signatures)*

            #[doc(hidden)]
            fn #initialize_fields(&self);

            /// Run the exit routine of the account, similar to [`AccountsExit`] but implemented
            /// as a regular method because we can't implement external traits for external structs.
            fn exit(&self, program_id: &anchor_lang::prelude::Pubkey) -> anchor_lang::Result<()>;
        }

        impl<'info> #lazy_ident for #lazy_acc_ty<'info, #ident> {
            fn load(&self) -> anchor_lang::Result<::core::cell::Ref<'_, #ident>> {
                self.#load_common_ident(|| {
                    // SAFETY: The common load method makes sure all fields are initialized.
                    ::core::cell::Ref::map(self.__account.borrow(), |acc| unsafe {
                        acc.assume_init_ref()
                    })
                })
            }

            fn load_mut(&self) -> anchor_lang::Result<::core::cell::RefMut<'_, #ident>> {
                self.#load_common_ident(|| {
                    // SAFETY: The common load method makes sure all fields are initialized.
                    ::core::cell::RefMut::map(self.__account.borrow_mut(), |acc| unsafe {
                        acc.assume_init_mut()
                    })
                })
            }

            #[inline(never)]
            fn #load_common_ident<R>(&self, f: impl FnOnce() -> R) -> anchor_lang::Result<R> {
                self.#initialize_fields();

                // Create a scope to drop the `__fields` borrow
                let all_uninit = {
                    // Return early if all fields are initialized
                    let fields = self.__fields.borrow();
                    let fields = fields.as_ref().unwrap();
                    if !fields.contains(&false) {
                        return Ok(f());
                    }

                    !fields.contains(&true)
                };

                if all_uninit {
                    // Nothing is initialized, initialize all
                    let offset = #disc_len;
                    let mut data = self.__info.data.borrow();
                    let val = anchor_lang::AnchorDeserialize::deserialize(&mut &data[offset..])?;
                    unsafe { self.__account.borrow_mut().as_mut_ptr().write(val) };

                    // Set fields to initialized
                    let mut fields = self.__fields.borrow_mut();
                    let fields = fields.as_mut().unwrap();
                    for field in fields {
                        *field = true;
                    }
                } else {
                    // Only initialize uninitialized fields (`load` methods already do this).
                    //
                    // This is not exactly efficient because `load` methods have a bit of
                    // runtime ownership overhead. This could be optimized further, but it
                    // requires some refactoring and also makes the code harder to reason about.
                    //
                    // We can return back to this if benchmarks show this is a bottleneck.
                    #(self.#load_idents()?;)*
                }

                Ok(f())
            }

            #(#loader_impls)*

            #[inline(always)]
            fn #initialize_fields(&self) {
                if self.__fields.borrow().is_none() {
                    *self.__fields.borrow_mut() = Some(vec![false; #total_fields]);
                }
            }

            // TODO: This method can be optimized to *only* serialize the fields that we have
            // initialized rather than deserializing the whole account, and then serializing it
            // back, which consumes a lot more CUs than it should for most accounts.
            fn exit(&self, program_id: &anchor_lang::prelude::Pubkey) -> anchor_lang::Result<()> {
                // Only persist if the owner is the current program and the account is not closed
                if &<#ident as anchor_lang::Owner>::owner() == program_id
                    && !anchor_lang::__private::is_closed(self.__info)
                {
                    // Make sure all fields are initialized
                    let acc = self.load()?;
                    let mut data = self.__info.try_borrow_mut_data()?;
                    let dst: &mut [u8] = &mut data;
                    let mut writer = anchor_lang::__private::BpfWriter::new(dst);
                    acc.try_serialize(&mut writer)?;
                }

                Ok(())
            }
        }
    })
}

/// Get the field's ident and if the ident doesn't exist (e.g. for tuple structs), default to the
/// given index.
fn to_field_ident(field: &syn::Field, i: usize) -> TokenStream {
    field
        .ident
        .as_ref()
        .map(ToTokens::to_token_stream)
        .unwrap_or_else(|| Literal::usize_unsuffixed(i).to_token_stream())
}

/// Convert to private ident.
///
/// This is used to indicate to the users that they shouldn't use this identifier.
fn to_private_ident<S: AsRef<str>>(ident: S) -> syn::Ident {
    format_ident!("__{}", ident.as_ref())
}
