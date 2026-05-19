use {
    super::common::{get_idl_module_path, get_no_docs},
    crate::parser::docs,
    proc_macro2::TokenStream,
    quote::quote,
    syn::{spanned::Spanned, Result},
};

/// Generate `IdlBuild` impl for a struct.
pub fn impl_idl_build_struct(item: &syn::ItemStruct) -> TokenStream {
    impl_idl_build(&item.ident, &item.generics, gen_idl_type_def_struct(item))
}

/// Generate `IdlBuild` impl for an enum.
pub fn impl_idl_build_enum(item: &syn::ItemEnum) -> TokenStream {
    impl_idl_build(&item.ident, &item.generics, gen_idl_type_def_enum(item))
}

/// Generate `IdlBuild` impl for a union.
///
/// Unions are not currently supported in the IDL.
pub fn impl_idl_build_union(item: &syn::ItemUnion) -> TokenStream {
    impl_idl_build(
        &item.ident,
        &item.generics,
        Err(syn::Error::new_spanned(item, "Unions are not supported")),
    )
}

/// Generate `IdlBuild` implementation.
fn impl_idl_build(
    ident: &syn::Ident,
    generics: &syn::Generics,
    type_def: Result<(TokenStream, Vec<syn::TypePath>)>,
) -> TokenStream {
    let idl = get_idl_module_path();
    let idl_build_trait = quote!(anchor_lang::idl::build::IdlBuild);
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let (idl_type_def, insert_defined) = match type_def {
        Err(e) => return e.into_compile_error(),
        Ok((ts, defined)) => (
            quote! { Some(#ts) },
            quote! {
                #(
                    if let Some(ty) = <#defined>::create_type() {
                        types.insert(<#defined>::get_full_path(), ty);
                        <#defined>::insert_types(types);
                    }
                );*
            },
        ),
    };

    quote! {
        impl #impl_generics #idl_build_trait for #ident #ty_generics #where_clause {
            fn create_type() -> Option<#idl::IdlTypeDef> {
                #idl_type_def
            }

            fn insert_types(
                types: &mut std::collections::BTreeMap<String, #idl::IdlTypeDef>
            ) {
                #insert_defined
            }

            fn get_full_path() -> String {
                format!("{}::{}", module_path!(), stringify!(#ident))
            }
        }
    }
}

pub fn gen_idl_type_def_struct(
    strct: &syn::ItemStruct,
) -> Result<(TokenStream, Vec<syn::TypePath>)> {
    gen_idl_type_def(&strct.attrs, &strct.generics, |generic_params| {
        let no_docs = get_no_docs();
        let idl = get_idl_module_path();

        let (fields, defined) = match &strct.fields {
            syn::Fields::Unit => (quote! { None }, vec![]),
            syn::Fields::Named(fields) => {
                let (fields, defined) = fields
                    .named
                    .iter()
                    .map(|f| gen_idl_field(f, generic_params, no_docs))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .unzip::<_, _, Vec<_>, Vec<_>>();

                (
                    quote! { Some(#idl::IdlDefinedFields::Named(vec![#(#fields),*])) },
                    defined,
                )
            }
            syn::Fields::Unnamed(fields) => {
                let (types, defined) = fields
                    .unnamed
                    .iter()
                    .map(|f| gen_idl_type(&f.ty, generic_params))
                    .collect::<Result<Vec<_>>>()?
                    .into_iter()
                    .unzip::<_, Vec<_>, Vec<_>, Vec<_>>();

                (
                    quote! { Some(#idl::IdlDefinedFields::Tuple(vec![#(#types),*])) },
                    defined,
                )
            }
        };
        let defined = defined.into_iter().flatten().collect::<Vec<_>>();

        Ok((
            quote! {
                #idl::IdlTypeDefTy::Struct {
                    fields: #fields,
                }
            },
            defined,
        ))
    })
}

fn gen_idl_type_def_enum(enm: &syn::ItemEnum) -> Result<(TokenStream, Vec<syn::TypePath>)> {
    gen_idl_type_def(&enm.attrs, &enm.generics, |generic_params| {
        let no_docs = get_no_docs();
        let idl = get_idl_module_path();

        let (variants, defined) = enm
            .variants
            .iter()
            .map(|variant| {
                let name = variant.ident.to_string();
                let (fields, defined) = match &variant.fields {
                    syn::Fields::Unit => (quote! { None }, vec![]),
                    syn::Fields::Named(fields) => {
                        let (fields, defined) = fields
                            .named
                            .iter()
                            .map(|f| gen_idl_field(f, generic_params, no_docs))
                            .collect::<Result<Vec<_>>>()?
                            .into_iter()
                            .unzip::<_, Vec<_>, Vec<_>, Vec<_>>();
                        let defined = defined.into_iter().flatten().collect::<Vec<_>>();

                        (
                            quote! { Some(#idl::IdlDefinedFields::Named(vec![#(#fields),*])) },
                            defined,
                        )
                    }
                    syn::Fields::Unnamed(fields) => {
                        let (types, defined) = fields
                            .unnamed
                            .iter()
                            .map(|f| gen_idl_type(&f.ty, generic_params))
                            .collect::<Result<Vec<_>>>()?
                            .into_iter()
                            .unzip::<_, Vec<_>, Vec<_>, Vec<_>>();
                        let defined = defined.into_iter().flatten().collect::<Vec<_>>();

                        (
                            quote! { Some(#idl::IdlDefinedFields::Tuple(vec![#(#types),*])) },
                            defined,
                        )
                    }
                };

                Ok((
                    quote! { #idl::IdlEnumVariant { name: #name.into(), fields: #fields } },
                    defined,
                ))
            })
            .collect::<Result<Vec<_>>>()?
            .into_iter()
            .unzip::<_, _, Vec<_>, Vec<_>>();
        let defined = defined.into_iter().flatten().collect::<Vec<_>>();

        Ok((
            quote! {
                #idl::IdlTypeDefTy::Enum {
                    variants: vec![#(#variants),*],
                }
            },
            defined,
        ))
    })
}

fn gen_idl_type_def<F>(
    attrs: &[syn::Attribute],
    generics: &syn::Generics,
    create_fields: F,
) -> Result<(TokenStream, Vec<syn::TypePath>)>
where
    F: Fn(&[syn::Ident]) -> Result<(TokenStream, Vec<syn::TypePath>)>,
{
    let no_docs = get_no_docs();
    let idl = get_idl_module_path();

    let docs = match docs::parse(attrs) {
        Some(docs) if !no_docs => quote! { vec![#(#docs.into()),*] },
        _ => quote! { vec![] },
    };

    let serialization = get_attr_str("derive", attrs)
        .and_then(|derive| {
            if derive.contains("bytemuck") {
                if derive.to_lowercase().contains("unsafe") {
                    Some(quote! { #idl::IdlSerialization::BytemuckUnsafe })
                } else {
                    Some(quote! { #idl::IdlSerialization::Bytemuck })
                }
            } else {
                None
            }
        })
        .unwrap_or_else(|| quote! { #idl::IdlSerialization::default() });

    let repr = get_attr_str("repr", attrs)
        .map(|repr| {
            let packed = repr.contains("packed");
            let align = repr
                .find("align")
                .and_then(|i| repr.get(i..))
                .and_then(|align| {
                    align
                        .find('(')
                        .and_then(|start| align.find(')').and_then(|end| align.get(start + 1..end)))
                })
                .and_then(|size| size.parse::<usize>().ok())
                .map(|size| quote! { Some(#size) })
                .unwrap_or_else(|| quote! { None });
            let modifier = quote! {
                #idl::IdlReprModifier {
                    packed: #packed,
                    align: #align,
                }
            };

            if repr.contains("transparent") {
                quote! { #idl::IdlRepr::Transparent }
            } else if repr.contains('C') {
                quote! { #idl::IdlRepr::C(#modifier) }
            } else {
                quote! { #idl::IdlRepr::Rust(#modifier) }
            }
        })
        .map(|repr| quote! { Some(#repr) })
        .unwrap_or_else(|| quote! { None });

    let generic_params = generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(ty) => Some(ty.ident.clone()),
            syn::GenericParam::Const(c) => Some(c.ident.clone()),
            _ => None,
        })
        .collect::<Vec<_>>();
    let (ty, defined) = create_fields(&generic_params)?;

    let generics = generics
        .params
        .iter()
        .filter_map(|p| match p {
            syn::GenericParam::Type(ty) => {
                let name = ty.ident.to_string();
                Some(quote! {
                    #idl::IdlTypeDefGeneric::Type {
                        name: #name.into(),
                    }
                })
            }
            syn::GenericParam::Const(c) => {
                let name = c.ident.to_string();
                let ty = match &c.ty {
                    syn::Type::Path(path) => get_first_segment(path).ident.to_string(),
                    _ => unreachable!("Const generic type can only be path"),
                };
                Some(quote! {
                    #idl::IdlTypeDefGeneric::Const {
                        name: #name.into(),
                        ty: #ty.into(),
                    }
                })
            }
            _ => None,
        })
        .collect::<Vec<_>>();

    Ok((
        quote! {
            #idl::IdlTypeDef {
                name: Self::get_full_path(),
                docs: #docs,
                serialization: #serialization,
                repr: #repr,
                generics: vec![#(#generics.into()),*],
                ty: #ty,
            }
        },
        defined,
    ))
}

fn get_attr_str(name: impl AsRef<str>, attrs: &[syn::Attribute]) -> Option<String> {
    attrs
        .iter()
        .filter(|attr| {
            attr.path()
                .segments
                .first()
                .filter(|seg| seg.ident == name)
                .is_some()
        })
        .filter_map(|attr| match &attr.meta {
            syn::Meta::List(list) => {
                let (open, close) = match list.delimiter {
                    syn::MacroDelimiter::Paren(_) => ("(", ")"),
                    syn::MacroDelimiter::Bracket(_) => ("[", "]"),
                    syn::MacroDelimiter::Brace(_) => ("{", "}"),
                };
                Some(format!("{open}{}{close}", list.tokens))
            }
            _ => None,
        })
        .reduce(|acc, cur| {
            format!(
                "{} , {}",
                acc.get(..acc.len() - 1).unwrap(),
                cur.get(1..).unwrap()
            )
        })
}

fn gen_idl_field(
    field: &syn::Field,
    generic_params: &[syn::Ident],
    no_docs: bool,
) -> Result<(TokenStream, Vec<syn::TypePath>)> {
    let idl = get_idl_module_path();

    let name = field.ident.as_ref().unwrap().to_string();
    let docs = match docs::parse(&field.attrs) {
        Some(docs) if !no_docs => quote! { vec![#(#docs.into()),*] },
        _ => quote! { vec![] },
    };
    let (ty, defined) = gen_idl_type(&field.ty, generic_params)?;

    Ok((
        quote! {
            #idl::IdlField {
                name: #name.into(),
                docs: #docs,
                ty: #ty,
            }
        },
        defined,
    ))
}

pub fn gen_idl_type(
    ty: &syn::Type,
    generic_params: &[syn::Ident],
) -> Result<(TokenStream, Vec<syn::TypePath>)> {
    let idl = get_idl_module_path();

    fn the_only_segment_is(path: &syn::TypePath, cmp: &str) -> bool {
        if path.path.segments.len() != 1 {
            return false;
        };
        return get_first_segment(path).ident == cmp;
    }

    fn get_angle_bracketed_type_args(seg: &syn::PathSegment) -> Vec<&syn::Type> {
        match &seg.arguments {
            syn::PathArguments::AngleBracketed(ab) => ab
                .args
                .iter()
                .filter_map(|arg| match arg {
                    syn::GenericArgument::Type(ty) => Some(ty),
                    _ => None,
                })
                .collect(),
            _ => panic!("No angle bracket for {seg:#?}"),
        }
    }

    match ty {
        syn::Type::Path(path) if the_only_segment_is(path, "bool") => {
            Ok((quote! { #idl::IdlType::Bool }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "u8") => {
            Ok((quote! { #idl::IdlType::U8 }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "i8") => {
            Ok((quote! { #idl::IdlType::I8 }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "u16") => {
            Ok((quote! { #idl::IdlType::U16 }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "i16") => {
            Ok((quote! { #idl::IdlType::I16 }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "u32") => {
            Ok((quote! { #idl::IdlType::U32 }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "i32") => {
            Ok((quote! { #idl::IdlType::I32 }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "f32") => {
            Ok((quote! { #idl::IdlType::F32 }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "u64") => {
            Ok((quote! { #idl::IdlType::U64 }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "i64") => {
            Ok((quote! { #idl::IdlType::I64 }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "f64") => {
            Ok((quote! { #idl::IdlType::F64 }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "u128") => {
            Ok((quote! { #idl::IdlType::U128 }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "i128") => {
            Ok((quote! { #idl::IdlType::I128 }, vec![]))
        }
        syn::Type::Path(path)
            if the_only_segment_is(path, "String") || the_only_segment_is(path, "str") =>
        {
            Ok((quote! { #idl::IdlType::String }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "Pubkey") => {
            Ok((quote! { #idl::IdlType::Pubkey }, vec![]))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "Option") => {
            let segment = get_first_segment(path);
            let arg = get_angle_bracketed_type_args(segment)
                .into_iter()
                .next()
                .unwrap();
            let (inner, defined) = gen_idl_type(arg, generic_params)?;
            Ok((quote! { #idl::IdlType::Option(Box::new(#inner)) }, defined))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "Vec") => {
            let segment = get_first_segment(path);
            let arg = get_angle_bracketed_type_args(segment)
                .into_iter()
                .next()
                .unwrap();
            match arg {
                syn::Type::Path(path) if the_only_segment_is(path, "u8") => {
                    return Ok((quote! {#idl::IdlType::Bytes}, vec![]));
                }
                _ => (),
            };
            let (inner, defined) = gen_idl_type(arg, generic_params)?;
            Ok((quote! { #idl::IdlType::Vec(Box::new(#inner)) }, defined))
        }
        syn::Type::Path(path) if the_only_segment_is(path, "Box") => {
            let segment = get_first_segment(path);
            let arg = get_angle_bracketed_type_args(segment)
                .into_iter()
                .next()
                .unwrap();
            gen_idl_type(arg, generic_params)
        }
        syn::Type::Array(arr) => {
            let len = &arr.len;
            let is_generic = generic_params.iter().any(|param| match len {
                syn::Expr::Path(path) => path.path.is_ident(param),
                _ => false,
            });

            let len = if is_generic {
                match len {
                    syn::Expr::Path(len) => {
                        let len = len.path.get_ident().unwrap().to_string();
                        quote! { #idl::IdlArrayLen::Generic(#len.into()) }
                    }
                    _ => unreachable!("Array length can only be a generic parameter"),
                }
            } else {
                quote! { #idl::IdlArrayLen::Value(#len) }
            };

            let (inner, defined) = gen_idl_type(&arr.elem, generic_params)?;
            Ok((
                quote! { #idl::IdlType::Array(Box::new(#inner), #len) },
                defined,
            ))
        }
        // Defined
        syn::Type::Path(path) => {
            let is_generic_param = generic_params.iter().any(|param| path.path.is_ident(param));
            if is_generic_param {
                let generic = get_first_segment(path).ident.to_string();
                return Ok((quote! { #idl::IdlType::Generic(#generic.into()) }, vec![]));
            }

            // Handle type aliases and external types
            {
                use {
                    super::{common::find_path, external::get_external_type},
                    crate::parser::context::CrateContext,
                    quote::ToTokens,
                    std::{
                        collections::{HashMap, HashSet},
                        sync::OnceLock,
                    },
                };

                struct CachedCrateData {
                    /// Names of all structs and enums defined in the crate
                    defined_names: HashSet<String>,
                    /// Type aliases stored as (name, source_text) for re-parsing
                    type_aliases: HashMap<String, String>,
                }

                static CRATE_DATA_CACHE: OnceLock<std::result::Result<CachedCrateData, String>> =
                    OnceLock::new();

                // If no path was found, just return an empty path and let the find_path function handle it
                let source_path = proc_macro2::Span::call_site()
                    .local_file()
                    .unwrap_or_default();

                'cache: {
                    let Ok(lib_path) = find_path("lib.rs", &source_path) else {
                        break 'cache;
                    };
                    let name = path.path.segments.last().unwrap().ident.to_string();

                    let cache = CRATE_DATA_CACHE.get_or_init(|| {
                        CrateContext::parse(&lib_path)
                            .map_err(|e| e.to_string())
                            .map(|ctx| {
                                let defined_names: HashSet<String> = ctx
                                    .structs()
                                    .map(|s| s.ident.to_string())
                                    .chain(ctx.enums().map(|e| e.ident.to_string()))
                                    .collect();
                                let mut type_aliases: HashMap<String, String> = HashMap::new();
                                for ty in ctx.type_aliases() {
                                    type_aliases
                                        .entry(ty.ident.to_string())
                                        .or_insert_with(|| ty.to_token_stream().to_string());
                                }
                                CachedCrateData {
                                    defined_names,
                                    type_aliases,
                                }
                            })
                    });

                    let cache = match cache {
                        Ok(data) => data,
                        Err(e) => {
                            return Err(syn::Error::new(
                                path.span(),
                                format!("Failed to parse crate: {e}"),
                            ));
                        }
                    };

                    let alias_src = cache.type_aliases.get(&name).cloned();
                    let is_external = !cache.defined_names.contains(&name);

                    let alias: Option<syn::ItemType> =
                        alias_src.and_then(|src| syn::parse_str(&src).ok());

                    if let Some(alias) = alias {
                        if let Some(segment) = path.path.segments.last() {
                            if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                                let inners = args
                                    .args
                                    .iter()
                                    .map(|arg| match arg {
                                        syn::GenericArgument::Type(ty) => match ty {
                                            syn::Type::Path(inner_ty) => {
                                                inner_ty.path.to_token_stream().to_string()
                                            }
                                            _ => {
                                                unimplemented!("Inner type not implemented: {ty:?}")
                                            }
                                        },
                                        syn::GenericArgument::Const(c) => {
                                            c.to_token_stream().to_string()
                                        }
                                        _ => unimplemented!("Arg not implemented: {arg:?}"),
                                    })
                                    .collect::<Vec<_>>();

                                let outer = match &*alias.ty {
                                    syn::Type::Path(outer_ty) => outer_ty.path.to_token_stream(),
                                    syn::Type::Array(outer_ty) => outer_ty.to_token_stream(),
                                    _ => unimplemented!("Type not implemented: {:?}", alias.ty),
                                }
                                .to_string();

                                let resolved_alias = alias
                                    .generics
                                    .params
                                    .iter()
                                    .map(|param| match param {
                                        syn::GenericParam::Const(param) => param.ident.to_string(),
                                        syn::GenericParam::Type(param) => param.ident.to_string(),
                                        _ => panic!("Lifetime parameters are not allowed"),
                                    })
                                    .enumerate()
                                    .fold(outer, |acc, (i, cur)| {
                                        let inner = &inners[i];
                                        // The spacing of the `outer` variable can differ between
                                        // versions, e.g. `[T; N]` and `[T ; N]`
                                        acc.replace(&format!(" {cur} "), &format!(" {inner} "))
                                            .replace(&format!(" {cur},"), &format!(" {inner},"))
                                            .replace(&format!("[{cur} "), &format!("[{inner} "))
                                            .replace(&format!("[{cur};"), &format!("[{inner};"))
                                            .replace(&format!(" {cur}]"), &format!(" {inner}]"))
                                    });
                                if let Ok(ty) = syn::parse_str(&resolved_alias) {
                                    return gen_idl_type(&ty, generic_params);
                                }
                            }
                        };

                        // Non-generic type alias e.g. `type UnixTimestamp = i64`
                        return gen_idl_type(&*alias.ty, generic_params);
                    }

                    // Handle external types
                    if is_external {
                        if let Ok(Some(ty)) = get_external_type(&name, source_path) {
                            return gen_idl_type(&ty, generic_params);
                        }
                    }
                }
            }

            // Defined in crate
            let mut generics = vec![];
            let mut defined = vec![path.clone()];

            if let Some(segment) = path.path.segments.last() {
                if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                    for arg in &args.args {
                        let generic = match arg {
                            syn::GenericArgument::Const(c) => {
                                quote! { #idl::IdlGenericArg::Const { value: #c.to_string() } }
                            }
                            // `MY_CONST` in `Foo<MY_CONST>` is parsed as `GenericArgument::Type`
                            // instead of `GenericArgument::Const` because they're indistinguishable
                            // syntactically, as mentioned in
                            // https://github.com/dtolnay/syn/blob/bfa790b8e445dc67b7ab94d75adb1a92d6296c9a/src/path.rs#L113-L115
                            //
                            // As a workaround, we're manually checking to see if it *looks* like a
                            // constant identifier to fix the issue mentioned in
                            // https://github.com/otter-sec/anchor/issues/3520
                            syn::GenericArgument::Type(syn::Type::Path(p))
                                if p.path
                                    .segments
                                    .last()
                                    .map(|seg| seg.ident.to_string())
                                    .map(|ident| ident.len() > 1 && ident == ident.to_uppercase())
                                    .unwrap_or_default() =>
                            {
                                quote! { #idl::IdlGenericArg::Const { value: #p.to_string() } }
                            }
                            syn::GenericArgument::Type(ty) => {
                                let (ty, def) = gen_idl_type(ty, generic_params)?;
                                defined.extend(def);
                                quote! { #idl::IdlGenericArg::Type { ty: #ty } }
                            }
                            _ => {
                                return Err(syn::Error::new(
                                    arg.span(),
                                    "Unsupported generic argument",
                                ))
                            }
                        };
                        generics.push(generic);
                    }
                }
            }

            Ok((
                quote! {
                    #idl::IdlType::Defined {
                        name: <#path>::get_full_path(),
                        generics: vec![#(#generics),*],
                    }
                },
                defined,
            ))
        }
        syn::Type::Reference(reference) => match reference.elem.as_ref() {
            syn::Type::Slice(slice) if matches!(&*slice.elem, syn::Type::Path(path) if the_only_segment_is(path, "u8")) => {
                Ok((quote! {#idl::IdlType::Bytes}, vec![]))
            }
            _ => gen_idl_type(&reference.elem, generic_params),
        },
        _ => Err(syn::Error::new_spanned(ty, "Unsupported type")),
    }
}

fn get_first_segment(type_path: &syn::TypePath) -> &syn::PathSegment {
    type_path.path.segments.first().unwrap()
}
