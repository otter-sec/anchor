use {
    anchor_lang_idl::types::{
        Idl, IdlArrayLen, IdlDefinedFields, IdlField, IdlGenericArg, IdlInstructionAccountItem,
        IdlInstructionAccounts, IdlRepr, IdlSerialization, IdlType, IdlTypeDef, IdlTypeDefGeneric,
        IdlTypeDefTy,
    },
    proc_macro2::Literal,
    quote::{format_ident, quote},
};

/// This function should ideally return the absolute path to the declared program's id but because
/// `proc_macro2::Span::call_site().source_file().path()` is behind an unstable feature flag, we
/// are not able to reliably decide where the definition is.
pub fn get_canonical_program_id() -> proc_macro2::TokenStream {
    quote! { super::__ID }
}

pub fn gen_docs(docs: &[String]) -> proc_macro2::TokenStream {
    let docs = docs
        .iter()
        .map(|doc| format!("{}{doc}", if doc.is_empty() { "" } else { " " }))
        .map(|doc| quote! { #[doc = #doc] });
    quote! { #(#docs)* }
}

pub fn gen_discriminator(disc: &[u8]) -> proc_macro2::TokenStream {
    quote! { [#(#disc), *] }
}

pub fn gen_accounts_common(idl: &Idl, prefix: &str) -> proc_macro2::TokenStream {
    let re_exports = idl
        .instructions
        .iter()
        .map(|ix| format_ident!("__{}_accounts_{}", prefix, ix.name))
        .map(|ident| quote! { pub use super::internal::#ident::*; });

    quote! {
        pub mod accounts {
            #(#re_exports)*
        }
    }
}

pub fn convert_idl_type_to_syn_type(ty: &IdlType) -> syn::Type {
    syn::parse_str(&convert_idl_type_to_str(ty, false)).unwrap()
}

// TODO: Impl `ToString` for `IdlType`
pub fn convert_idl_type_to_str(ty: &IdlType, is_const: bool) -> String {
    match ty {
        IdlType::Bool => "bool".into(),
        IdlType::U8 => "u8".into(),
        IdlType::I8 => "i8".into(),
        IdlType::U16 => "u16".into(),
        IdlType::I16 => "i16".into(),
        IdlType::U32 => "u32".into(),
        IdlType::I32 => "i32".into(),
        IdlType::F32 => "f32".into(),
        IdlType::U64 => "u64".into(),
        IdlType::I64 => "i64".into(),
        IdlType::F64 => "f64".into(),
        IdlType::U128 => "u128".into(),
        IdlType::I128 => "i128".into(),
        IdlType::U256 => "u256".into(),
        IdlType::I256 => "i256".into(),
        IdlType::Bytes => if is_const { "&[u8]" } else { "Vec<u8>" }.into(),
        IdlType::String => if is_const { "&str" } else { "String" }.into(),
        IdlType::Pubkey => "Pubkey".into(),
        IdlType::Option(ty) => format!("Option<{}>", convert_idl_type_to_str(ty, is_const)),
        IdlType::Vec(ty) => format!("Vec<{}>", convert_idl_type_to_str(ty, is_const)),
        IdlType::Array(ty, len) => format!(
            "[{}; {}]",
            convert_idl_type_to_str(ty, is_const),
            match len {
                IdlArrayLen::Generic(len) => len.into(),
                IdlArrayLen::Value(len) => len.to_string(),
            }
        ),
        IdlType::Defined { name, generics } => generics
            .iter()
            .map(|generic| match generic {
                IdlGenericArg::Type { ty } => convert_idl_type_to_str(ty, is_const),
                IdlGenericArg::Const { value } => value.into(),
            })
            .reduce(|mut acc, cur| {
                if !acc.is_empty() {
                    acc.push(',');
                }
                acc.push_str(&cur);
                acc
            })
            .map(|generics| format!("{name}<{generics}>"))
            .unwrap_or(name.into()),
        IdlType::Generic(ty) => ty.into(),
        _ => unimplemented!("{ty:?}"),
    }
}

pub fn convert_idl_type_def_to_ts(
    ty_def: &IdlTypeDef,
    ty_defs: &[IdlTypeDef],
) -> proc_macro2::TokenStream {
    let name = format_ident!("{}", ty_def.name);
    let docs = gen_docs(&ty_def.docs);

    let generics = {
        let generics = ty_def
            .generics
            .iter()
            .map(|generic| match generic {
                IdlTypeDefGeneric::Type { name } => {
                    let name = format_ident!("{}", name);
                    quote! { #name }
                }
                IdlTypeDefGeneric::Const { name, ty } => {
                    let name = format_ident!("{}", name);
                    let ty = format_ident!("{}", ty);
                    quote! { const #name: #ty }
                }
            })
            .collect::<Vec<_>>();
        if generics.is_empty() {
            quote!()
        } else {
            quote!(<#(#generics,)*>)
        }
    };

    let attrs = {
        let debug_attr = quote!(#[derive(Debug)]);

        let default_attr =
            can_derive_default(ty_def, ty_defs).then_some(quote!(#[derive(Default)]));

        let ser_attr = match &ty_def.serialization {
            IdlSerialization::Borsh => quote!(#[derive(AnchorSerialize, AnchorDeserialize)]),
            IdlSerialization::Bytemuck => quote!(#[zero_copy]),
            IdlSerialization::BytemuckUnsafe => quote!(#[zero_copy(unsafe)]),
            _ => unimplemented!("{:?}", ty_def.serialization),
        };

        let clone_attr = matches!(ty_def.serialization, IdlSerialization::Borsh)
            .then(|| quote!(#[derive(Clone)]))
            .unwrap_or_default();

        let copy_attr = matches!(ty_def.serialization, IdlSerialization::Borsh)
            .then(|| can_derive_copy(ty_def, ty_defs).then(|| quote!(#[derive(Copy)])))
            .flatten()
            .unwrap_or_default();

        // `ser_attr` must be expanded first, as it may produce `repr(packed)`
        // This affects builtin derives so must be visible to them
        // https://github.com/solana-foundation/anchor/issues/4072
        quote! {
            #ser_attr
            #debug_attr
            #default_attr
            #clone_attr
            #copy_attr
        }
    };

    let repr = ty_def.repr.as_ref().map(|repr| {
        let kind = match repr {
            IdlRepr::Rust(_) => "Rust",
            IdlRepr::C(_) => "C",
            IdlRepr::Transparent => "transparent",
            _ => unimplemented!("{repr:?}"),
        };
        let kind = format_ident!("{kind}");

        let modifier = match repr {
            IdlRepr::Rust(modifier) | IdlRepr::C(modifier) => {
                let packed = modifier.packed.then_some(quote!(packed));
                let align = modifier
                    .align
                    .map(Literal::usize_unsuffixed)
                    .map(|align| quote!(align(#align)));

                match (packed, align) {
                    (None, None) => None,
                    (Some(p), None) => Some(quote!(#p)),
                    (None, Some(a)) => Some(quote!(#a)),
                    (Some(p), Some(a)) => Some(quote!(#p, #a)),
                }
            }
            _ => None,
        }
        .map(|m| quote!(, #m));
        quote! { #[repr(#kind #modifier)] }
    });

    match &ty_def.ty {
        IdlTypeDefTy::Struct { fields } => {
            let declare_struct = quote! { pub struct #name #generics };
            let ty = handle_defined_fields(
                fields.as_ref(),
                || quote! { #declare_struct; },
                |fields| {
                    let fields = fields.iter().map(|field| {
                        let name = format_ident!("{}", field.name);
                        let ty = convert_idl_type_to_syn_type(&field.ty);
                        quote! { pub #name : #ty }
                    });
                    quote! {
                        #declare_struct {
                            #(#fields,)*
                        }
                    }
                },
                |tys| {
                    let tys = tys
                        .iter()
                        .map(convert_idl_type_to_syn_type)
                        .map(|ty| quote! { pub #ty });

                    quote! {
                        #declare_struct (#(#tys,)*);
                    }
                },
            );

            quote! {
                #docs
                #attrs
                #repr
                #ty
            }
        }
        IdlTypeDefTy::Enum { variants, .. } => {
            let variants = variants.iter().map(|variant| {
                let variant_name = format_ident!("{}", variant.name);
                handle_defined_fields(
                    variant.fields.as_ref(),
                    || quote! { #variant_name },
                    |fields| {
                        let fields = fields.iter().map(|field| {
                            let name = format_ident!("{}", field.name);
                            let ty = convert_idl_type_to_syn_type(&field.ty);
                            quote! { #name : #ty }
                        });
                        quote! {
                            #variant_name {
                                #(#fields,)*
                            }
                        }
                    },
                    |tys| {
                        let tys = tys.iter().map(convert_idl_type_to_syn_type);
                        quote! {
                            #variant_name (#(#tys,)*)
                        }
                    },
                )
            });

            quote! {
                #docs
                #attrs
                #repr
                pub enum #name #generics {
                    #(#variants,)*
                }
            }
        }
        IdlTypeDefTy::Type { alias } => {
            let alias = convert_idl_type_to_syn_type(alias);
            quote! {
                #docs
                pub type #name = #alias;
            }
        }
    }
}

fn can_derive_copy(ty_def: &IdlTypeDef, ty_defs: &[IdlTypeDef]) -> bool {
    match &ty_def.ty {
        IdlTypeDefTy::Struct { fields } => {
            can_derive_common(fields.as_ref(), ty_defs, can_derive_copy_ty)
        }
        IdlTypeDefTy::Enum { variants, .. } => variants
            .iter()
            .all(|variant| can_derive_common(variant.fields.as_ref(), ty_defs, can_derive_copy_ty)),
        IdlTypeDefTy::Type { alias } => can_derive_copy_ty(alias, ty_defs),
    }
}

fn can_derive_default(ty_def: &IdlTypeDef, ty_defs: &[IdlTypeDef]) -> bool {
    match &ty_def.ty {
        IdlTypeDefTy::Struct { fields } => {
            can_derive_common(fields.as_ref(), ty_defs, can_derive_default_ty)
        }
        // TODO: Consider storing the default enum variant in IDL
        IdlTypeDefTy::Enum { .. } => false,
        IdlTypeDefTy::Type { alias } => can_derive_default_ty(alias, ty_defs),
    }
}

fn can_derive_copy_ty(ty: &IdlType, ty_defs: &[IdlTypeDef]) -> bool {
    match ty {
        IdlType::Option(inner) => can_derive_copy_ty(inner, ty_defs),
        IdlType::Array(inner, len) => {
            if !can_derive_copy_ty(inner, ty_defs) {
                return false;
            }

            match len {
                IdlArrayLen::Value(_) => true,
                IdlArrayLen::Generic(_) => false,
            }
        }
        IdlType::Defined { name, .. } => ty_defs
            .iter()
            .find(|ty_def| &ty_def.name == name)
            .map(|ty_def| can_derive_copy(ty_def, ty_defs))
            .expect("Type def must exist"),
        IdlType::Bytes | IdlType::String | IdlType::Vec(_) | IdlType::Generic(_) => false,
        _ => true,
    }
}

fn can_derive_default_ty(ty: &IdlType, ty_defs: &[IdlTypeDef]) -> bool {
    match ty {
        IdlType::Option(inner) => can_derive_default_ty(inner, ty_defs),
        IdlType::Vec(inner) => can_derive_default_ty(inner, ty_defs),
        IdlType::Array(inner, len) => {
            if !can_derive_default_ty(inner, ty_defs) {
                return false;
            }

            match len {
                IdlArrayLen::Value(len) => *len <= 32,
                IdlArrayLen::Generic(_) => false,
            }
        }
        IdlType::Defined { name, .. } => ty_defs
            .iter()
            .find(|ty_def| &ty_def.name == name)
            .map(|ty_def| can_derive_default(ty_def, ty_defs))
            .expect("Type def must exist"),
        IdlType::Generic(_) => false,
        _ => true,
    }
}

fn can_derive_common(
    fields: Option<&IdlDefinedFields>,
    ty_defs: &[IdlTypeDef],
    can_derive_ty: fn(&IdlType, &[IdlTypeDef]) -> bool,
) -> bool {
    handle_defined_fields(
        fields,
        || true,
        |fields| {
            fields
                .iter()
                .map(|field| &field.ty)
                .all(|ty| can_derive_ty(ty, ty_defs))
        },
        |tys| tys.iter().all(|ty| can_derive_ty(ty, ty_defs)),
    )
}

fn handle_defined_fields<R>(
    fields: Option<&IdlDefinedFields>,
    unit_cb: impl Fn() -> R,
    named_cb: impl Fn(&[IdlField]) -> R,
    tuple_cb: impl Fn(&[IdlType]) -> R,
) -> R {
    match fields {
        Some(fields) => match fields {
            IdlDefinedFields::Named(fields) => named_cb(fields),
            IdlDefinedFields::Tuple(tys) => tuple_cb(tys),
        },
        _ => unit_cb(),
    }
}

/// Combine regular instruction accounts with non-instruction composite accounts.
pub fn get_all_instruction_accounts(idl: &Idl) -> Vec<IdlInstructionAccounts> {
    // It's possible to declare an accounts struct and not use it as an instruction, see
    // https://github.com/solana-foundation/anchor/issues/3274
    //
    // NOTE: Returned accounts will not be unique if non-instruction composite accounts have been
    // used multiple times https://github.com/solana-foundation/anchor/issues/3349
    fn get_non_instruction_composite_accounts<'a>(
        accs: &'a [IdlInstructionAccountItem],
        idl: &'a Idl,
    ) -> Vec<&'a IdlInstructionAccounts> {
        accs.iter()
            .flat_map(|acc| match acc {
                IdlInstructionAccountItem::Composite(accs)
                    if !idl
                        .instructions
                        .iter()
                        .any(|ix| ix.accounts == accs.accounts) =>
                {
                    let mut nica = get_non_instruction_composite_accounts(&accs.accounts, idl);
                    nica.push(accs);
                    nica
                }
                _ => Default::default(),
            })
            .collect()
    }

    let ix_accs = idl
        .instructions
        .iter()
        .flat_map(|ix| ix.accounts.to_owned())
        .collect::<Vec<_>>();
    get_non_instruction_composite_accounts(&ix_accs, idl)
        .into_iter()
        .fold(Vec::<IdlInstructionAccounts>::default(), |mut all, accs| {
            // Make sure they are unique
            if all.iter().all(|a| a.accounts != accs.accounts) {
                // The name is not guaranteed to be the same as the one used in the actual source
                // code of the program because the IDL only stores the field names
                let name = if all.iter().all(|a| a.name != accs.name) {
                    accs.name.to_owned()
                } else {
                    // Append numbers to the field name until we find a unique name
                    (2..)
                        .find_map(|i| {
                            let name = format!("{}{i}", accs.name);
                            all.iter().all(|a| a.name != name).then_some(name)
                        })
                        .expect("Should always find a valid name")
                };

                all.push(IdlInstructionAccounts {
                    name,
                    accounts: accs.accounts.to_owned(),
                })
            }

            all
        })
        .into_iter()
        .chain(idl.instructions.iter().map(|ix| IdlInstructionAccounts {
            name: ix.name.to_owned(),
            accounts: ix.accounts.to_owned(),
        }))
        .collect()
}
