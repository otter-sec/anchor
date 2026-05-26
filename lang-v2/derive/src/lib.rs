extern crate proc_macro;

mod access_control;
mod constant;
mod error_code;
mod idl;
mod init_space;
mod parse;
mod pda;
mod pod_wrapper;

use {
    proc_macro::TokenStream,
    proc_macro2::TokenStream as TokenStream2,
    quote::quote,
    syn::{
        parse::Parser, parse_macro_input, spanned::Spanned, Data, DeriveInput, Expr, Fields, FnArg,
        Ident, ItemMod, ItemStruct, Pat, Type,
    },
};

// ---------------------------------------------------------------------------
// #[derive(Accounts)]
// ---------------------------------------------------------------------------

#[proc_macro_derive(Accounts, attributes(account, instruction, accounts_program_id))]
pub fn derive_accounts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(impl_accounts(&input))
}

// ---------------------------------------------------------------------------
// #[derive(ToCpiAccounts)]
// ---------------------------------------------------------------------------

#[proc_macro_derive(
    ToCpiAccounts,
    attributes(signer, nested, account_meta, accounts_program_id)
)]
pub fn derive_to_cpi_accounts(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(impl_to_cpi_accounts(&input))
}

#[derive(Clone, Copy)]
enum CpiFieldKind {
    Readonly,
    Writable,
    OptionalReadonly,
    OptionalWritable,
    Nested,
    Phantom,
}

struct SignerExpr {
    present: bool,
    expr: TokenStream2,
}

#[derive(Default)]
struct AccountMetaAttrs {
    skip: bool,
    duplicate_readonly: bool,
}

fn impl_to_cpi_accounts(input: &DeriveInput) -> TokenStream2 {
    let fields = match &input.data {
        Data::Struct(s) => match &s.fields {
            Fields::Named(fields) => &fields.named,
            _ => {
                return syn::Error::new_spanned(
                    input,
                    "ToCpiAccounts can only be derived for structs with named fields",
                )
                .to_compile_error();
            }
        },
        _ => {
            return syn::Error::new_spanned(input, "ToCpiAccounts can only be derived for structs")
                .to_compile_error();
        }
    };

    let accounts_program_id = match accounts_program_id_expr(input) {
        Ok(expr) => expr,
        Err(err) => return err.to_compile_error(),
    };

    let mut cpi_fields = Vec::new();
    let mut cpi_lifetime = None::<syn::Lifetime>;
    for field in fields {
        let ident = field
            .ident
            .as_ref()
            .expect("named fields always have identifiers");
        let account_meta = match account_meta_attrs(field) {
            Ok(attrs) => attrs,
            Err(err) => return err.to_compile_error(),
        };
        let signer = match signer_expr_attr(field) {
            Ok(signer) => signer,
            Err(err) => return err.to_compile_error(),
        };
        let nested = match has_nested_attr(field) {
            Ok(nested) => nested,
            Err(err) => return err.to_compile_error(),
        };
        if account_meta.skip {
            if signer.present {
                return syn::Error::new_spanned(
                    field,
                    "#[account_meta(skip)] cannot be combined with #[signer]",
                )
                .to_compile_error();
            }
            if nested {
                return syn::Error::new_spanned(
                    field,
                    "#[account_meta(skip)] cannot be combined with #[nested]",
                )
                .to_compile_error();
            }
            continue;
        }
        if nested && signer.present {
            return syn::Error::new_spanned(field, "#[nested] cannot be combined with #[signer]")
                .to_compile_error();
        }
        let (kind, lifetime) = match cpi_field_kind(&field.ty, nested) {
            Ok(kind) => kind,
            Err(err) => return err.to_compile_error(),
        };
        if matches!(kind, CpiFieldKind::Phantom) && signer.present {
            return syn::Error::new_spanned(
                field,
                "PhantomData fields cannot be combined with #[signer]",
            )
            .to_compile_error();
        }
        if let Some(existing) = &cpi_lifetime {
            if existing.ident != lifetime.ident {
                return syn::Error::new_spanned(
                    &field.ty,
                    "all CpiHandle and CpiHandleMut fields must use the same lifetime",
                )
                .to_compile_error();
            }
        } else {
            cpi_lifetime = Some(lifetime);
        }
        if !matches!(kind, CpiFieldKind::Phantom) {
            if account_meta.duplicate_readonly {
                if !matches!(kind, CpiFieldKind::Readonly | CpiFieldKind::Writable) {
                    return syn::Error::new_spanned(
                        field,
                        "#[account_meta(duplicate_readonly)] requires a direct CpiHandle or CpiHandleMut field",
                    )
                    .to_compile_error();
                }
                cpi_fields.push((ident, CpiFieldKind::Readonly, quote! { false }));
            }
            cpi_fields.push((ident, kind, signer.expr));
        }
    }

    let Some(cpi_lifetime) = cpi_lifetime else {
        return syn::Error::new_spanned(
            input,
            "ToCpiAccounts requires at least one CpiHandle or CpiHandleMut field",
        )
        .to_compile_error();
    };

    let name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    let meta_steps = cpi_fields.iter().map(|(ident, kind, signer)| {
        let writable = matches!(
            kind,
            CpiFieldKind::Writable | CpiFieldKind::OptionalWritable
        );
        match kind {
            CpiFieldKind::Phantom => quote! {},
            CpiFieldKind::Nested => quote! {
                __accounts.extend(anchor_lang_v2::ToCpiAccounts::to_instruction_accounts(&self.#ident));
            },
            CpiFieldKind::Readonly | CpiFieldKind::Writable => quote! {
                __accounts.push(
                    anchor_lang_v2::pinocchio::instruction::InstructionAccount::new(
                        self.#ident.address(),
                        #writable,
                        #signer,
                    ),
                );
            },
            CpiFieldKind::OptionalReadonly | CpiFieldKind::OptionalWritable => quote! {
                match self.#ident {
                    ::core::option::Option::Some(__account) => {
                        __accounts.push(
                            anchor_lang_v2::pinocchio::instruction::InstructionAccount::new(
                                __account.address(),
                                #writable,
                                #signer,
                            ),
                        );
                    }
                    ::core::option::Option::None => {
                        __accounts.push(
                            anchor_lang_v2::pinocchio::instruction::InstructionAccount::readonly(
                                &#accounts_program_id,
                            ),
                        );
                    }
                }
            },
        }
    });

    let handle_steps = cpi_fields.iter().map(|(ident, kind, _)| match kind {
        CpiFieldKind::Phantom => quote! {},
        CpiFieldKind::Nested => quote! {
            __handles.extend(anchor_lang_v2::ToCpiAccounts::to_cpi_handles(&self.#ident));
        },
        CpiFieldKind::Readonly => quote! {
            __handles.push(self.#ident);
        },
        CpiFieldKind::Writable => quote! {
            __handles.push(self.#ident.into());
        },
        CpiFieldKind::OptionalReadonly => quote! {
            if let ::core::option::Option::Some(__account) = self.#ident {
                __handles.push(__account);
            }
        },
        CpiFieldKind::OptionalWritable => quote! {
            if let ::core::option::Option::Some(__account) = self.#ident {
                __handles.push(__account.into());
            }
        },
    });

    quote! {
        impl #impl_generics anchor_lang_v2::ToCpiAccounts<#cpi_lifetime>
            for #name #ty_generics #where_clause
        {
            fn to_instruction_accounts(
                &self,
            ) -> anchor_lang_v2::__alloc::vec::Vec<
                anchor_lang_v2::pinocchio::instruction::InstructionAccount<#cpi_lifetime>,
            > {
                let mut __accounts = anchor_lang_v2::__alloc::vec::Vec::new();
                #(#meta_steps)*
                __accounts
            }

            fn to_cpi_handles(
                &self,
            ) -> anchor_lang_v2::__alloc::vec::Vec<anchor_lang_v2::CpiHandle<#cpi_lifetime>> {
                let mut __handles = anchor_lang_v2::__alloc::vec::Vec::new();
                #(#handle_steps)*
                __handles
            }
        }
    }
}

fn accounts_program_id_expr(input: &DeriveInput) -> syn::Result<TokenStream2> {
    for attr in &input.attrs {
        if !attr.path().is_ident("accounts_program_id") {
            continue;
        }
        return match &attr.meta {
            syn::Meta::List(list) => {
                let expr: Expr = syn::parse2(list.tokens.clone())?;
                Ok(quote! { #expr })
            }
            syn::Meta::NameValue(nv) => {
                let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit),
                    ..
                }) = &nv.value
                else {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "expected #[accounts_program_id(expr)] or #[accounts_program_id = \"expr\"]",
                    ));
                };
                let expr: Expr = lit.parse()?;
                Ok(quote! { #expr })
            }
            _ => Err(syn::Error::new_spanned(
                attr,
                "expected #[accounts_program_id(expr)] or #[accounts_program_id = \"expr\"]",
            )),
        };
    }
    Ok(quote! { crate::ID })
}

fn signer_expr_attr(field: &syn::Field) -> syn::Result<SignerExpr> {
    let mut signer = None::<TokenStream2>;
    for attr in &field.attrs {
        if !attr.path().is_ident("signer") {
            continue;
        }
        if signer.is_some() {
            return Err(syn::Error::new_spanned(
                attr,
                "duplicate #[signer] attribute",
            ));
        }
        signer = Some(match &attr.meta {
            syn::Meta::Path(_) => quote! { true },
            syn::Meta::NameValue(nv) => {
                let expr = &nv.value;
                quote! { #expr }
            }
            syn::Meta::List(list) => {
                let expr: Expr = syn::parse2(list.tokens.clone())?;
                quote! { #expr }
            }
        });
    }
    Ok(match signer {
        Some(expr) => SignerExpr {
            present: true,
            expr,
        },
        None => SignerExpr {
            present: false,
            expr: quote! { false },
        },
    })
}

fn has_nested_attr(field: &syn::Field) -> syn::Result<bool> {
    let mut nested = false;
    for attr in &field.attrs {
        if !attr.path().is_ident("nested") {
            continue;
        }
        if nested {
            return Err(syn::Error::new_spanned(
                attr,
                "duplicate #[nested] attribute",
            ));
        }
        if !matches!(attr.meta, syn::Meta::Path(_)) {
            return Err(syn::Error::new_spanned(attr, "expected #[nested]"));
        }
        nested = true;
    }
    Ok(nested)
}

fn account_meta_attrs(field: &syn::Field) -> syn::Result<AccountMetaAttrs> {
    let mut attrs = AccountMetaAttrs::default();
    for attr in &field.attrs {
        if !attr.path().is_ident("account_meta") {
            continue;
        }
        if !matches!(attr.meta, syn::Meta::List(_)) {
            return Err(syn::Error::new_spanned(
                attr,
                "expected #[account_meta(skip)]",
            ));
        }
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("skip") {
                if attrs.skip {
                    return Err(meta.error("duplicate account_meta skip"));
                }
                attrs.skip = true;
                Ok(())
            } else if meta.path.is_ident("duplicate_readonly") {
                if attrs.duplicate_readonly {
                    return Err(meta.error("duplicate account_meta duplicate_readonly"));
                }
                attrs.duplicate_readonly = true;
                Ok(())
            } else {
                Err(meta.error("unsupported account_meta option"))
            }
        })?;
    }
    if attrs.skip && attrs.duplicate_readonly {
        return Err(syn::Error::new_spanned(
            field,
            "#[account_meta(skip)] cannot be combined with #[account_meta(duplicate_readonly)]",
        ));
    }
    Ok(attrs)
}

fn cpi_field_kind(ty: &Type, nested: bool) -> syn::Result<(CpiFieldKind, syn::Lifetime)> {
    if nested {
        if option_inner(ty).is_some() {
            return Err(syn::Error::new_spanned(
                ty,
                "#[nested] fields cannot be wrapped in Option",
            ));
        }
        let Some(lifetime) = path_type_first_lifetime(ty) else {
            return Err(syn::Error::new_spanned(
                ty,
                "#[nested] fields must name a type carrying the CPI lifetime, e.g. Inner<'a>",
            ));
        };
        return Ok((CpiFieldKind::Nested, lifetime));
    }
    if let Some(lifetime) = phantom_data_lifetime(ty) {
        return Ok((CpiFieldKind::Phantom, lifetime));
    }
    if let Some((inner, _)) = option_inner(ty) {
        let (kind, lifetime) = direct_cpi_field_kind(inner)?;
        return match kind {
            CpiFieldKind::Readonly => Ok((CpiFieldKind::OptionalReadonly, lifetime)),
            CpiFieldKind::Writable => Ok((CpiFieldKind::OptionalWritable, lifetime)),
            _ => unreachable!("direct_cpi_field_kind never returns optional kinds"),
        };
    }
    direct_cpi_field_kind(ty)
}

fn phantom_data_lifetime(ty: &Type) -> Option<syn::Lifetime> {
    let Type::Path(tp) = ty else {
        return None;
    };
    let seg = tp.path.segments.last()?;
    if seg.ident != "PhantomData" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    let syn::GenericArgument::Type(Type::Reference(reference)) = args.args.first()? else {
        return None;
    };
    let Type::Tuple(tuple) = reference.elem.as_ref() else {
        return None;
    };
    if !tuple.elems.is_empty() {
        return None;
    }
    reference.lifetime.clone()
}

fn direct_cpi_field_kind(ty: &Type) -> syn::Result<(CpiFieldKind, syn::Lifetime)> {
    let Some((ident, lifetime)) = path_type_ident_and_lifetime(ty) else {
        return Err(syn::Error::new_spanned(
            ty,
            "expected CpiHandle<'a>, CpiHandleMut<'a>, Option<CpiHandle<'a>>, or Option<CpiHandleMut<'a>>",
        ));
    };
    match ident.to_string().as_str() {
        "CpiHandle" => Ok((CpiFieldKind::Readonly, lifetime)),
        "CpiHandleMut" => Ok((CpiFieldKind::Writable, lifetime)),
        _ => Err(syn::Error::new_spanned(
            ty,
            "expected CpiHandle<'a> or CpiHandleMut<'a>",
        )),
    }
}

fn option_inner(ty: &Type) -> Option<(&Type, &syn::Path)> {
    let Type::Path(tp) = ty else {
        return None;
    };
    let seg = tp.path.segments.last()?;
    if seg.ident != "Option" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    let syn::GenericArgument::Type(inner) = args.args.first()? else {
        return None;
    };
    Some((inner, &tp.path))
}

fn path_type_ident_and_lifetime(ty: &Type) -> Option<(Ident, syn::Lifetime)> {
    let Type::Path(tp) = ty else {
        return None;
    };
    let seg = tp.path.segments.last()?;
    let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
        return None;
    };
    if args.args.len() != 1 {
        return None;
    }
    let syn::GenericArgument::Lifetime(lifetime) = args.args.first()? else {
        return None;
    };
    Some((seg.ident.clone(), lifetime.clone()))
}

fn path_type_first_lifetime(ty: &Type) -> Option<syn::Lifetime> {
    let Type::Path(tp) = ty else {
        return None;
    };
    tp.path.segments.iter().find_map(|seg| {
        let syn::PathArguments::AngleBracketed(args) = &seg.arguments else {
            return None;
        };
        args.args.iter().find_map(|arg| match arg {
            syn::GenericArgument::Lifetime(lifetime) => Some(lifetime.clone()),
            _ => None,
        })
    })
}

/// Returns true if `ty` needs the `'ix` lifetime injected when used as an
/// instruction arg. This is the case for top-level references (`&[u8]`, `&T`)
/// and for path types carrying lifetime generic args (`CreateArgs<'_>`,
/// `Option<&[u8]>`, etc.).
fn needs_ix_lifetime(ty: &Type) -> bool {
    match ty {
        Type::Reference(_) => true,
        Type::Path(tp) => tp.path.segments.iter().any(|seg| {
            if let syn::PathArguments::AngleBracketed(ab) = &seg.arguments {
                ab.args.iter().any(|arg| match arg {
                    syn::GenericArgument::Lifetime(_) => true,
                    syn::GenericArgument::Type(inner) => needs_ix_lifetime(inner),
                    _ => false,
                })
            } else {
                false
            }
        }),
        _ => false,
    }
}

/// Recursively rewrites any elided or named lifetimes in `ty` to `ix`.
///
/// - `&[T]` / `&T` with elided lifetime → `&'ix [T]` / `&'ix T`
///   (explicit lifetimes on references are preserved — a handler asking for
///   `&'static [u8]` still gets `'static`)
/// - `Foo<'_>`, `Foo<'a, ...>` → `Foo<'ix, ...>` (every lifetime arg in the
///   path gets rewritten; users can't introduce named lifetimes at the
///   handler scope anyway)
/// - Nested types are walked (`Option<&[u8]>`, `Result<Args<'_>, E>`, ...)
///
/// This lets a handler fn take a borrowed struct arg like
/// `args: MyArgs<'_>` and have the generated `__Args` struct bind the
/// lifetime correctly.
fn with_ix_lifetime(ty: &Type, ix: &syn::Lifetime) -> Type {
    match ty {
        Type::Reference(tr) => {
            let mut new_tr = tr.clone();
            let is_elided = new_tr
                .lifetime
                .as_ref()
                .map(|l| l.ident == "_")
                .unwrap_or(true);
            if is_elided {
                new_tr.lifetime = Some(ix.clone());
            }
            new_tr.elem = Box::new(with_ix_lifetime(&new_tr.elem, ix));
            Type::Reference(new_tr)
        }
        Type::Path(tp) => {
            let mut new_tp = tp.clone();
            for seg in new_tp.path.segments.iter_mut() {
                if let syn::PathArguments::AngleBracketed(ab) = &mut seg.arguments {
                    for arg in ab.args.iter_mut() {
                        match arg {
                            syn::GenericArgument::Lifetime(lt) => {
                                *lt = ix.clone();
                            }
                            syn::GenericArgument::Type(inner) => {
                                *inner = with_ix_lifetime(inner, ix);
                            }
                            _ => {}
                        }
                    }
                }
            }
            Type::Path(new_tp)
        }
        _ => ty.clone(),
    }
}

fn nested_client_accounts_type(ty: &Type) -> Option<TokenStream2> {
    let inner = parse::extract_nested_inner_type(ty)?;
    let Type::Path(inner_path) = inner else {
        return None;
    };
    let inner_ident = &inner_path.path.segments.last()?.ident;
    let module_ident = Ident::new(
        &format!(
            "__client_accounts_{}",
            inner_ident.to_string().to_lowercase()
        ),
        inner_ident.span(),
    );
    Some(quote! { super::#module_ident::#inner_ident })
}

fn idl_field_ty(field: &parse::AccountField) -> Option<&Type> {
    field.idl_field_ty.as_ref()
}

fn client_meta_signer_expr(field: &parse::AccountField) -> TokenStream2 {
    let init_signer = field.idl_init_signer;
    match idl_field_ty(field) {
        Some(ty) => quote! {
            if <#ty as anchor_lang_v2::AnchorAccount>::IS_SIGNER || #init_signer {
                _is_signer.unwrap_or(true)
            } else {
                false
            }
        },
        None if init_signer => quote! { _is_signer.unwrap_or(true) },
        None => quote! { false },
    }
}

fn cpi_meta_signer_expr(field: &parse::AccountField) -> TokenStream2 {
    let init_signer = field.idl_init_signer;
    match idl_field_ty(field) {
        Some(ty) => quote! {
            <#ty as anchor_lang_v2::AnchorAccount>::IS_SIGNER || #init_signer
        },
        None => quote! { #init_signer },
    }
}

struct ArgsDeser {
    deser: TokenStream2,
    arg_types: Vec<Type>,
}

/// Compute lifetime-rewritten argument types plus a flag indicating whether
/// any argument carries an instruction-data borrow.
fn args_meta(args: &[(&Ident, &Type)]) -> (Vec<Type>, bool) {
    let ix_lifetime: syn::Lifetime = syn::parse_quote!('ix);
    let arg_types: Vec<Type> = args
        .iter()
        .map(|(_, t)| with_ix_lifetime(t, &ix_lifetime))
        .collect();
    let has_refs = args.iter().any(|(_, t)| needs_ix_lifetime(t));
    (arg_types, has_refs)
}

/// Build the `#[derive(SchemaRead)] struct + deserialize` block for a list of
/// `(name, type)` argument pairs. Used by both `#[instruction(...)]` in
/// `impl_accounts` and handler extra-args in `impl_program`.
///
/// `inline_error`: when `true`, deser failure returns a `u64` directly (handler
/// wrapper context); when `false`, it returns `Err(...)` (try_accounts context).
fn emit_args_deser(args: &[(&Ident, &Type)], struct_name: &str, inline_error: bool) -> ArgsDeser {
    let ix_lifetime: syn::Lifetime = syn::parse_quote!('ix);
    let arg_types: Vec<Type> = args
        .iter()
        .map(|(_, t)| with_ix_lifetime(t, &ix_lifetime))
        .collect();
    let has_refs = args.iter().any(|(_, t)| needs_ix_lifetime(t));
    let (lt_decl, lt_use) = if has_refs {
        (quote! { <'ix> }, quote! { <'_> })
    } else {
        (quote! {}, quote! {})
    };

    let names: Vec<_> = args.iter().map(|(n, _)| *n).collect();
    let struct_ident = Ident::new(struct_name, proc_macro2::Span::call_site());

    let deser = if args.is_empty() {
        quote! {}
    } else {
        let error_handling = if inline_error {
            quote! {
                match anchor_lang_v2::wincode::config::deserialize(
                    __ix_data,
                    anchor_lang_v2::BORSH_CONFIG,
                ) {
                    Ok(__v) => __v,
                    Err(_) => return {
                        let __e: anchor_lang_v2::Error =
                            anchor_lang_v2::ErrorCode::InstructionDidNotDeserialize.into();
                        __e.into()
                    },
                }
            }
        } else {
            quote! {
                anchor_lang_v2::wincode::config::deserialize(
                    __ix_data,
                    anchor_lang_v2::BORSH_CONFIG,
                )
                    .map_err(|_| anchor_lang_v2::ErrorCode::InstructionDidNotDeserialize)?
            }
        };
        quote! {
            #[derive(anchor_lang_v2::wincode::SchemaRead)]
            struct #struct_ident #lt_decl { #(#names: #arg_types,)* }
            let __args: #struct_ident #lt_use = #error_handling;
            #(let #names = __args.#names;)*
        }
    };

    ArgsDeser { deser, arg_types }
}

/// Parse `#[instruction(name: Type, ...)]` from struct-level attributes.
/// Returns a list of (name, type) pairs.
fn parse_instruction_attrs(attrs: &[syn::Attribute]) -> syn::Result<Vec<(Ident, Type)>> {
    let mut result = Vec::new();
    for attr in attrs {
        if !attr.path().is_ident("instruction") {
            continue;
        }
        attr.parse_args_with(|input: syn::parse::ParseStream| {
            while !input.is_empty() {
                let name: Ident = input.parse()?;
                input.parse::<syn::Token![:]>()?;
                let ty: Type = input.parse()?;
                result.push((name, ty));
                if !input.is_empty() {
                    input.parse::<syn::Token![,]>()?;
                }
            }
            Ok(())
        })?;
    }
    Ok(result)
}

/// Parse the internal `#[accounts_program_id(expr)]` override used by
/// `declare_program!` for generated account structs. Ordinary user-written
/// `#[derive(Accounts)]` structs continue to default to the current crate's ID.
fn parse_accounts_program_id_attr(attrs: &[syn::Attribute]) -> syn::Result<Expr> {
    let mut program_id = None;
    for attr in attrs {
        if !attr.path().is_ident("accounts_program_id") {
            continue;
        }
        if program_id.is_some() {
            return Err(syn::Error::new(
                attr.span(),
                "duplicate `accounts_program_id` attribute",
            ));
        }
        program_id = Some(match &attr.meta {
            syn::Meta::List(_) => attr.parse_args::<Expr>()?,
            syn::Meta::NameValue(nv) => {
                let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Str(lit),
                    ..
                }) = &nv.value
                else {
                    return Err(syn::Error::new_spanned(
                        attr,
                        "expected #[accounts_program_id(expr)] or #[accounts_program_id = \"expr\"]",
                    ));
                };
                lit.parse()?
            }
            _ => {
                return Err(syn::Error::new_spanned(
                    attr,
                    "expected #[accounts_program_id(expr)] or #[accounts_program_id = \"expr\"]",
                ));
            }
        });
    }
    Ok(program_id.unwrap_or_else(|| syn::parse_quote!(crate::ID)))
}

fn impl_accounts(input: &DeriveInput) -> TokenStream2 {
    let name = &input.ident;
    let bumps_name = syn::Ident::new(&format!("{name}Bumps"), name.span());
    let accounts_program_id = match parse_accounts_program_id_attr(&input.attrs) {
        Ok(program_id) => program_id,
        Err(err) => return err.to_compile_error(),
    };

    // Bail with a properly-spanned diagnostic on unsupported shapes.
    let named_fields = match &input.data {
        Data::Struct(data) => match &data.fields {
            Fields::Named(named) => named,
            _ => {
                return syn::Error::new(name.span(), "`Accounts` derive only supports named fields")
                    .to_compile_error()
            }
        },
        _ => {
            return syn::Error::new(name.span(), "`Accounts` derive only supports structs")
                .to_compile_error()
        }
    };

    // Collect field names first so we can rewrite bare-ident seed expressions.
    let raw_field_names: Vec<String> = named_fields
        .named
        .iter()
        .filter_map(|f| f.ident.as_ref().map(|i| i.to_string()))
        .collect();

    if named_fields.named.len() > 255 {
        return syn::Error::new(name.span(), "`Accounts` derive supports at most 255 fields")
            .to_compile_error();
    }

    // Parse #[instruction(arg: Type, ...)] — consumed both for the
    // runtime deser block AND for PDA seed classification: seeds that
    // reference an ix arg resolve to `IdlSeed::Arg{path}`.
    let ix_args = match parse_instruction_attrs(&input.attrs) {
        Ok(args) => args,
        Err(err) => return err.to_compile_error(),
    };
    let ix_arg_names: Vec<String> = ix_args.iter().map(|(n, _)| n.to_string()).collect();

    // Compute the views-slice offset for each field. Direct fields occupy 1
    // slot; `Nested<Inner>` fields occupy `Inner::HEADER_SIZE` slots. Each
    // offset is a const expression resolved at monomorphization time.
    let mut offset_exprs: Vec<proc_macro2::TokenStream> = Vec::new();
    let mut current_offset: proc_macro2::TokenStream = quote::quote! { 0usize };
    for f in named_fields.named.iter() {
        offset_exprs.push(current_offset.clone());
        if let Some(inner_ty) = parse::extract_nested_inner_type(&f.ty) {
            current_offset = quote::quote! {
                #current_offset + <#inner_ty as anchor_lang_v2::TryAccounts>::HEADER_SIZE
            };
        } else {
            current_offset = quote::quote! { #current_offset + 1 };
        }
    }
    let field_offsets: Vec<(String, proc_macro2::TokenStream)> = raw_field_names
        .iter()
        .cloned()
        .zip(offset_exprs.iter().cloned())
        .collect();

    let fields: Vec<parse::AccountField> = match named_fields
        .named
        .iter()
        .zip(offset_exprs)
        .map(|(f, offset)| {
            parse::parse_field(f, &raw_field_names, &field_offsets, offset, &ix_arg_names)
        })
        .collect::<syn::Result<_>>()
    {
        Ok(fields) => fields,
        Err(err) => return err.to_compile_error(),
    };

    for field in fields.iter().filter(|f| f.init_payer.is_some()) {
        let payer = field.init_payer.as_ref().unwrap();
        match fields
            .iter()
            .find(|candidate| candidate.name == payer.as_str())
        {
            Some(payer_field) if payer_field.idl_writable => {}
            Some(_) => {
                return syn::Error::new(
                    field.name.span(),
                    "the payer specified for an init constraint must be mutable",
                )
                .to_compile_error();
            }
            None => {
                return syn::Error::new(
                    field.name.span(),
                    "the payer specified for an init constraint does not exist",
                )
                .to_compile_error();
            }
        }
    }

    let field_names: Vec<_> = fields.iter().map(|f| &f.name).collect();
    let loads: Vec<_> = fields.iter().map(|f| &f.load).collect();
    let deferred_loads: Vec<_> = fields
        .iter()
        .filter_map(|f| f.deferred_load.as_ref())
        .collect();
    let constraints: Vec<_> = fields.iter().flat_map(|f| &f.constraints).collect();
    let exits: Vec<_> = fields.iter().filter_map(|f| f.exit.as_ref()).collect();
    // Collect per-field dup checks under a single outer `if let Some(__dups)`
    // gate so non-dup txs pay one Option-tag branch for the whole struct,
    // not one per mut field.
    let dup_checks: Vec<_> = fields.iter().filter_map(|f| f.dup_check.as_ref()).collect();
    let dup_check_block = if dup_checks.is_empty() {
        quote::quote! {}
    } else {
        quote::quote! {
            if let Some(__dups) = __duplicates {
                #(#dup_checks)*
            }
        }
    };
    // Bumps fields. Optional accounts get `Option<u8>` so the default
    // (`None`) maps cleanly to the sentinel-`None` load path; the seeds
    // check assigns `Some(bump)` only when the inner is `Some`. Mirrors
    // v1's `bumps.rs:36` Optional handling.
    let bump_fields: Vec<proc_macro2::TokenStream> = fields
        .iter()
        .filter(|f| f.has_bump)
        .map(|f| {
            let n = &f.name;
            if f.is_optional {
                quote! { #n: Option<u8> }
            } else {
                quote! { #n: u8 }
            }
        })
        .collect();

    // Compile-time sum for `<T as TryAccounts>::HEADER_SIZE`:
    //   - 1 per non-`Nested<_>` field (consumes one account view)
    //   - `<Inner as TryAccounts>::HEADER_SIZE` per `Nested<Inner>` field,
    //     which recursively expands at monomorphization time.
    // The direct-field count is a single literal so the emitted
    // const is short in the common (no-nested) case.
    let direct_count: usize = fields
        .iter()
        .filter(|f| !parse::is_nested_type(&f.ty))
        .count();
    let nested_inner_types: Vec<&syn::Type> = fields
        .iter()
        .filter_map(|f| parse::extract_nested_inner_type(&f.ty))
        .collect();
    let header_size_expr = if nested_inner_types.is_empty() {
        quote::quote! { #direct_count }
    } else {
        quote::quote! {
            #direct_count #(+ <#nested_inner_types as anchor_lang_v2::TryAccounts>::HEADER_SIZE)*
        }
    };

    // Compile-time `MUT_MASK` composition:
    //   - bit at `offset` per direct mut field (non-Option, non-`unsafe(dup)`)
    //   - `<Inner as TryAccounts>::MUT_MASK << child_offset` per `Nested<Inner>`
    // Folded into a single `const` expression so LLVM sees a literal at
    // `run_handler`'s inline site — zero runtime composition cost, and the
    // `intersects(&T::MUT_MASK)` call const-folds away entirely when the
    // resulting mask is all-zero.
    let mut_mask_steps: Vec<proc_macro2::TokenStream> = fields
        .iter()
        .filter_map(|f| {
            let offset = &f.offset_expr;
            if f.contributes_mut_bit {
                Some(quote! {
                    __mask = anchor_lang_v2::mut_mask_set_bit(__mask, #offset);
                })
            } else if let Some(inner_ty) = parse::extract_nested_inner_type(&f.ty) {
                Some(quote! {
                    __mask = anchor_lang_v2::mut_mask_or_shifted(
                        __mask,
                        <#inner_ty as anchor_lang_v2::TryAccounts>::MUT_MASK,
                        #offset,
                    );
                })
            } else {
                None
            }
        })
        .collect();
    let mut_mask_expr = if mut_mask_steps.is_empty() {
        quote::quote! { [0u64; 4] }
    } else {
        quote::quote! {
            {
                let mut __mask = [0u64; 4];
                #(#mut_mask_steps)*
                __mask
            }
        }
    };
    let dynamic_mut_mask_terms: Vec<proc_macro2::TokenStream> = fields
        .iter()
        .filter_map(|f| {
            if f.contributes_active_mut_bit {
                Some(quote::quote! { true })
            } else {
                parse::extract_nested_inner_type(&f.ty).map(|inner_ty| {
                    quote::quote! {
                        <#inner_ty as anchor_lang_v2::TryAccounts>::HAS_DYNAMIC_MUT_MASK
                    }
                })
            }
        })
        .collect();
    let has_dynamic_mut_mask_expr = if dynamic_mut_mask_terms.is_empty() {
        quote::quote! { false }
    } else {
        quote::quote! {
            false #(|| #dynamic_mut_mask_terms)*
        }
    };
    let active_mut_mask_steps: Vec<proc_macro2::TokenStream> = fields
        .iter()
        .filter_map(|f| {
            let field_name = &f.name;
            let offset = &f.offset_expr;
            if f.contributes_active_mut_bit {
                Some(quote! {
                    if self.#field_name.is_some() {
                        __mask = anchor_lang_v2::mut_mask_set_bit(__mask, #offset);
                    }
                })
            } else if let Some(inner_ty) = parse::extract_nested_inner_type(&f.ty) {
                Some(quote! {
                    if <#inner_ty as anchor_lang_v2::TryAccounts>::HAS_DYNAMIC_MUT_MASK {
                        __mask = anchor_lang_v2::mut_mask_or_shifted(
                            __mask,
                            <#inner_ty as anchor_lang_v2::TryAccounts>::active_mut_mask(&self.#field_name.0),
                            #offset,
                        );
                    }
                })
            } else {
                None
            }
        })
        .collect();
    let active_mut_mask_body = if active_mut_mask_steps.is_empty() {
        quote::quote! { Self::MUT_MASK }
    } else {
        quote::quote! {
            let mut __mask = Self::MUT_MASK;
            #(#active_mut_mask_steps)*
            __mask
        }
    };

    // IDL collection — the accounts-JSON emission is a runtime function
    // (not a `&'static str` const) so it can read
    // `<FieldTy as IdlAccountType>::__IDL_IS_SIGNER / __IDL_ADDRESS` off
    // the wrapper type. Compile-time flags (writable, init_signer,
    // optional, relations) are baked directly into the format strings, so
    // the runtime only pays for trait dispatch + concatenation.
    let field_names_str: Vec<String> = fields.iter().map(|f| f.name.to_string()).collect();

    // Build the inverse has_one mapping: relations on field `X` lists every
    // sibling whose `has_one = X` chain targets X. Mirrors v1's
    // `get_relations` — relations live on the target, not the source.
    //
    // Two sources feed this map:
    //   * `has_one = X` on field `f` → push `f` to `X.relations`.
    //   * `address = <sibling>.<f>` on `f` (the v1-encodable shape) →
    //     same inverse as if `<sibling>` had written `has_one = f`: push
    //     `<sibling>` to `f.relations`. The non-encodable shapes (const
    //     paths, subfield name ≠ `f`) leave `idl_address_v1_source = None`
    //     and land in the `address` JSON key instead.
    let relations_by_target: std::collections::HashMap<String, Vec<String>> = {
        let mut m: std::collections::HashMap<String, Vec<String>> =
            std::collections::HashMap::new();
        for f in &fields {
            let src = f.name.to_string();
            for target in &f.idl_has_one {
                m.entry(target.clone()).or_default().push(src.clone());
            }
            if let Some(sibling) = &f.idl_address_v1_source {
                m.entry(src.clone()).or_default().push(sibling.clone());
            }
        }
        m
    };

    // Pre-build per-field `pda` body emission. Each entry is a token
    // expression that evaluates to the JSON string at IDL-build time;
    // `build_accounts_emission` splices it into the runtime assembly.
    let pda_jsons: Vec<Option<proc_macro2::TokenStream>> = fields
        .iter()
        .map(|f| {
            f.idl_pda
                .as_ref()
                .map(|p| idl::pda_object_emission(&p.seeds, p.program.as_ref()))
        })
        .collect();

    let accounts_fields: Vec<idl::AccountsJsonField<'_>> = fields
        .iter()
        .enumerate()
        .map(|(i, f)| {
            let name: &str = &field_names_str[i];
            let relations: Vec<&str> = relations_by_target
                .get(name)
                .map(|v| v.iter().map(|s| s.as_str()).collect())
                .unwrap_or_default();
            idl::AccountsJsonField {
                name,
                writable: f.idl_writable,
                init_signer: f.idl_init_signer,
                is_optional: f.is_optional,
                relations,
                docs: &f.idl_docs,
                pda_json: pda_jsons[i].clone(),
                field_ty: &f.idl_field_ty,
                address_override: f.idl_address.as_deref(),
                // `Nested<Inner>` flattens at IDL emission time by calling
                // into `Inner::__idl_accounts()`. Grab the inner `Type` so
                // the emitter can synthesize that call.
                nested_inner_ty: parse::extract_nested_inner_type(&f.ty),
            }
        })
        .collect();
    let idl_accounts_fn = idl::build_accounts_emission(&accounts_fields);
    // Only path-typed fields drive the IDL dep walk. `idl_field_ty` is
    // already the post-Option-unwrap base type (see `parse::parse_field`),
    // and is `None` for non-Path fields — filter those out so the emitted
    // trait call has a concrete type to dispatch on.
    let idl_field_tys: Vec<&syn::Type> = fields
        .iter()
        .filter_map(|f| f.idl_field_ty.as_ref())
        .collect();

    let (ix_deser, ix_args_assoc, ix_args_return) = if ix_args.is_empty() {
        (quote! {}, quote! { type IxArgs<'ix> = (); }, quote! { () })
    } else {
        let pairs: Vec<(&Ident, &Type)> = ix_args.iter().map(|(n, t)| (n, t)).collect();
        let ix_args_deser = emit_args_deser(&pairs, "__IxArgs", false);
        let ix_arg_types = ix_args_deser.arg_types;
        let ix_arg_names: Vec<&Ident> = ix_args.iter().map(|(n, _)| n).collect();
        (
            ix_args_deser.deser,
            quote! { type IxArgs<'ix> = (#(#ix_arg_types,)*); },
            quote! { (#(#ix_arg_names,)*) },
        )
    };

    // Conditional bumps: empty → type alias, non-empty → struct
    let has_bumps = !bump_fields.is_empty();
    let bumps_def = if has_bumps {
        quote! {
            #[derive(Default, Clone)]
            pub struct #bumps_name { #(pub #bump_fields,)* }
        }
    } else {
        quote! { pub type #bumps_name = (); }
    };
    let bumps_init = if has_bumps {
        quote! { let mut __bumps = #bumps_name::default(); }
    } else {
        quote! { let __bumps = #bumps_name::default(); }
    };

    // --- Client-side struct for off-chain usage (tests, CPI, SDK) ---
    //
    // The struct only contains fields the user must provide (Signer, raw
    // accounts, etc.). Derivable fields (Program<T>, PDAs) are computed
    // inside `to_account_metas()` so the user never has to fill them.
    let client_mod_name = syn::Ident::new(
        &format!("__client_accounts_{}", name.to_string().to_lowercase()),
        name.span(),
    );

    // Classify each field as user-required or auto-derivable.
    #[derive(Clone)]
    enum FieldKind {
        Required,
        Program(proc_macro2::TokenStream),
        Pda {
            seed_exprs: Vec<proc_macro2::TokenStream>,
            deps: Vec<String>,
        },
    }

    let field_kinds: Vec<_> = fields
        .iter()
        .zip(named_fields.named.iter())
        .map(|(f, raw_field)| {
            let base_ty = match parse::extract_option_inner(&f.ty) {
                Some(inner) => inner,
                None => &f.ty,
            };
            let ty_name = parse::field_ty_str(base_ty);

            if ty_name == "Program" {
                if let Type::Path(tp) = base_ty {
                    if let Some(seg) = tp.path.segments.last() {
                        if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                            if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                                return (
                                    f,
                                    FieldKind::Program(quote! {
                                        <#inner as anchor_lang_v2::Id>::id()
                                    }),
                                );
                            }
                        }
                    }
                }
            }

            let attrs = parse::parse_account_attrs(&raw_field.attrs)
                .expect("parse_field already validated account attributes");
            if let Some(ref seeds_expr) = attrs.seeds {
                // Client-side PDA derivation only works when we can
                // inspect individual seed expressions — requires the
                // array-bracket form `seeds = [...]`. Expression-form
                // seeds (e.g. `seeds = my_fn()`) are opaque to the
                // macro and fall through to FieldKind::Required.
                if let syn::Expr::Array(arr) = seeds_expr {
                    if attrs.seeds_program.is_none() {
                        let mut seed_exprs = Vec::new();
                        let mut deps = Vec::new();
                        let mut all_derivable = true;

                        for seed in &arr.elems {
                            if let Some(bytes) = pda::seed_as_const_bytes(seed) {
                                let lits: Vec<_> = bytes.iter().map(|b| quote! { #b }).collect();
                                seed_exprs.push(quote! { &[#(#lits),*] as &[u8] });
                            } else if let Some(ref root) = idl::receiver_root_ident_str(seed) {
                                if raw_field_names.contains(root) {
                                    let ident =
                                        syn::Ident::new(root, proc_macro2::Span::call_site());
                                    seed_exprs.push(quote! { #ident.as_ref() });
                                    deps.push(root.clone());
                                } else {
                                    all_derivable = false;
                                    break;
                                }
                            } else {
                                all_derivable = false;
                                break;
                            }
                        }

                        if all_derivable {
                            return (f, FieldKind::Pda { seed_exprs, deps });
                        }
                    }
                }
            }

            (f, FieldKind::Required)
        })
        .collect();

    // Client struct fields: only the ones the user must provide.
    let client_fields: Vec<_> = field_kinds
        .iter()
        .filter(|(_, kind)| matches!(kind, FieldKind::Required))
        .map(|(f, _)| {
            let fname = &f.name;
            if let Some(nested_ty) = nested_client_accounts_type(&f.ty) {
                quote! { pub #fname: #nested_ty }
            } else if f.is_optional {
                quote! { pub #fname: Option<anchor_lang_v2::Address> }
            } else {
                quote! { pub #fname: anchor_lang_v2::Address }
            }
        })
        .collect();

    // Topo-sort derivable fields: programs first, then PDAs by dependency order.
    let required_names: std::collections::HashSet<String> = field_kinds
        .iter()
        .filter(|(_, kind)| matches!(kind, FieldKind::Required))
        .map(|(f, _)| f.name.to_string())
        .collect();

    let mut derive_order: Vec<usize> = Vec::new();
    let mut resolved = required_names.clone();
    for (i, (f, kind)) in field_kinds.iter().enumerate() {
        if matches!(kind, FieldKind::Program(_)) {
            derive_order.push(i);
            resolved.insert(f.name.to_string());
        }
    }
    let mut progress = true;
    while progress {
        progress = false;
        for (i, (f, kind)) in field_kinds.iter().enumerate() {
            let name_str = f.name.to_string();
            if resolved.contains(&name_str) {
                continue;
            }
            if let FieldKind::Pda { deps, .. } = kind {
                if deps.iter().all(|dep| resolved.contains(dep)) {
                    derive_order.push(i);
                    resolved.insert(name_str);
                    progress = true;
                }
            }
        }
    }

    // Inside to_account_metas: copy required fields to locals, then derive the rest.
    let required_locals: Vec<_> = field_kinds
        .iter()
        .filter(|(_, kind)| matches!(kind, FieldKind::Required))
        .map(|(f, _)| {
            let fname = &f.name;
            if nested_client_accounts_type(&f.ty).is_some() {
                quote! { let #fname = &self.#fname; }
            } else {
                quote! { let #fname = self.#fname; }
            }
        })
        .collect();

    let derive_stmts: Vec<_> = derive_order
        .iter()
        .filter_map(|&i| {
            let (f, kind) = &field_kinds[i];
            let ident = &f.name;
            // When the field is `Option<Program<T>>` / `Option<...seeds...>`,
            // the downstream `client_meta_entries` match expects the local
            // binding to be `Option<Address>`, not a bare `Address`. Wrap
            // the derived value in `Some(...)` in that case so the match
            // arms type-check.
            match kind {
                FieldKind::Program(expr) => {
                    let init = if f.is_optional {
                        quote! { Some(#expr) }
                    } else {
                        quote! { #expr }
                    };
                    Some(quote! { let #ident = #init; })
                }
                FieldKind::Pda { seed_exprs, .. } => {
                    if f.is_optional {
                        Some(quote! {
                            let #ident = {
                                let (__addr, _) = anchor_lang_v2::find_program_address(
                                    &[#(#seed_exprs),*], &#accounts_program_id,
                                );
                                Some(__addr)
                            };
                        })
                    } else {
                        Some(quote! {
                            let (#ident, _) = anchor_lang_v2::find_program_address(
                                &[#(#seed_exprs),*], &#accounts_program_id,
                            );
                        })
                    }
                }
                _ => None,
            }
        })
        .collect();

    // Build AccountMeta entries in original field order, using bare idents.
    let client_meta_steps: Vec<_> = field_kinds
        .iter()
        .map(|(field, _kind)| {
            let writable = field.idl_writable;
            let signer_expr = client_meta_signer_expr(field);
            let field_ident = &field.name;
            if nested_client_accounts_type(&field.ty).is_some() {
                quote! {
                    __metas.extend(anchor_lang_v2::ToAccountMetas::to_account_metas(
                        #field_ident,
                        _is_signer,
                    ));
                }
            } else if field.is_optional {
                quote! {
                    match #field_ident {
                        Some(__addr) => __metas.push(anchor_lang_v2::AccountMeta {
                            pubkey: __addr,
                            is_writable: #writable,
                            is_signer: #signer_expr,
                        }),
                        None => __metas.push(anchor_lang_v2::AccountMeta {
                            pubkey: #accounts_program_id,
                            is_writable: false,
                            is_signer: false,
                        }),
                    }
                }
            } else {
                quote! {
                    __metas.push(anchor_lang_v2::AccountMeta {
                        pubkey: #field_ident,
                        is_writable: #writable,
                        is_signer: #signer_expr,
                    });
                }
            }
        })
        .collect();

    // PDA finder functions for seed-bearing fields.
    let pda_fns: Vec<TokenStream2> = fields
        .iter()
        .filter_map(|f| {
            let attrs = parse::parse_account_attrs(
                &named_fields
                    .named
                    .iter()
                    .find(|nf| nf.ident.as_ref() == Some(&f.name))?
                    .attrs,
            )
            .expect("parse_field already validated account attributes");
            let seeds_expr = attrs.seeds.as_ref()?;
            // Expression-form seeds (e.g. `seeds = Counter::seeds()`) don't
            // support client-side PDA helpers — skip.
            let seed_arr = match seeds_expr {
                syn::Expr::Array(arr) => arr,
                _ => return None,
            };

            let field_name = &f.name;
            let fn_name =
                syn::Ident::new(&format!("find_{}_address", field_name), field_name.span());

            let mut params: Vec<TokenStream2> = Vec::new();
            let mut seed_exprs: Vec<TokenStream2> = Vec::new();
            let mut seen_params = std::collections::HashSet::new();

            for seed_expr in &seed_arr.elems {
                let root = idl::receiver_root_ident_str(seed_expr);
                if let Some(ref root_name) = root {
                    if raw_field_names.contains(root_name) {
                        let param_ident =
                            syn::Ident::new(root_name, proc_macro2::Span::call_site());
                        if seen_params.insert(root_name.clone()) {
                            params.push(quote! { #param_ident: &anchor_lang_v2::Address });
                        }
                        seed_exprs.push(quote! { #param_ident.as_ref() });
                        continue;
                    }
                    if ix_arg_names.contains(root_name) {
                        let param_ident =
                            syn::Ident::new(root_name, proc_macro2::Span::call_site());
                        if seen_params.insert(root_name.clone()) {
                            params.push(quote! { #param_ident: &[u8] });
                        }
                        seed_exprs.push(quote! { #param_ident });
                        continue;
                    }
                }
                if let Some(bytes) = pda::seed_as_const_bytes(seed_expr) {
                    let byte_lits: Vec<_> = bytes.iter().map(|b| quote! { #b }).collect();
                    seed_exprs.push(quote! { &[#(#byte_lits),*] });
                } else {
                    return None;
                }
            }

            Some(quote! {
                pub fn #fn_name(#(#params),*) -> (anchor_lang_v2::Address, u8) {
                    anchor_lang_v2::find_program_address(
                        &[#(#seed_exprs),*],
                        &#accounts_program_id,
                    )
                }
            })
        })
        .collect();

    // Full struct: all fields as Address, user fills every one manually.
    let all_client_fields: Vec<_> = fields
        .iter()
        .map(|f| {
            let fname = &f.name;
            if let Some(nested_ty) = nested_client_accounts_type(&f.ty) {
                quote! { pub #fname: #nested_ty }
            } else if f.is_optional {
                quote! { pub #fname: Option<anchor_lang_v2::Address> }
            } else {
                quote! { pub #fname: anchor_lang_v2::Address }
            }
        })
        .collect();

    // Full struct to_account_metas: straightforward self.field access.
    let full_meta_steps: Vec<_> = fields
        .iter()
        .map(|field| {
            let writable = field.idl_writable;
            let signer_expr = client_meta_signer_expr(field);
            let field_ident = &field.name;
            if nested_client_accounts_type(&field.ty).is_some() {
                quote! {
                    __metas.extend(anchor_lang_v2::ToAccountMetas::to_account_metas(
                        &self.#field_ident,
                        _is_signer,
                    ));
                }
            } else if field.is_optional {
                quote! {
                    match self.#field_ident {
                        Some(__addr) => __metas.push(anchor_lang_v2::AccountMeta {
                            pubkey: __addr,
                            is_writable: #writable,
                            is_signer: #signer_expr,
                        }),
                        None => __metas.push(anchor_lang_v2::AccountMeta {
                            pubkey: #accounts_program_id,
                            is_writable: false,
                            is_signer: false,
                        }),
                    }
                }
            } else {
                quote! {
                    __metas.push(anchor_lang_v2::AccountMeta {
                        pubkey: self.#field_ident,
                        is_writable: #writable,
                        is_signer: #signer_expr,
                    });
                }
            }
        })
        .collect();

    let resolved_name = syn::Ident::new(&format!("{name}Resolved"), name.span());

    // --- CPI accounts struct (cross-program invocation, on-chain side) ---
    //
    // Emits a sibling `__cpi_accounts_<name>` module containing a struct of
    // `CpiHandle<'a>` / `CpiHandleMut<'a>` fields and a `ToCpiAccounts<'a>`
    // impl driven by each field's compile-time writable / signer flags.
    // `Nested<T>` fields hold T's generated CPI accounts struct and flatten through `ToCpiAccounts`,
    // matching the account ordering used by `TryAccounts`. Optional accounts
    // are emitted as `Option<CpiHandle<'a>>` or `Option<CpiHandleMut<'a>>`;
    // `None` emits the program-id sentinel account meta and omits the handle,
    // matching the callee's optional-account parser.
    // The `#[program]` macro re-exports the resulting type under
    // `cpi::accounts::<name>` and synthesizes the per-instruction wrapper
    // functions.
    let cpi_mod_name = syn::Ident::new(
        &format!("__cpi_accounts_{}", name.to_string().to_lowercase()),
        name.span(),
    );
    let cpi_accounts_mod = {
        let cpi_field_decls: Vec<_> = fields
            .iter()
            .map(|f| {
                let n = &f.name;
                if parse::is_nested_type(&f.ty) {
                    let inner_ty = parse::extract_nested_inner_type(&f.ty).expect(
                        "is_nested_type was true but extract_nested_inner_type returned None",
                    );
                    let inner_name = parse::field_ty_str(inner_ty);
                    let inner_ident = syn::Ident::new(&inner_name, n.span());
                    let inner_mod = syn::Ident::new(
                        &format!("__cpi_accounts_{}", inner_name.to_lowercase()),
                        n.span(),
                    );
                    quote! {
                        #[nested]
                        pub #n: super::#inner_mod::#inner_ident<'a>
                    }
                } else if f.is_optional && f.idl_writable {
                    let signer_expr = cpi_meta_signer_expr(f);
                    quote! {
                        #[signer(#signer_expr)]
                        pub #n: ::core::option::Option<anchor_lang_v2::CpiHandleMut<'a>>
                    }
                } else if f.is_optional {
                    let signer_expr = cpi_meta_signer_expr(f);
                    quote! {
                        #[signer(#signer_expr)]
                        pub #n: ::core::option::Option<anchor_lang_v2::CpiHandle<'a>>
                    }
                } else if f.idl_writable {
                    let signer_expr = cpi_meta_signer_expr(f);
                    quote! {
                        #[signer(#signer_expr)]
                        pub #n: anchor_lang_v2::CpiHandleMut<'a>
                    }
                } else {
                    let signer_expr = cpi_meta_signer_expr(f);
                    quote! {
                        #[signer(#signer_expr)]
                        pub #n: anchor_lang_v2::CpiHandle<'a>
                    }
                }
            })
            .collect();
        // An empty Accounts struct would otherwise emit `pub struct Foo<'a> {}`,
        // which fails E0392 because nothing on `Self` references `'a`. Anchor
        // the lifetime via a `PhantomData<&'a ()>` field — kept hidden — and
        // expose a no-arg `new()` / `Default` so callers don't need to spell
        // out the marker. Non-empty structs already bind `'a` through their
        // CPI handle fields and skip the extra field entirely.
        let (extra_field, ctor_impl) = if fields.is_empty() {
            (
                quote! {
                    #[doc(hidden)]
                    pub _phantom: ::core::marker::PhantomData<&'a ()>,
                },
                quote! {
                    impl<'a> #name<'a> {
                        #[inline]
                        pub const fn new() -> Self {
                            Self { _phantom: ::core::marker::PhantomData }
                        }
                    }
                    impl<'a> ::core::default::Default for #name<'a> {
                        #[inline]
                        fn default() -> Self { Self::new() }
                    }
                },
            )
        } else {
            (quote! {}, quote! {})
        };
        quote! {
            pub mod #cpi_mod_name {
                extern crate alloc;
                use super::*;
                #[derive(anchor_lang_v2::ToCpiAccounts)]
                #[accounts_program_id(#accounts_program_id)]
                pub struct #name<'a> {
                    #(#cpi_field_decls,)*
                    #extra_field
                }
                #ctor_impl
            }
        }
    };

    let resolved_struct = quote! {
        pub struct #resolved_name {
            #(#client_fields,)*
        }
        impl anchor_lang_v2::ToAccountMetas for #resolved_name {
            fn to_account_metas(&self, _is_signer: Option<bool>) -> alloc::vec::Vec<anchor_lang_v2::AccountMeta> {
                #(#required_locals)*
                #(#derive_stmts)*
                let mut __metas = alloc::vec::Vec::new();
                #(#client_meta_steps)*
                __metas
            }
        }
    };

    quote! {
        pub mod #client_mod_name {
            extern crate alloc;
            use super::*;
            pub struct #name {
                #(#all_client_fields,)*
            }
            impl anchor_lang_v2::ToAccountMetas for #name {
                fn to_account_metas(&self, _is_signer: Option<bool>) -> alloc::vec::Vec<anchor_lang_v2::AccountMeta> {
                    let mut __metas = alloc::vec::Vec::new();
                    #(#full_meta_steps)*
                    __metas
                }
            }
            #resolved_struct
            impl #name {
                #(#pda_fns)*
            }
        }

        #cpi_accounts_mod

        #bumps_def

        impl anchor_lang_v2::Bumps for #name {
            type Bumps = #bumps_name;
        }

        impl anchor_lang_v2::TryAccounts for #name {
            const HEADER_SIZE: usize = #header_size_expr;
            const MUT_MASK: [u64; 4] = #mut_mask_expr;
            const HAS_DYNAMIC_MUT_MASK: bool = #has_dynamic_mut_mask_expr;

            #[inline(always)]
            fn active_mut_mask(&self) -> [u64; 4] {
                #active_mut_mask_body
            }

            #ix_args_assoc

            #[inline]
            fn try_accounts<'ix>(
                __program_id: &anchor_lang_v2::Address,
                __views: &[anchor_lang_v2::AccountView],
                __duplicates: ::core::option::Option<&anchor_lang_v2::AccountBitvec>,
                __base_offset: usize,
                __ix_data: &'ix [u8],
            ) -> anchor_lang_v2::Result<(Self, #bumps_name, Self::IxArgs<'ix>)> {
                #ix_deser
                #bumps_init
                #(#loads)*
                #(#deferred_loads)*
                #dup_check_block
                #(#constraints)*
                Ok((Self { #(#field_names),* }, __bumps, #ix_args_return))
            }

            //
            #[inline(always)]
            fn exit_accounts(&mut self) -> anchor_lang_v2::Result<()> {
                #(#exits)*
                Ok(())
            }
        }

        #[cfg(feature = "idl-build")]
        #[doc(hidden)]
        impl #name {
            // Runtime-assembled accounts JSON: reads per-wrapper signer /
            // address trait consts, splices in compile-time flags.
            #idl_accounts_fn

            /// **Opaque / unstable.** Walks each account field's
            /// [`IdlAccountType::__register_idl_deps`] so nested
            /// user-defined types (plain `#[derive(IdlType)]` structs,
            /// `#[account]` data types, `#[event]` payload types, etc.)
            /// transitively register into the IDL's program-level
            /// `accounts[]` and `types[]` arrays.
            #[doc(hidden)]
            pub fn __idl_register_deps(
                accounts: &mut anchor_lang_v2::__alloc::vec::Vec<&'static str>,
                types: &mut anchor_lang_v2::__alloc::vec::Vec<&'static str>,
            ) {
                #(
                    <#idl_field_tys as anchor_lang_v2::IdlAccountType>::__register_idl_deps(accounts, types);
                )*
            }
        }
    }
}

// ---------------------------------------------------------------------------
// #[account]
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn account(attr: TokenStream, item: TokenStream) -> TokenStream {
    let is_borsh = match parse_account_mode(attr) {
        Ok(b) => b,
        Err(err) => return err.to_compile_error().into(),
    };
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();
    let vis = &input.vis;
    let attrs = &input.attrs;
    let fields = match &input.data {
        Data::Struct(data) => &data.fields,
        _ => {
            return syn::Error::new(name.span(), "`#[account]` only supports structs")
                .to_compile_error()
                .into()
        }
    };

    use sha2::Digest;
    let hash = sha2::Sha256::digest(format!("account:{name_str}").as_bytes());
    let disc_bytes = &hash[..8];
    let disc_literals: Vec<_> = disc_bytes.iter().map(|b| quote! { #b }).collect();

    let struct_docs = idl::extract_doc_lines(attrs);
    // `#[account]` has two modes: default zero-copy (Pod + repr(C)) and opt-in
    // borsh (`#[account(borsh)]`). The borsh mode is implemented on top of
    // wincode + `BORSH_CONFIG`, which produces byte-identical output to a
    // real borsh impl while skipping the borsh crate's slower encode/decode
    // path. The mode propagates into the IDL type definition's
    // `serialization` / `repr` fields (spec:180-216) so downstream codegen
    // knows which wire format to use.
    let type_kind = if is_borsh {
        idl::TypeKind::Borsh
    } else {
        idl::TypeKind::BytemuckRepr
    };
    let idl_type_strings = if let Fields::Named(named) = fields {
        idl::build_type_strings(&name_str, disc_bytes, &struct_docs, &named.named, type_kind)
    } else {
        idl::build_type_strings(
            &name_str,
            disc_bytes,
            &struct_docs,
            &syn::punctuated::Punctuated::new(),
            type_kind,
        )
    };
    let idl_account_entry = match idl_type_strings.account_entry {
        Some(s) => quote! { Some(#s) },
        None => quote! { None },
    };
    let idl_type_def = idl_type_strings.type_def;

    // Named-field types for the transitive IDL dep walk. `__register_idl_deps`
    // fans out through each field type so nested user structs land in the
    // IDL's `types[]` array even when the user never wrote `#[account]` on
    // them (e.g. a plain `#[derive(IdlType)] struct Inner` embedded in this
    // account's body).
    let idl_field_tys: Vec<&Type> = match fields {
        Fields::Named(named) => named.named.iter().map(|f| &f.ty).collect(),
        Fields::Unnamed(unnamed) => unnamed.unnamed.iter().map(|f| &f.ty).collect(),
        Fields::Unit => Vec::new(),
    };

    // Client-side `AccountDeserialize` impl. Mode-dependent: borsh accounts
    // run wincode (with `BORSH_CONFIG`, borsh-wire-compatible) over the
    // post-disc tail; pod accounts do a `bytemuck::pod_read_unaligned` on a
    // sized slice. Both share the disc-check shape so a wrong-type fetch
    // surfaces as `InvalidAccountData`.
    let account_deserialize_impl = if is_borsh {
        quote! {
            impl anchor_lang_v2::AccountDeserialize for #name {
                fn try_deserialize(
                    buf: &mut &[u8],
                ) -> ::core::result::Result<Self, anchor_lang_v2::Error> {
                    use anchor_lang_v2::Discriminator as _;
                    if buf.len() < <Self as anchor_lang_v2::Discriminator>::DISCRIMINATOR.len() {
                        return Err(anchor_lang_v2::Error::AccountDataTooSmall);
                    }
                    let (disc, rest) = buf.split_at(<Self as anchor_lang_v2::Discriminator>::DISCRIMINATOR.len());
                    if disc != <Self as anchor_lang_v2::Discriminator>::DISCRIMINATOR {
                        return Err(anchor_lang_v2::Error::InvalidAccountData);
                    }
                    *buf = rest;
                    Self::try_deserialize_unchecked(buf)
                }
                fn try_deserialize_unchecked(
                    buf: &mut &[u8],
                ) -> ::core::result::Result<Self, anchor_lang_v2::Error> {
                    // Use `SchemaRead::get` (reads from a `&mut &[u8]` Reader
                    // and advances it) rather than `config::deserialize`,
                    // which takes the input by value and would leave `*buf`
                    // unchanged — `AccountDeserialize`'s cursor contract
                    // requires the post-disc slice to advance through the
                    // payload so chained deserializations stay aligned.
                    <Self as anchor_lang_v2::wincode::SchemaRead<
                        '_,
                        anchor_lang_v2::BorshConfig,
                    >>::get(buf)
                        .map_err(|_| anchor_lang_v2::Error::InvalidAccountData)
                }
            }
        }
    } else {
        quote! {
            impl anchor_lang_v2::AccountDeserialize for #name {
                fn try_deserialize(
                    buf: &mut &[u8],
                ) -> ::core::result::Result<Self, anchor_lang_v2::Error> {
                    use anchor_lang_v2::Discriminator as _;
                    if buf.len() < <Self as anchor_lang_v2::Discriminator>::DISCRIMINATOR.len() {
                        return Err(anchor_lang_v2::Error::AccountDataTooSmall);
                    }
                    let (disc, rest) = buf.split_at(<Self as anchor_lang_v2::Discriminator>::DISCRIMINATOR.len());
                    if disc != <Self as anchor_lang_v2::Discriminator>::DISCRIMINATOR {
                        return Err(anchor_lang_v2::Error::InvalidAccountData);
                    }
                    *buf = rest;
                    Self::try_deserialize_unchecked(buf)
                }
                fn try_deserialize_unchecked(
                    buf: &mut &[u8],
                ) -> ::core::result::Result<Self, anchor_lang_v2::Error> {
                    let n = ::core::mem::size_of::<Self>();
                    if buf.len() < n {
                        return Err(anchor_lang_v2::Error::AccountDataTooSmall);
                    }
                    let value: Self = anchor_lang_v2::bytemuck::pod_read_unaligned(&buf[..n]);
                    *buf = &buf[n..];
                    Ok(value)
                }
            }
        }
    };

    let (struct_attrs, pod_impls) = if is_borsh {
        (
            quote! { #[derive(anchor_lang_v2::wincode::SchemaWrite, anchor_lang_v2::wincode::SchemaRead)] },
            quote! {},
        )
    } else {
        let field_types: Vec<_> = if let Fields::Named(named) = fields {
            named.named.iter().map(|f| &f.ty).collect()
        } else {
            vec![]
        };

        // Targeted diagnostics for common non-Pod field types. Emits a
        // `compile_error!` with a concrete suggestion instead of letting the
        // user hit an opaque `the trait bound Vec<u8>: Pod is not satisfied`.
        // Intentionally avoids recommending `#[account(borsh)]` — borsh is a
        // per-instruction serialization cost, rarely what the user actually
        // wants. The fix is almost always a Pod-compatible alternative.
        let field_diagnostics: Vec<proc_macro2::TokenStream> = if let Fields::Named(named) = fields
        {
            named
                .named
                .iter()
                .filter_map(|f| {
                    let fname = f.ident.as_ref()?.to_string();
                    let msg = diagnose_non_pod_field(&f.ty, &fname, &name_str)?;
                    let span = f.ty.span();
                    Some(quote::quote_spanned!(span=> const _: () = { compile_error!(#msg); };))
                })
                .collect()
        } else {
            Vec::new()
        };

        (
            quote! { #[derive(Clone, Copy)] #[repr(C)] },
            quote! {
                #(#field_diagnostics)*

                const _: fn() = || {
                    fn assert_pod<T: anchor_lang_v2::bytemuck::Pod>() {}
                    #( assert_pod::<#field_types>(); )*
                };
                // Verify no padding: struct size must equal sum of field sizes.
                // repr(C) inserts padding between fields with different alignments
                // (e.g. u8 followed by u64 → 7 bytes of padding). Padding bytes
                // are uninitialized, violating Pod's all-bytes-initialized requirement.
                const _: () = assert!(
                    core::mem::size_of::<#name>() == 0 #(+ core::mem::size_of::<#field_types>())*,
                    "account struct has padding bytes — reorder fields from largest to smallest alignment to eliminate padding (e.g. u64 before u32 before u8)"
                );
                unsafe impl anchor_lang_v2::bytemuck::Pod for #name {}
                unsafe impl anchor_lang_v2::bytemuck::Zeroable for #name {}
            },
        )
    };

    TokenStream::from(quote! {
        #(#attrs)*
        #struct_attrs
        #vis struct #name #fields

        #pod_impls

        impl anchor_lang_v2::Owner for #name {
            const OWNER: anchor_lang_v2::Address = crate::ID;
        }
        impl anchor_lang_v2::Discriminator for #name {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_literals),*];
        }
        #account_deserialize_impl
        #[cfg(feature = "idl-build")]
        #[doc(hidden)]
        impl anchor_lang_v2::IdlAccountType for #name {
            const __IDL_ACCOUNT_ENTRY: Option<&'static str> = #idl_account_entry;
            const __IDL_TYPE_DEF: Option<&'static str> = Some(#idl_type_def);
            fn __register_idl_deps(
                accounts: &mut ::anchor_lang_v2::__alloc::vec::Vec<&'static str>,
                types: &mut ::anchor_lang_v2::__alloc::vec::Vec<&'static str>,
            ) {
                if let Some(a) = <Self as anchor_lang_v2::IdlAccountType>::__IDL_ACCOUNT_ENTRY {
                    accounts.push(a);
                }
                if let Some(t) = <Self as anchor_lang_v2::IdlAccountType>::__IDL_TYPE_DEF {
                    types.push(t);
                }
                #(
                    <#idl_field_tys as anchor_lang_v2::IdlAccountType>::__register_idl_deps(accounts, types);
                )*
            }
        }
    })
}

// ---------------------------------------------------------------------------
// #[derive(IdlType)]
// ---------------------------------------------------------------------------

/// Register a plain (non-`#[account]`, non-`#[event]`) struct or enum in
/// the IDL's `types[]` array.
///
/// V1 had `#[derive(IdlBuild)]` for the same purpose. Without this, a nested
/// user-defined struct or enum referenced by an `#[event]` / `#[account]`
/// field appears in the outer type's JSON as `{"defined":{"name":"Inner"}}`
/// but has no corresponding entry in `types[]` — TS clients then fail to
/// decode the nested field.
///
/// Unlike `#[account]`, this derive carries **none** of the account-kind
/// baggage: no discriminator, no `Owner`, no `Discriminator` trait, no
/// forced `#[repr(C)]`, no Pod/Zeroable derives. It only emits the
/// `IdlAccountType` impl so the type gets picked up by the transitive
/// walker.
///
/// Unions are rejected — the IDL spec has no tagged-union shape for them.
///
/// # Examples
///
/// ```ignore
/// // Plain struct dep.
/// #[repr(C)]
/// #[derive(Clone, Copy, Pod, Zeroable, IdlType)]
/// pub struct Inner { pub a: u64, pub b: u64 }
///
/// #[event]
/// pub struct NestedEvent {
///     pub outer_id: u64,
///     pub inner: Inner, // <- pulls `Inner` into the IDL's types[]
/// }
///
/// // Enum dep — unit, tuple, and struct variants all supported.
/// #[derive(IdlType)]
/// pub enum Kind {
///     Spot,
///     Futures(u64),
///     Margin { leverage: u8, symbol: [u8; 8] },
/// }
/// ```
#[proc_macro_derive(IdlType)]
pub fn derive_idl_type(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    let name = &input.ident;
    let name_str = name.to_string();

    let docs = idl::extract_doc_lines(&input.attrs);
    // `IdlType` items have no discriminator — they're just plain type
    // definitions referenced from other structs. `build_*_type_json` still
    // expect a discriminator for shape uniformity with `#[account]` /
    // `#[event]`; we pass an empty slice and strip the discriminator field
    // downstream when splitting accounts vs types entries. The spec's
    // `IdlTypeDef` doesn't carry `discriminator`, so the program-level
    // assembly already elides it when reconstructing types entries.
    let empty_disc: [u8; 0] = [];
    // Default to `Borsh` serialization (no `serialization` / `repr` fields).
    // `IdlType` is layout-agnostic — users opt into Pod separately via
    // their own `bytemuck::Pod` derive if they need zero-copy. Forcing a
    // `"bytemuck"` tag here would lie in the IDL for non-Pod types.
    let (idl_type_strings, field_tys) = match &input.data {
        Data::Struct(data) => {
            let strings = match &data.fields {
                Fields::Named(named) => idl::build_type_strings(
                    &name_str,
                    &empty_disc,
                    &docs,
                    &named.named,
                    idl::TypeKind::Borsh,
                ),
                // Unnamed (tuple) and unit structs are emitted with an empty
                // named-fields array. The spec doesn't carry a tuple-struct
                // distinction at the top level of `IdlTypeDef` — only enum
                // variants get `IdlDefinedFields::Tuple` — so tuple structs
                // fall through to struct-with-empty-fields. Users needing
                // tuple shapes should prefer named fields.
                _ => idl::build_type_strings(
                    &name_str,
                    &empty_disc,
                    &docs,
                    &syn::punctuated::Punctuated::new(),
                    idl::TypeKind::Borsh,
                ),
            };
            // Walk fields for the transitive dep registration. Tuple structs
            // still contribute their field types; unit structs have no inner
            // fields to recurse into.
            let field_tys: Vec<&Type> = match &data.fields {
                Fields::Named(named) => named.named.iter().map(|f| &f.ty).collect(),
                Fields::Unnamed(unnamed) => unnamed.unnamed.iter().map(|f| &f.ty).collect(),
                Fields::Unit => Vec::new(),
            };
            (strings, field_tys)
        }
        Data::Enum(data) => {
            let strings = idl::build_enum_type_strings(
                &name_str,
                &empty_disc,
                &docs,
                &data.variants,
                idl::TypeKind::Borsh,
            );
            // Every variant's fields contribute dependent types for the
            // transitive walker — a `Foo::Bar(Inner)` variant needs to pull
            // `Inner` into `types[]` just like a struct field would.
            let field_tys: Vec<&Type> = data
                .variants
                .iter()
                .flat_map(|v| match &v.fields {
                    Fields::Named(named) => named.named.iter().map(|f| &f.ty).collect::<Vec<_>>(),
                    Fields::Unnamed(unnamed) => unnamed.unnamed.iter().map(|f| &f.ty).collect(),
                    Fields::Unit => Vec::new(),
                })
                .collect();
            (strings, field_tys)
        }
        Data::Union(_) => {
            return syn::Error::new(
                name.span(),
                "`#[derive(IdlType)]` does not support unions — use a struct or enum instead",
            )
            .to_compile_error()
            .into();
        }
    };
    // Plain `IdlType` types never carry a discriminator and never appear in
    // `accounts[]`; only `__IDL_TYPE_DEF` is `Some`. The builder enforces
    // this via the empty-disc → `account_entry: None` rule.
    debug_assert!(idl_type_strings.account_entry.is_none());
    let idl_type_def = idl_type_strings.type_def;

    // Thread any generic / lifetime params from the input type through
    // the impl so `#[derive(IdlType)] struct Foo<'a>` lowers to
    // `impl<'a> IdlAccountType for Foo<'a>`. Without this, borrowed ix-arg
    // structs (e.g. `MixedArgs<'a> { values: &'a [u64] }`) wouldn't compile
    // with the derive.
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    TokenStream::from(quote! {
        #[cfg(feature = "idl-build")]
        #[doc(hidden)]
        impl #impl_generics anchor_lang_v2::IdlAccountType for #name #ty_generics #where_clause {
            const __IDL_TYPE_DEF: Option<&'static str> = Some(#idl_type_def);
            fn __register_idl_deps(
                accounts: &mut ::anchor_lang_v2::__alloc::vec::Vec<&'static str>,
                types: &mut ::anchor_lang_v2::__alloc::vec::Vec<&'static str>,
            ) {
                // `IdlType` plain types never push to `accounts[]` —
                // `__IDL_ACCOUNT_ENTRY` defaults to `None`. We only
                // contribute to `types[]` and recurse for transitive deps.
                if let Some(t) = <Self as anchor_lang_v2::IdlAccountType>::__IDL_TYPE_DEF {
                    types.push(t);
                }
                #(
                    <#field_tys as anchor_lang_v2::IdlAccountType>::__register_idl_deps(accounts, types);
                )*
            }
        }
    })
}

/// Syntactic diagnosis for common non-Pod field types on `#[account]` structs.
/// Produces a targeted, actionable error message when we can recognize the
/// shape of the offending type (Vec, String, Option, Box, bool, etc.). Falls
/// through to `None` for types we can't identify by name — the surrounding
/// `assert_pod::<T>` check in the macro output catches those generically.
///
/// Intentionally never suggests `#[account(borsh)]`: borsh accounts incur a
/// per-instruction (de)serialization cost that's rarely what a user actually
/// wants. The fix for "this field isn't Pod" is almost always a Pod-
/// compatible alternative (fixed-size array, sentinel value, `PodBool`, a
/// `Slab<H, T>` tail, etc.).
fn diagnose_non_pod_field(ty: &Type, field_name: &str, struct_name: &str) -> Option<String> {
    let Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;
    let ident = seg.ident.to_string();
    match ident.as_str() {
        "Vec" => Some(format!(
            "field `{field_name}` on `#[account] struct {struct_name}` uses `Vec`, which \
             allocates on the heap and isn't Pod. Zero-copy accounts need fixed-size fields. Use \
             `[T; N]` for a bounded array, or restructure `{struct_name}` as `Slab<Header, T>` if \
             you need a dynamic tail."
        )),
        "String" => Some(format!(
            "field `{field_name}` on `#[account] struct {struct_name}` uses `String`, which \
             allocates on the heap and isn't Pod. Use a fixed-size `[u8; N]` buffer to store \
             string data in a zero-copy account."
        )),
        "Option" => Some(format!(
            "field `{field_name}` on `#[account] struct {struct_name}` uses `Option`, which \
             carries a discriminant byte that breaks the zero-copy layout contract. Use a \
             sentinel value (e.g. an all-zero `[u8; 32]` for \"no address\") or a `PodBool` flag \
             stored alongside the value."
        )),
        "Box" | "Rc" | "Arc" => Some(format!(
            "field `{field_name}` on `#[account] struct {struct_name}` uses `{ident}`, which \
             heap-allocates and isn't valid in a zero-copy account. Store the inner type directly."
        )),
        "bool" => Some(format!(
            "field `{field_name}` on `#[account] struct {struct_name}` uses `bool`. `bytemuck` \
             disallows `bool` as Pod because only `0x00` and `0x01` are valid bit-patterns (any \
             other byte read as `bool` is UB). Use `anchor_lang_v2::PodBool` instead."
        )),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// #[program]
// ---------------------------------------------------------------------------

#[proc_macro_attribute]
pub fn program(attr: TokenStream, item: TokenStream) -> TokenStream {
    let config = match parse_program_config(attr) {
        Ok(config) => config,
        Err(err) => return TokenStream::from(err.to_compile_error()),
    };
    let module = parse_macro_input!(item as ItemMod);
    TokenStream::from(impl_program(&module, &config))
}

#[proc_macro]
pub fn declare_program(input: TokenStream) -> TokenStream {
    let name = parse_macro_input!(input as Ident);
    match impl_declare_program(&name) {
        Ok(tokens) => TokenStream::from(tokens),
        Err(err) => TokenStream::from(err.to_compile_error()),
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ProgramMode {
    Executable,
    Interface,
}

struct ProgramConfig {
    mode: ProgramMode,
    program_id: Expr,
}

#[derive(Clone)]
struct DiscrimAttr {
    bytes: Vec<u8>,
    span: proc_macro2::Span,
}

fn parse_program_config(attr: TokenStream) -> syn::Result<ProgramConfig> {
    parse_program_config_tokens(TokenStream2::from(attr))
}

fn parse_program_config_tokens(attr: TokenStream2) -> syn::Result<ProgramConfig> {
    let mut mode = ProgramMode::Executable;
    let mut program_id = None;
    let mut program_id_span = None;

    if attr.is_empty() {
        return Ok(ProgramConfig {
            mode,
            program_id: syn::parse_quote!(crate::ID),
        });
    }

    let parser = syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated;
    for meta in parser.parse2(attr)? {
        match meta {
            syn::Meta::Path(path) if path.is_ident("interface") => {
                mode = ProgramMode::Interface;
            }
            syn::Meta::NameValue(nv) if nv.path.is_ident("program_id") => {
                if program_id.is_some() {
                    return Err(syn::Error::new(nv.path.span(), "duplicate `program_id`"));
                }
                program_id_span = Some(nv.path.span());
                program_id = Some(nv.value);
            }
            other => {
                return Err(syn::Error::new(
                    other.span(),
                    "unsupported `#[program]` argument; expected `interface` or `program_id = <expr>`",
                ));
            }
        }
    }

    if mode == ProgramMode::Executable {
        if let Some(span) = program_id_span {
            return Err(syn::Error::new(
                span,
                "`program_id` is only supported with `#[program(interface, ...)]`",
            ));
        }
    }

    Ok(ProgramConfig {
        mode,
        program_id: program_id.unwrap_or_else(|| syn::parse_quote!(crate::ID)),
    })
}

fn impl_declare_program(name: &Ident) -> syn::Result<TokenStream2> {
    let idl = read_declare_program_idl(name)?;
    gen_declared_program(name, &idl)
}

fn read_declare_program_idl(name: &Ident) -> syn::Result<serde_json::Value> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR").map_err(|err| {
        syn::Error::new(
            name.span(),
            format!("failed to read `CARGO_MANIFEST_DIR` for `declare_program!`: {err}"),
        )
    })?;
    let manifest_dir = std::path::PathBuf::from(manifest_dir);
    let idl_dir = manifest_dir
        .ancestors()
        .find_map(|ancestor| {
            let candidate = ancestor.join("idls");
            candidate.exists().then_some(candidate)
        })
        .ok_or_else(|| syn::Error::new(name.span(), "`idls` directory not found"))?;
    let idl_path = idl_dir.join(name.to_string()).with_extension("json");
    let idl = std::fs::read(&idl_path).map_err(|err| {
        syn::Error::new(
            name.span(),
            format!("failed to read IDL `{}`: {err}", idl_path.display()),
        )
    })?;
    let idl = anchor_lang_idl::convert::convert_idl(&idl).map_err(|err| {
        syn::Error::new(
            name.span(),
            format!("failed to parse IDL `{}`: {err}", idl_path.display()),
        )
    })?;
    serde_json::to_value(idl).map_err(|err| {
        syn::Error::new(
            name.span(),
            format!(
                "failed to normalize IDL `{}` after parsing: {err}",
                idl_path.display()
            ),
        )
    })
}

fn gen_declared_program(name: &Ident, idl: &serde_json::Value) -> syn::Result<TokenStream2> {
    let address = idl
        .get("address")
        .or_else(|| idl.pointer("/metadata/address"))
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| syn::Error::new(name.span(), "IDL is missing program address"))?;
    let address_lit = syn::LitStr::new(address, name.span());
    let instructions = idl
        .get("instructions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| syn::Error::new(name.span(), "IDL is missing instructions array"))?;
    validate_declare_program_discriminators(idl, name.span())?;

    let types = gen_declare_program_types(idl)?;
    let type_idents = gen_declare_program_type_idents(idl)?;
    let type_reexports = type_idents
        .iter()
        .map(|ident| quote! { pub use super::#ident; });
    let constants = gen_declare_program_constants(idl)?;
    let events = gen_declare_program_events(idl)?;
    let errors = gen_declare_program_errors(idl, name.span())?;
    let mut account_groups = std::collections::BTreeMap::<String, Vec<DeclareAccountField>>::new();
    let mut handlers = Vec::new();
    for ix in instructions {
        let ix_name = json_str(ix, "name", name.span())?;
        let ix_ident = Ident::new(&to_snake_case(ix_name), name.span());
        let accounts_name = to_type_name(ix_name);
        let accounts_ident = Ident::new(&accounts_name, name.span());
        let accounts = ix
            .get("accounts")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                syn::Error::new(
                    name.span(),
                    format!("instruction `{ix_name}` is missing accounts array"),
                )
            })?;
        collect_declare_account_group(&accounts_name, accounts, &mut account_groups, name.span())?;

        let discrim = ix
            .get("discriminator")
            .and_then(serde_json::Value::as_array)
            .map(|values| parse_discriminator_array(values, name.span()))
            .transpose()?
            .unwrap_or_else(|| default_instruction_discriminator(&to_snake_case(ix_name)));
        let discrim_tokens: Vec<_> = discrim.iter().map(|b| quote! { #b }).collect();

        let args = ix
            .get("args")
            .and_then(serde_json::Value::as_array)
            .ok_or_else(|| {
                syn::Error::new(
                    name.span(),
                    format!("instruction `{ix_name}` is missing args array"),
                )
            })?;
        let mut arg_decls = Vec::new();
        let mut arg_uses = Vec::new();
        for arg in args {
            let arg_name = json_str(arg, "name", name.span())?;
            let arg_ident = Ident::new(&to_snake_case(arg_name), name.span());
            let ty_value = arg.get("type").ok_or_else(|| {
                syn::Error::new(
                    name.span(),
                    format!("argument `{arg_name}` is missing type"),
                )
            })?;
            let ty = declare_idl_type_to_tokens(ty_value, name.span())?;
            arg_decls.push(quote! { #arg_ident: #ty });
            arg_uses.push(quote! { let _ = #arg_ident; });
        }
        let return_ty = ix
            .get("returns")
            .map(|ty| declare_idl_type_to_tokens(ty, name.span()))
            .transpose()?
            .unwrap_or_else(|| quote! { () });

        handlers.push(quote! {
            #[discrim = [#(#discrim_tokens),*]]
            pub fn #ix_ident(_ctx: &mut anchor_lang_v2::Context<#accounts_ident>, #(#arg_decls),*) -> anchor_lang_v2::Result<#return_ty> {
                #(#arg_uses)*
                unreachable!()
            }
        });
    }

    let mut account_structs = Vec::new();
    for (group_name, fields) in account_groups {
        let group_ident = Ident::new(&group_name, name.span());
        let field_tokens = fields.into_iter().map(|field| field.to_tokens());
        account_structs.push(quote! {
            #[derive(anchor_lang_v2::Accounts)]
            #[accounts_program_id(ID)]
            pub struct #group_ident {
                #(#field_tokens)*
            }
        });
    }

    let marker_name = Ident::new(&to_type_name(&name.to_string()), name.span());
    Ok(quote! {
        pub mod #name {
            use super::*;

            pub const ID: anchor_lang_v2::Address =
                anchor_lang_v2::address!(#address_lit);

            pub mod program {
                pub struct #marker_name;

                impl anchor_lang_v2::Id for #marker_name {
                    fn id() -> anchor_lang_v2::Address {
                        super::ID
                    }
                }
            }

            #(#types)*
            pub mod types {
                #(#type_reexports)*
            }
            pub mod constants {
                #(#constants)*
            }
            #events
            #errors
            #(#account_structs)*

            #[anchor_lang_v2::program(interface, program_id = ID)]
            pub mod __program {
                use super::*;
                #(#handlers)*
            }
        }
    })
}

fn gen_declare_program_errors(
    idl: &serde_json::Value,
    span: proc_macro2::Span,
) -> syn::Result<TokenStream2> {
    let Some(errors) = idl.get("errors").and_then(serde_json::Value::as_array) else {
        return Ok(quote! {
            pub mod error {
                use super::*;
            }
        });
    };

    if errors.is_empty() {
        return Ok(quote! {
            pub mod error {
                use super::*;
            }
        });
    }

    let program_name = idl
        .pointer("/metadata/name")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("DeclaredProgram");
    let enum_ident = Ident::new(&format!("{}Error", to_type_name(program_name)), span);
    let mut variants = Vec::new();
    for error in errors {
        let name = json_str(error, "name", span)?;
        let ident = Ident::new(&to_type_name(name), span);
        let code = error
            .get("code")
            .and_then(serde_json::Value::as_u64)
            .filter(|code| *code <= u32::MAX as u64)
            .ok_or_else(|| {
                syn::Error::new(
                    span,
                    format!("IDL error `{name}` is missing a u32 code in `{error}`"),
                )
            })? as u32;
        let msg = error
            .get("msg")
            .and_then(serde_json::Value::as_str)
            .map(|msg| {
                let msg_lit = syn::LitStr::new(msg, span);
                quote! { #[msg(#msg_lit)] }
            });
        variants.push(quote! {
            #msg
            #ident = #code,
        });
    }

    Ok(quote! {
        pub mod error {
            use super::*;

            #[anchor_lang_v2::error_code(offset = 0)]
            pub enum #enum_ident {
                #(#variants)*
            }
        }
    })
}

fn validate_declare_program_discriminators(
    idl: &serde_json::Value,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    let instructions = idl
        .get("instructions")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .map(|ix| {
            let name = json_str(ix, "name", span)?;
            let discriminator = ix
                .get("discriminator")
                .and_then(serde_json::Value::as_array)
                .map(|values| parse_discriminator_array(values, span))
                .transpose()?
                .unwrap_or_else(|| default_instruction_discriminator(&to_snake_case(name)));
            Ok((name.to_string(), discriminator))
        })
        .collect::<syn::Result<Vec<_>>>()?;
    validate_discriminator_prefixes("instructions", &instructions, span)?;

    for section in ["accounts", "events"] {
        let discriminators = idl
            .get(section)
            .and_then(serde_json::Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|item| {
                let name = match json_str(item, "name", span) {
                    Ok(name) => name,
                    Err(err) => return Some(Err(err)),
                };
                let discriminator = item.get("discriminator")?.as_array()?;
                Some(
                    parse_discriminator_array(discriminator, span)
                        .map(|discriminator| (name.to_string(), discriminator)),
                )
            })
            .collect::<syn::Result<Vec<_>>>()?;
        validate_discriminator_prefixes(section, &discriminators, span)?;
    }

    Ok(())
}

fn validate_discriminator_prefixes(
    section: &str,
    discriminators: &[(String, Vec<u8>)],
    span: proc_macro2::Span,
) -> syn::Result<()> {
    for (outer_name, outer_disc) in discriminators {
        for (inner_name, inner_disc) in discriminators {
            if outer_name != inner_name && outer_disc.starts_with(inner_disc) {
                return Err(syn::Error::new(
                    span,
                    format!(
                        "Ambiguous discriminators for {section}: `{inner_name}` discriminator {} \
                         is a prefix of `{outer_name}` discriminator {}",
                        format_discriminator_bytes(inner_disc),
                        format_discriminator_bytes(outer_disc),
                    ),
                ));
            }
        }
    }
    Ok(())
}

fn format_discriminator_bytes(discriminator: &[u8]) -> String {
    let bytes = discriminator
        .iter()
        .map(u8::to_string)
        .collect::<Vec<_>>()
        .join(", ");
    format!("[{bytes}]")
}

struct DeclareAccountField {
    name: Ident,
    ty: TokenStream2,
    attrs: TokenStream2,
}

impl DeclareAccountField {
    fn to_tokens(self) -> TokenStream2 {
        let name = self.name;
        let ty = self.ty;
        let attrs = self.attrs;
        quote! {
            #attrs
            pub #name: #ty,
        }
    }
}

fn collect_declare_account_group(
    group_name: &str,
    accounts: &[serde_json::Value],
    groups: &mut std::collections::BTreeMap<String, Vec<DeclareAccountField>>,
    span: proc_macro2::Span,
) -> syn::Result<()> {
    if groups.contains_key(group_name) {
        return Ok(());
    }

    let mut fields = Vec::new();
    for account in accounts {
        let name = json_str(account, "name", span)?;
        let ident = Ident::new(&to_snake_case(name), span);
        if let Some(nested) = account
            .get("accounts")
            .and_then(serde_json::Value::as_array)
        {
            let nested_name = to_type_name(name);
            collect_declare_account_group(&nested_name, nested, groups, span)?;
            let nested_ident = Ident::new(&nested_name, span);
            fields.push(DeclareAccountField {
                name: ident,
                ty: quote! { anchor_lang_v2::Nested<#nested_ident> },
                attrs: quote! {},
            });
            continue;
        }

        let writable = account
            .get("writable")
            .or_else(|| account.get("isMut"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let signer = account
            .get("signer")
            .or_else(|| account.get("isSigner"))
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);
        let optional = account
            .get("optional")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false);

        let base_ty = quote! { anchor_lang_v2::accounts::UncheckedAccount };
        let ty = if optional {
            quote! { Option<#base_ty> }
        } else {
            base_ty
        };
        let attrs = match (writable, signer) {
            (true, true) => quote! { #[account(mut, signer)] },
            (true, false) => quote! { #[account(mut)] },
            (false, true) => quote! { #[account(signer)] },
            (false, false) => quote! {},
        };
        fields.push(DeclareAccountField {
            name: ident,
            ty,
            attrs,
        });
    }

    groups.insert(group_name.to_string(), fields);
    Ok(())
}

fn gen_declare_program_types(idl: &serde_json::Value) -> syn::Result<Vec<TokenStream2>> {
    let Some(types) = idl.get("types").and_then(serde_json::Value::as_array) else {
        return Ok(Vec::new());
    };
    let account_entries: std::collections::BTreeMap<_, _> = idl
        .get("accounts")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|account| {
            let name = account.get("name")?.as_str()?;
            Some((name, account))
        })
        .collect();
    let mut out = Vec::new();
    for ty_def in types {
        let name = json_str(ty_def, "name", proc_macro2::Span::call_site())?;
        let ident = Ident::new(&to_type_name(name), proc_macro2::Span::call_site());
        let docs = gen_declare_program_docs(ty_def, ident.span());
        let repr = gen_declare_program_repr(ty_def, ident.span())?;
        let serialization = declare_type_serialization(ty_def, ident.span())?;
        let bytemuck_repr = if repr.is_none() && serialization.is_bytemuck() {
            quote! { #[repr(C)] }
        } else {
            quote! { #repr }
        };
        let generics = gen_declare_program_type_generics(ty_def, ident.span())?;
        let account_entry = account_entries.get(name).copied();
        let discriminator = account_entry
            .and_then(|account| account.get("discriminator"))
            .and_then(serde_json::Value::as_array);
        let discriminator_impl = if let Some(discriminator) = discriminator {
            let bytes: Vec<_> = discriminator
                .iter()
                .map(|value| {
                    let byte = value.as_u64().ok_or_else(|| {
                        syn::Error::new(
                            ident.span(),
                            format!("account `{name}` discriminator entries must be integers"),
                        )
                    })?;
                    if byte > u8::MAX as u64 {
                        return Err(syn::Error::new(
                            ident.span(),
                            format!("account `{name}` discriminator entry `{byte}` exceeds u8"),
                        ));
                    }
                    Ok(byte as u8)
                })
                .collect::<syn::Result<_>>()?;
            let bytes = bytes.iter().map(|byte| quote! { #byte });
            let impl_generics = &generics.impl_generics;
            let ty_generics = &generics.ty_generics;
            quote! {
                impl #impl_generics anchor_lang_v2::Owner for #ident #ty_generics {
                    const OWNER: anchor_lang_v2::Address = ID;
                }

                impl #impl_generics anchor_lang_v2::Discriminator for #ident #ty_generics {
                    const DISCRIMINATOR: &'static [u8] = &[#(#bytes),*];
                }
            }
        } else {
            quote! {}
        };
        let type_obj = ty_def.get("type").ok_or_else(|| {
            syn::Error::new(ident.span(), format!("type `{name}` is missing type body"))
        })?;
        let idl_type_def = syn::LitStr::new(&ty_def.to_string(), ident.span());
        let idl_account_entry =
            account_entry.map(|account| syn::LitStr::new(&account.to_string(), ident.span()));
        let kind = json_str(type_obj, "kind", ident.span())?;
        match kind {
            "struct" => {
                let fields = match type_obj.get("fields") {
                    Some(fields) => {
                        let fields = fields.as_array().ok_or_else(|| {
                            syn::Error::new(
                                ident.span(),
                                format!("struct type `{name}` fields must be an array"),
                            )
                        })?;
                        gen_declare_program_type_fields(fields, ident.span(), true)?
                    }
                    None => DeclareTypeFields::Unit,
                };
                let idl_field_tys = fields.tys();
                let idl_impl = gen_declare_program_idl_account_type_impl(
                    &ident,
                    &generics,
                    idl_account_entry.as_ref(),
                    &idl_type_def,
                    idl_field_tys,
                );
                let pod_impls = serialization
                    .is_bytemuck()
                    .then(|| gen_declare_program_pod_impls(&ident, &generics, &fields))
                    .unwrap_or_default();
                let impl_generics = &generics.impl_generics;
                out.push(match fields {
                    DeclareTypeFields::Named { fields, .. } if serialization.is_bytemuck() => quote! {
                        #(#docs)*
                        #[derive(Clone, Copy)]
                        #bytemuck_repr
                        pub struct #ident #impl_generics {
                            #(#fields)*
                        }
                        #pod_impls
                        #discriminator_impl
                        #idl_impl
                    },
                    DeclareTypeFields::Named { fields, .. } => quote! {
                        #(#docs)*
                        #repr
                        #[derive(Clone, anchor_lang_v2::wincode::SchemaRead, anchor_lang_v2::wincode::SchemaWrite)]
                        pub struct #ident #impl_generics {
                            #(#fields)*
                        }
                        #discriminator_impl
                        #idl_impl
                    },
                    DeclareTypeFields::Tuple { fields, .. } if serialization.is_bytemuck() => quote! {
                        #(#docs)*
                        #[derive(Clone, Copy)]
                        #bytemuck_repr
                        pub struct #ident #impl_generics(#(#fields),*);
                        #pod_impls
                        #discriminator_impl
                        #idl_impl
                    },
                    DeclareTypeFields::Tuple { fields, .. } => quote! {
                        #(#docs)*
                        #repr
                        #[derive(Clone, anchor_lang_v2::wincode::SchemaRead, anchor_lang_v2::wincode::SchemaWrite)]
                        pub struct #ident #impl_generics(#(#fields),*);
                        #discriminator_impl
                        #idl_impl
                    },
                    DeclareTypeFields::Unit if serialization.is_bytemuck() => quote! {
                        #(#docs)*
                        #[derive(Clone, Copy)]
                        #bytemuck_repr
                        pub struct #ident #impl_generics;
                        #pod_impls
                        #discriminator_impl
                        #idl_impl
                    },
                    DeclareTypeFields::Unit => quote! {
                        #(#docs)*
                        #repr
                        #[derive(Clone, anchor_lang_v2::wincode::SchemaRead, anchor_lang_v2::wincode::SchemaWrite)]
                        pub struct #ident #impl_generics;
                        #discriminator_impl
                        #idl_impl
                    },
                });
            }
            "enum" => {
                if serialization.is_bytemuck() {
                    return Err(syn::Error::new(
                        ident.span(),
                        format!("declare_program! does not support bytemuck enum type `{name}`"),
                    ));
                }
                let variants = type_obj
                    .get("variants")
                    .and_then(serde_json::Value::as_array)
                    .ok_or_else(|| {
                        syn::Error::new(
                            ident.span(),
                            format!("enum type `{name}` is missing variants"),
                        )
                    })?;
                let mut variant_tokens = Vec::new();
                let mut idl_field_tys = Vec::new();
                for variant in variants {
                    let variant_name = json_str(variant, "name", ident.span())?;
                    let variant_ident = Ident::new(variant_name, ident.span());
                    let Some(fields) = variant.get("fields") else {
                        variant_tokens.push(quote! { #variant_ident, });
                        continue;
                    };
                    let fields = fields.as_array().ok_or_else(|| {
                        syn::Error::new(
                            ident.span(),
                            format!("variant `{variant_name}` fields must be an array"),
                        )
                    })?;
                    let fields = gen_declare_program_type_fields(fields, ident.span(), false)?;
                    idl_field_tys.extend(fields.tys().iter().cloned());
                    match fields {
                        DeclareTypeFields::Named { fields, .. } => {
                            variant_tokens.push(quote! { #variant_ident { #(#fields)* }, });
                        }
                        DeclareTypeFields::Tuple { fields, .. } => {
                            variant_tokens.push(quote! { #variant_ident(#(#fields),*), });
                        }
                        DeclareTypeFields::Unit => {
                            variant_tokens.push(quote! { #variant_ident, });
                        }
                    }
                }
                let idl_impl = gen_declare_program_idl_account_type_impl(
                    &ident,
                    &generics,
                    idl_account_entry.as_ref(),
                    &idl_type_def,
                    &idl_field_tys,
                );
                let impl_generics = &generics.impl_generics;
                out.push(quote! {
                    #(#docs)*
                    #repr
                    #[derive(Clone, anchor_lang_v2::wincode::SchemaRead, anchor_lang_v2::wincode::SchemaWrite)]
                    pub enum #ident #impl_generics {
                        #(#variant_tokens)*
                    }
                    #discriminator_impl
                    #idl_impl
                });
            }
            "type" => {
                let alias = type_obj.get("alias").ok_or_else(|| {
                    syn::Error::new(
                        ident.span(),
                        format!("type alias `{name}` is missing alias"),
                    )
                })?;
                let alias = declare_idl_type_to_tokens(alias, ident.span())?;
                let impl_generics = &generics.impl_generics;
                out.push(quote! {
                    #(#docs)*
                    pub type #ident #impl_generics = #alias;
                });
            }
            _ => {
                return Err(syn::Error::new(
                    ident.span(),
                    format!("declare_program! only supports struct, enum, and type alias IDL types for now, got `{kind}`"),
                ));
            }
        }
    }
    Ok(out)
}

#[derive(Clone, Copy)]
enum DeclareTypeSerialization {
    Borsh,
    Bytemuck,
    BytemuckUnsafe,
}

impl DeclareTypeSerialization {
    fn is_bytemuck(self) -> bool {
        matches!(self, Self::Bytemuck | Self::BytemuckUnsafe)
    }
}

fn gen_declare_program_docs(
    value: &serde_json::Value,
    span: proc_macro2::Span,
) -> Vec<TokenStream2> {
    value
        .get("docs")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(serde_json::Value::as_str)
        .map(|doc| format!("{}{doc}", if doc.is_empty() { "" } else { " " }))
        .map(|doc| {
            let doc = syn::LitStr::new(&doc, span);
            quote! { #[doc = #doc] }
        })
        .collect()
}

fn gen_declare_program_repr(
    ty_def: &serde_json::Value,
    span: proc_macro2::Span,
) -> syn::Result<Option<TokenStream2>> {
    let Some(repr) = ty_def.get("repr") else {
        return Ok(None);
    };
    let kind = match json_str(repr, "kind", span)? {
        "rust" => Ident::new("Rust", span),
        "c" => Ident::new("C", span),
        "transparent" => Ident::new("transparent", span),
        other => {
            return Err(syn::Error::new(
                span,
                format!("unsupported IDL repr kind `{other}`"),
            ))
        }
    };

    let modifier = if repr.get("kind").and_then(serde_json::Value::as_str) == Some("transparent") {
        None
    } else {
        let packed = repr
            .get("packed")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false)
            .then_some(quote! { packed });
        let align = repr
            .get("align")
            .and_then(serde_json::Value::as_u64)
            .map(|align| proc_macro2::Literal::usize_unsuffixed(align as usize))
            .map(|align| quote! { align(#align) });

        match (packed, align) {
            (None, None) => None,
            (Some(packed), None) => Some(quote! { #packed }),
            (None, Some(align)) => Some(quote! { #align }),
            (Some(packed), Some(align)) => Some(quote! { #packed, #align }),
        }
    }
    .map(|modifier| quote! { , #modifier });

    Ok(Some(quote! { #[repr(#kind #modifier)] }))
}

fn declare_type_serialization(
    ty_def: &serde_json::Value,
    span: proc_macro2::Span,
) -> syn::Result<DeclareTypeSerialization> {
    match ty_def
        .get("serialization")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("borsh")
    {
        "borsh" => Ok(DeclareTypeSerialization::Borsh),
        "bytemuck" => Ok(DeclareTypeSerialization::Bytemuck),
        "bytemuckunsafe" | "bytemuckUnsafe" => Ok(DeclareTypeSerialization::BytemuckUnsafe),
        other => Err(syn::Error::new(
            span,
            format!("unsupported IDL type serialization `{other}`"),
        )),
    }
}

fn gen_declare_program_type_idents(idl: &serde_json::Value) -> syn::Result<Vec<Ident>> {
    let mut names = std::collections::BTreeSet::new();
    for section in ["accounts", "types"] {
        if let Some(items) = idl.get(section).and_then(serde_json::Value::as_array) {
            for item in items {
                names.insert(json_str(item, "name", proc_macro2::Span::call_site())?.to_string());
            }
        }
    }
    Ok(names
        .into_iter()
        .map(|name| Ident::new(&to_type_name(&name), proc_macro2::Span::call_site()))
        .collect())
}

fn gen_declare_program_constants(idl: &serde_json::Value) -> syn::Result<Vec<TokenStream2>> {
    let Some(constants) = idl.get("constants").and_then(serde_json::Value::as_array) else {
        return Ok(Vec::new());
    };
    let mut out = Vec::new();
    for constant in constants {
        let name = json_str(constant, "name", proc_macro2::Span::call_site())?;
        let ident = Ident::new(name, proc_macro2::Span::call_site());
        let docs = gen_declare_program_docs(constant, ident.span());
        let ty_value = constant.get("type").ok_or_else(|| {
            syn::Error::new(ident.span(), format!("constant `{name}` is missing type"))
        })?;
        let value = json_str(constant, "value", ident.span())?;
        let constant = gen_declare_program_constant(&ident, ty_value, value)?;
        out.push(quote! {
            #(#docs)*
            #constant
        });
    }
    Ok(out)
}

fn gen_declare_program_events(idl: &serde_json::Value) -> syn::Result<TokenStream2> {
    let Some(events) = idl.get("events").and_then(serde_json::Value::as_array) else {
        return Ok(quote! {
            pub mod events {
                use super::*;
            }
        });
    };

    let type_defs: std::collections::BTreeMap<_, _> = idl
        .get("types")
        .and_then(serde_json::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|ty_def| {
            let name = ty_def.get("name")?.as_str()?;
            Some((name, ty_def))
        })
        .collect();

    let mut event_reexports = Vec::new();
    let mut impls = Vec::new();
    let mut parser_variants = Vec::new();
    let mut parser_branches = Vec::new();
    let span = proc_macro2::Span::call_site();

    for event in events {
        let name = json_str(event, "name", span)?;
        let ident = Ident::new(&to_type_name(name), span);
        let discriminator = event
            .get("discriminator")
            .and_then(serde_json::Value::as_array)
            .map(|values| parse_discriminator_array(values, span))
            .transpose()?
            .ok_or_else(|| {
                syn::Error::new(
                    span,
                    format!("event `{name}` is missing discriminator in `{event}`"),
                )
            })?;
        let discriminator = discriminator.iter().map(|byte| quote! { #byte });
        let ty_def = type_defs.get(name).copied().ok_or_else(|| {
            syn::Error::new(
                span,
                format!("event `{name}` is missing matching type definition"),
            )
        })?;
        let serialization = declare_type_serialization(ty_def, span)?;

        event_reexports.push(quote! { pub use super::#ident; });
        parser_variants.push(quote! { #ident(super::events::#ident) });

        let event_impl = if serialization.is_bytemuck() {
            quote! {
                impl anchor_lang_v2::Event for #ident {
                    fn data(&self) -> anchor_lang_v2::__alloc::vec::Vec<u8> {
                        let disc = <Self as anchor_lang_v2::Discriminator>::DISCRIMINATOR;
                        let payload = anchor_lang_v2::bytemuck::bytes_of(self);
                        let mut data =
                            anchor_lang_v2::__alloc::vec::Vec::with_capacity(disc.len() + payload.len());
                        data.extend_from_slice(disc);
                        data.extend_from_slice(payload);
                        data
                    }
                }
            }
        } else {
            quote! {
                impl anchor_lang_v2::Event for #ident {
                    fn data(&self) -> anchor_lang_v2::__alloc::vec::Vec<u8> {
                        let disc = <Self as anchor_lang_v2::Discriminator>::DISCRIMINATOR;
                        let mut data =
                            anchor_lang_v2::__alloc::vec::Vec::with_capacity(disc.len() + 256);
                        data.extend_from_slice(disc);
                        anchor_lang_v2::wincode::config::serialize_into(
                            &mut data,
                            self,
                            anchor_lang_v2::BORSH_CONFIG,
                        )
                        .expect("declared event serialization cannot fail for derived SchemaWrite types");
                        data
                    }
                }
            }
        };

        impls.push(quote! {
            impl anchor_lang_v2::Discriminator for #ident {
                const DISCRIMINATOR: &'static [u8] = &[#(#discriminator),*];
            }

            #event_impl
        });

        let parser_body = if serialization.is_bytemuck() {
            quote! {
                let payload = &value[<super::events::#ident as anchor_lang_v2::Discriminator>::DISCRIMINATOR.len()..];
                let expected = core::mem::size_of::<super::events::#ident>();
                if payload.len() != expected {
                    return Err(anchor_lang_v2::Error::InvalidInstructionData);
                }
                return Ok(Self::#ident(
                    anchor_lang_v2::bytemuck::pod_read_unaligned(payload)
                ));
            }
        } else {
            quote! {
                let mut payload = &value[<super::events::#ident as anchor_lang_v2::Discriminator>::DISCRIMINATOR.len()..];
                let decoded =
                    <super::events::#ident as anchor_lang_v2::wincode::SchemaRead<
                        '_,
                        anchor_lang_v2::BorshConfig,
                    >>::get(&mut payload)
                    .map_err(|_| anchor_lang_v2::Error::InvalidInstructionData)?;
                if !payload.is_empty() {
                    return Err(anchor_lang_v2::Error::InvalidInstructionData);
                }
                return Ok(Self::#ident(decoded));
            }
        };
        parser_branches.push(quote! {
            if value.starts_with(
                <super::events::#ident as anchor_lang_v2::Discriminator>::DISCRIMINATOR,
            ) {
                #parser_body
            }
        });
    }

    let parser_mod = if parser_variants.is_empty() {
        quote! {}
    } else {
        quote! {
            pub mod parsers {
                use super::*;

                pub enum Event {
                    #(#parser_variants,)*
                }

                impl Event {
                    pub fn parse(data: &[u8]) -> anchor_lang_v2::Result<Self> {
                        Self::try_from(data)
                    }
                }

                impl core::convert::TryFrom<&[u8]> for Event {
                    type Error = anchor_lang_v2::Error;

                    fn try_from(value: &[u8]) -> anchor_lang_v2::Result<Self> {
                        #(#parser_branches)*
                        Err(anchor_lang_v2::Error::InvalidArgument)
                    }
                }
            }
        }
    };

    Ok(quote! {
        pub mod events {
            use super::*;

            #(#event_reexports)*
        }

        #(#impls)*

        #parser_mod
    })
}

fn gen_declare_program_constant(
    ident: &Ident,
    ty_value: &serde_json::Value,
    value: &str,
) -> syn::Result<TokenStream2> {
    let span = ident.span();
    if ty_value.as_str() == Some("bytes") {
        let bytes = parse_declare_program_byte_array(value, span)?;
        let bytes = bytes.iter().map(|b| quote! { #b });
        return Ok(quote! { pub const #ident: &'static [u8] = &[#(#bytes),*]; });
    }

    if ty_value.as_str() == Some("string") {
        let expr: Expr = syn::parse_str(value).map_err(|err| {
            syn::Error::new(
                span,
                format!("failed to parse string constant value: {err}"),
            )
        })?;
        return Ok(quote! { pub const #ident: &'static str = #expr; });
    }

    if ty_value.as_str() == Some("pubkey") {
        let value = syn::parse_str::<syn::LitStr>(value)
            .map(|value| value.value())
            .unwrap_or_else(|_| value.to_owned());
        let value = syn::LitStr::new(&value, span);
        return Ok(quote! {
            pub const #ident: anchor_lang_v2::Address =
                anchor_lang_v2::address!(#value);
        });
    }

    if let Some(array) = ty_value.get("array").and_then(serde_json::Value::as_array) {
        if array.len() == 2 && array[0].as_str() == Some("u8") {
            let len = declare_idl_array_len_to_tokens(&array[1], span)?;
            let bytes = parse_declare_program_byte_array(value, span)?;
            if let Some(len) = array[1].as_u64() {
                let len = len as usize;
                if bytes.len() != len {
                    return Err(syn::Error::new(
                        span,
                        format!(
                            "constant `{ident}` has {} bytes, expected {len}",
                            bytes.len()
                        ),
                    ));
                }
            }
            let bytes = bytes.iter().map(|b| quote! { #b });
            return Ok(quote! { pub const #ident: [u8; #len] = [#(#bytes),*]; });
        }
    }

    let ty = declare_idl_const_type_to_tokens(ty_value, span)?;
    let expr: Expr = syn::parse_str(value)
        .map_err(|err| syn::Error::new(span, format!("failed to parse constant value: {err}")))?;
    Ok(quote! { pub const #ident: #ty = #expr; })
}

fn declare_idl_const_type_to_tokens(
    value: &serde_json::Value,
    span: proc_macro2::Span,
) -> syn::Result<TokenStream2> {
    if let Some(s) = value.as_str() {
        match s {
            "bytes" => return Ok(quote! { &'static [u8] }),
            "string" => return Ok(quote! { &'static str }),
            _ => {}
        }
    }
    declare_idl_type_to_tokens(value, span)
}

fn parse_declare_program_byte_array(value: &str, span: proc_macro2::Span) -> syn::Result<Vec<u8>> {
    let value = value.trim();
    let value = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .ok_or_else(|| syn::Error::new(span, "byte-array constant value must be `[u8, ...]`"))?;
    if value.trim().is_empty() {
        return Ok(Vec::new());
    }
    value
        .split(',')
        .map(|part| {
            part.trim()
                .parse::<u8>()
                .map_err(|err| syn::Error::new(span, format!("invalid byte constant: {err}")))
        })
        .collect()
}

enum DeclareTypeFields {
    Named {
        fields: Vec<TokenStream2>,
        tys: Vec<TokenStream2>,
    },
    Tuple {
        fields: Vec<TokenStream2>,
        tys: Vec<TokenStream2>,
    },
    Unit,
}

impl DeclareTypeFields {
    fn tys(&self) -> &[TokenStream2] {
        match self {
            Self::Named { tys, .. } | Self::Tuple { tys, .. } => tys,
            Self::Unit => &[],
        }
    }
}

struct DeclareTypeGenerics {
    impl_generics: TokenStream2,
    ty_generics: TokenStream2,
    idl_where_clause: TokenStream2,
    pod_where_clause: TokenStream2,
}

fn gen_declare_program_type_generics(
    ty_def: &serde_json::Value,
    span: proc_macro2::Span,
) -> syn::Result<DeclareTypeGenerics> {
    let Some(generics) = ty_def.get("generics").and_then(serde_json::Value::as_array) else {
        return Ok(DeclareTypeGenerics {
            impl_generics: quote! {},
            ty_generics: quote! {},
            idl_where_clause: quote! {},
            pod_where_clause: quote! {},
        });
    };
    let mut impl_params = Vec::new();
    let mut ty_params = Vec::new();
    let mut idl_bounds = Vec::new();
    let mut pod_bounds = Vec::new();
    for generic in generics {
        let name = json_str(generic, "name", span)?;
        let ident = Ident::new(name, span);
        match json_str(generic, "kind", span)? {
            "type" => {
                impl_params.push(quote! { #ident });
                ty_params.push(quote! { #ident });
                idl_bounds.push(quote! { #ident: anchor_lang_v2::IdlAccountType });
                pod_bounds.push(quote! {
                    #ident: anchor_lang_v2::bytemuck::Pod + anchor_lang_v2::bytemuck::Zeroable
                });
            }
            "const" => {
                let ty = json_str(generic, "type", span)?;
                let ty: Type = syn::parse_str(ty).map_err(|err| {
                    syn::Error::new(
                        span,
                        format!("failed to parse const generic `{name}` type `{ty}`: {err}"),
                    )
                })?;
                impl_params.push(quote! { const #ident: #ty });
                ty_params.push(quote! { #ident });
            }
            other => {
                return Err(syn::Error::new(
                    span,
                    format!("unsupported IDL type generic kind `{other}`"),
                ))
            }
        }
    }
    let impl_generics = quote! { <#(#impl_params),*> };
    let ty_generics = quote! { <#(#ty_params),*> };
    let idl_where_clause = if idl_bounds.is_empty() {
        quote! {}
    } else {
        quote! { where #(#idl_bounds),* }
    };
    let pod_where_clause = if pod_bounds.is_empty() {
        quote! {}
    } else {
        quote! { where #(#pod_bounds),* }
    };
    Ok(DeclareTypeGenerics {
        impl_generics,
        ty_generics,
        idl_where_clause,
        pod_where_clause,
    })
}

fn gen_declare_program_idl_account_type_impl(
    ident: &Ident,
    generics: &DeclareTypeGenerics,
    account_entry: Option<&syn::LitStr>,
    type_def: &syn::LitStr,
    field_tys: &[TokenStream2],
) -> TokenStream2 {
    let account_entry = match account_entry {
        Some(account_entry) => quote! { Some(#account_entry) },
        None => quote! { None },
    };
    let impl_generics = &generics.impl_generics;
    let ty_generics = &generics.ty_generics;
    let where_clause = &generics.idl_where_clause;
    quote! {
        #[doc(hidden)]
        impl #impl_generics anchor_lang_v2::IdlAccountType for #ident #ty_generics #where_clause {
            const __IDL_ACCOUNT_ENTRY: Option<&'static str> = #account_entry;
            const __IDL_TYPE_DEF: Option<&'static str> = Some(#type_def);

            fn __register_idl_deps(
                accounts: &mut anchor_lang_v2::__alloc::vec::Vec<&'static str>,
                types: &mut anchor_lang_v2::__alloc::vec::Vec<&'static str>,
            ) {
                if let Some(a) = <Self as anchor_lang_v2::IdlAccountType>::__IDL_ACCOUNT_ENTRY {
                    accounts.push(a);
                }
                if let Some(t) = <Self as anchor_lang_v2::IdlAccountType>::__IDL_TYPE_DEF {
                    types.push(t);
                }
                #(
                    <#field_tys as anchor_lang_v2::IdlAccountType>::__register_idl_deps(accounts, types);
                )*
            }
        }
    }
}

fn gen_declare_program_pod_impls(
    ident: &Ident,
    generics: &DeclareTypeGenerics,
    fields: &DeclareTypeFields,
) -> TokenStream2 {
    let field_types = fields.tys();
    let impl_generics = &generics.impl_generics;
    let ty_generics = &generics.ty_generics;
    let where_clause = &generics.pod_where_clause;
    quote! {
        impl #impl_generics #ident #ty_generics #where_clause {
            const __ANCHOR_DECLARE_PROGRAM_POD_ASSERT: fn() = || {
                fn assert_pod<T: anchor_lang_v2::bytemuck::Pod>() {}
                #( assert_pod::<#field_types>(); )*
            };
            const __ANCHOR_DECLARE_PROGRAM_NO_PADDING: () = assert!(
                core::mem::size_of::<Self>() == 0 #(+ core::mem::size_of::<#field_types>())*,
                "declared bytemuck type has padding bytes"
            );
        }
        unsafe impl #impl_generics anchor_lang_v2::bytemuck::Pod for #ident #ty_generics #where_clause {}
        unsafe impl #impl_generics anchor_lang_v2::bytemuck::Zeroable for #ident #ty_generics #where_clause {}
    }
}

fn gen_declare_program_type_fields(
    fields: &[serde_json::Value],
    span: proc_macro2::Span,
    public: bool,
) -> syn::Result<DeclareTypeFields> {
    let visibility = public.then(|| quote! { pub });
    if fields.iter().all(serde_json::Value::is_object) {
        let mut field_tokens = Vec::new();
        let mut field_tys = Vec::new();
        for field in fields {
            let field_name = json_str(field, "name", span)?;
            let field_ident = Ident::new(&to_snake_case(field_name), span);
            let ty_value = field.get("type").ok_or_else(|| {
                syn::Error::new(span, format!("field `{field_name}` is missing type"))
            })?;
            let ty = declare_idl_type_to_tokens(ty_value, span)?;
            field_tokens.push(quote! { #visibility #field_ident: #ty, });
            field_tys.push(ty);
        }
        Ok(DeclareTypeFields::Named {
            fields: field_tokens,
            tys: field_tys,
        })
    } else {
        let mut field_tokens = Vec::new();
        let mut field_tys = Vec::new();
        for field in fields {
            let ty = declare_idl_type_to_tokens(field, span)?;
            field_tokens.push(quote! { #visibility #ty });
            field_tys.push(ty);
        }
        Ok(DeclareTypeFields::Tuple {
            fields: field_tokens,
            tys: field_tys,
        })
    }
}

fn declare_idl_type_to_tokens(
    value: &serde_json::Value,
    span: proc_macro2::Span,
) -> syn::Result<TokenStream2> {
    if let Some(s) = value.as_str() {
        return Ok(match s {
            "bool" => quote! { bool },
            "u8" => quote! { u8 },
            "i8" => quote! { i8 },
            "u16" => quote! { u16 },
            "i16" => quote! { i16 },
            "u32" => quote! { u32 },
            "i32" => quote! { i32 },
            "f32" => quote! { f32 },
            "u64" => quote! { u64 },
            "i64" => quote! { i64 },
            "f64" => quote! { f64 },
            "u128" => quote! { u128 },
            "i128" => quote! { i128 },
            "bytes" => quote! { anchor_lang_v2::__alloc::vec::Vec<u8> },
            "string" => quote! { anchor_lang_v2::__alloc::string::String },
            "pubkey" => quote! { anchor_lang_v2::Address },
            other => {
                return Err(syn::Error::new(
                    span,
                    format!("unsupported IDL type string `{other}`"),
                ))
            }
        });
    }

    if let Some(defined) = value.get("defined") {
        let defined_name = if let Some(name) = defined.as_str() {
            name
        } else {
            json_str(defined, "name", span)?
        };
        let ident = Ident::new(&to_type_name(defined_name), span);
        let generic_args = defined
            .get("generics")
            .and_then(serde_json::Value::as_array)
            .map(|generics| {
                generics
                    .iter()
                    .map(|generic| declare_idl_generic_arg_to_tokens(generic, span))
                    .collect::<syn::Result<Vec<_>>>()
            })
            .transpose()?
            .unwrap_or_default();
        if generic_args.is_empty() {
            return Ok(quote! { #ident });
        }
        return Ok(quote! { #ident<#(#generic_args),*> });
    }
    if let Some(generic) = value.get("generic").and_then(serde_json::Value::as_str) {
        let ident = Ident::new(generic, span);
        return Ok(quote! { #ident });
    }
    if let Some(inner) = value.get("vec") {
        let inner = declare_idl_type_to_tokens(inner, span)?;
        return Ok(quote! { anchor_lang_v2::__alloc::vec::Vec<#inner> });
    }
    if let Some(inner) = value.get("option") {
        let inner = declare_idl_type_to_tokens(inner, span)?;
        return Ok(quote! { Option<#inner> });
    }
    if let Some(array) = value.get("array").and_then(serde_json::Value::as_array) {
        if array.len() != 2 {
            return Err(syn::Error::new(
                span,
                "IDL array type must have two elements",
            ));
        }
        let inner = declare_idl_type_to_tokens(&array[0], span)?;
        let len = declare_idl_array_len_to_tokens(&array[1], span)?;
        return Ok(quote! { [#inner; #len] });
    }

    Err(syn::Error::new(
        span,
        format!("unsupported IDL type `{value}`"),
    ))
}

fn declare_idl_array_len_to_tokens(
    value: &serde_json::Value,
    span: proc_macro2::Span,
) -> syn::Result<TokenStream2> {
    if let Some(len) = value.as_u64() {
        let len = len as usize;
        return Ok(quote! { #len });
    }
    if let Some(generic) = value.get("generic").and_then(serde_json::Value::as_str) {
        let ident = Ident::new(generic, span);
        return Ok(quote! { #ident });
    }
    if let Some(generic) = value.as_str() {
        let ident = Ident::new(generic, span);
        return Ok(quote! { #ident });
    }
    Err(syn::Error::new(
        span,
        format!("unsupported IDL array length `{value}`"),
    ))
}

fn declare_idl_generic_arg_to_tokens(
    value: &serde_json::Value,
    span: proc_macro2::Span,
) -> syn::Result<TokenStream2> {
    match json_str(value, "kind", span)? {
        "type" => {
            let ty = value.get("type").ok_or_else(|| {
                syn::Error::new(
                    span,
                    format!("generic type arg is missing type in `{value}`"),
                )
            })?;
            declare_idl_type_to_tokens(ty, span)
        }
        "const" => {
            let value = json_str(value, "value", span)?;
            let expr: Expr = syn::parse_str(value).map_err(|err| {
                syn::Error::new(
                    span,
                    format!("failed to parse const generic value `{value}`: {err}"),
                )
            })?;
            Ok(quote! { #expr })
        }
        other => Err(syn::Error::new(
            span,
            format!("unsupported IDL generic arg kind `{other}`"),
        )),
    }
}

fn json_str<'a>(
    value: &'a serde_json::Value,
    key: &str,
    span: proc_macro2::Span,
) -> syn::Result<&'a str> {
    value
        .get(key)
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            syn::Error::new(
                span,
                format!("IDL object is missing string field `{key}` in `{value}`"),
            )
        })
}

fn parse_discriminator_array(
    values: &[serde_json::Value],
    span: proc_macro2::Span,
) -> syn::Result<Vec<u8>> {
    if values.is_empty() {
        return Err(syn::Error::new(span, "IDL discriminator must not be empty"));
    }
    values
        .iter()
        .map(|value| {
            value
                .as_u64()
                .filter(|value| *value <= u8::MAX as u64)
                .map(|value| value as u8)
                .ok_or_else(|| syn::Error::new(span, "IDL discriminator values must be bytes"))
        })
        .collect()
}

fn default_instruction_discriminator(name: &str) -> Vec<u8> {
    use sha2::Digest;
    sha2::Sha256::digest(format!("global:{name}").as_bytes())[..8].to_vec()
}

fn to_snake_case(input: &str) -> String {
    let mut out = String::new();
    for (i, ch) in input.chars().enumerate() {
        if ch == '-' || ch == ' ' {
            out.push('_');
        } else if ch.is_ascii_uppercase() {
            if i != 0 {
                out.push('_');
            }
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

fn to_type_name(input: &str) -> String {
    to_snake_case(input)
        .split('_')
        .filter(|part| !part.is_empty())
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(first) => first.to_ascii_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

struct HandlerCodegen {
    error: Option<TokenStream2>,
    dispatch_arm: TokenStream2,
    wrapper: TokenStream2,
    instruction_struct: TokenStream2,
    accounts_reexport: TokenStream2,
    /// Per-handler CPI wrapper function — `pub fn name(ctx, args...)` in the
    /// emitted `cpi` module. References `cpi::accounts::<Accounts>` and
    /// `instruction::<Camel>` from the same crate.
    cpi_wrapper: TokenStream2,
    /// Re-export of the auto-generated CPI accounts struct under
    /// `cpi::accounts::<Accounts>`. Deduped against `accounts_type_name`.
    cpi_accounts_reexport: TokenStream2,
    /// Name of the Accounts struct (e.g. `MutateItemList`). Used to dedupe
    /// `accounts::*` re-exports when multiple handlers share the same Accounts.
    accounts_type_name: String,
    idl_name: String,
    idl_disc: String,
    idl_args: String,
    idl_returns_json: String,
    /// Pre-rendered `,"docs":[...]` fragment (including the leading comma
    /// separator) that gets spliced into the per-instruction IDL JSON
    /// between `"name"` and `"discriminator"`. Empty string when the
    /// handler carries no `///` doc comments.
    idl_docs_json: String,
    idl_accounts_type: TokenStream2,
    /// Original (non-lifetime-transformed) arg types for min-length computation.
    arg_types: Vec<Type>,
    return_type: Option<Type>,
}

impl HandlerCodegen {
    /// Build a codegen result that surfaces a single `compile_error!` in the
    /// emitted handler wrapper. Used when handler validation fails so the
    /// proc-macro returns a properly-spanned error instead of panicking.
    fn error(handler: &syn::ItemFn, err: syn::Error) -> Self {
        let err_tokens = err.to_compile_error();
        let fn_name = &handler.sig.ident;
        Self {
            error: Some(err_tokens.clone()),
            dispatch_arm: quote! {},
            wrapper: quote! {
                #[allow(non_snake_case)]
                pub fn #fn_name() {
                    #err_tokens
                }
            },
            instruction_struct: quote! {},
            accounts_reexport: quote! {},
            cpi_wrapper: quote! {},
            cpi_accounts_reexport: quote! {},
            accounts_type_name: String::new(),
            idl_name: fn_name.to_string(),
            idl_disc: "[]".to_string(),
            idl_args: "[]".to_string(),
            idl_returns_json: String::new(),
            idl_docs_json: String::new(),
            idl_accounts_type: quote! { () },
            arg_types: Vec::new(),
            return_type: None,
        }
    }
}

fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

fn extract_result_return_type(output: &syn::ReturnType) -> syn::Result<Option<Type>> {
    let syn::ReturnType::Type(_, ty) = output else {
        return Ok(None);
    };

    let Type::Path(type_path) = &**ty else {
        return Err(syn::Error::new(
            ty.span(),
            "handler return type must be `Result<T>`",
        ));
    };
    let Some(segment) = type_path.path.segments.last() else {
        return Err(syn::Error::new(
            ty.span(),
            "handler return type must be `Result<T>`",
        ));
    };
    if segment.ident != "Result" {
        return Err(syn::Error::new(
            ty.span(),
            "handler return type must be `Result<T>`",
        ));
    }
    let syn::PathArguments::AngleBracketed(args) = &segment.arguments else {
        return Err(syn::Error::new(
            ty.span(),
            "handler return type must be `Result<T>`",
        ));
    };
    let Some(syn::GenericArgument::Type(return_ty)) = args.args.first() else {
        return Err(syn::Error::new(
            ty.span(),
            "handler return type must be `Result<T>`",
        ));
    };

    if is_unit_type(return_ty) {
        Ok(None)
    } else {
        Ok(Some(return_ty.clone()))
    }
}

fn process_handler(
    handler: &syn::ItemFn,
    mod_name: &Ident,
    use_byte_disc: bool,
    discrim_bytes: Option<&[u8]>,
    program_id: &Expr,
) -> HandlerCodegen {
    let fn_name = &handler.sig.ident;
    let fn_name_str = fn_name.to_string();
    let return_type = match extract_result_return_type(&handler.sig.output) {
        Ok(return_ty) => return_ty,
        Err(err) => return HandlerCodegen::error(handler, err),
    };
    let return_ty = return_type
        .as_ref()
        .map(|return_ty| quote! { #return_ty })
        .unwrap_or_else(|| quote! { () });
    let returns_value = return_type.is_some();
    let idl_returns_json = return_type
        .as_ref()
        .map(|return_ty| format!(",\"returns\":{}", idl::rust_type_to_idl(return_ty)))
        .unwrap_or_default();
    let set_return_data = returns_value.then(|| {
        quote! {
            let mut __return_data = anchor_lang_v2::__alloc::vec::Vec::with_capacity(256);
            anchor_lang_v2::wincode::config::serialize_into(
                &mut __return_data,
                &__result,
                anchor_lang_v2::BORSH_CONFIG,
            )
                .expect("return data serialization failed");
            anchor_lang_v2::pinocchio::cpi::set_return_data(&__return_data);
        }
    });

    // Discriminator: explicit bytes for declared interfaces, 1-byte
    // user-specified executable dispatch, or 8-byte sha256 hash by default.
    use sha2::Digest;
    let hash = sha2::Sha256::digest(format!("global:{fn_name_str}").as_bytes());
    let (disc_bytes_for_idl, disc_literal_bytes, disc_match_arm_pattern): (
        Vec<u8>,
        Vec<TokenStream2>,
        TokenStream2,
    ) = if let Some(discrim_bytes) = discrim_bytes {
        let lits: Vec<_> = discrim_bytes.iter().map(|b| quote! { #b }).collect();
        let match_arm = if use_byte_disc {
            let byte = discrim_bytes[0];
            quote! { #byte }
        } else if discrim_bytes.len() == 8 {
            let disc_u64 = u64::from_le_bytes(
                discrim_bytes
                    .try_into()
                    .expect("checked length before u64 conversion"),
            );
            quote! { #disc_u64 }
        } else {
            // Non-executable interface mode does not emit dispatch arms.
            quote! { _ }
        };
        (discrim_bytes.to_vec(), lits, match_arm)
    } else {
        let disc_bytes = &hash[..8];
        let disc_u64 = u64::from_le_bytes(
            disc_bytes
                .try_into()
                .expect("sha256[..8] is always 8 bytes"),
        );
        let lits: Vec<_> = disc_bytes.iter().map(|b| quote! { #b }).collect();
        (disc_bytes.to_vec(), lits, quote! { #disc_u64 })
    };
    let fn_name_log = format!("Instruction: {fn_name_str}");

    // Parse args.
    let mut args_iter = handler.sig.inputs.iter();
    let first_arg = match args_iter.next() {
        Some(arg) => arg,
        None => {
            return HandlerCodegen::error(
                handler,
                syn::Error::new(
                    handler.sig.ident.span(),
                    "handler must have a `ctx: &mut Context<T>` parameter",
                ),
            )
        }
    };
    let accounts_type = match extract_context_accounts_ident(first_arg) {
        Ok(accounts_type) => accounts_type,
        Err(err) => return HandlerCodegen::error(handler, err),
    };

    let extra_args: Vec<(&Ident, &Type)> = args_iter
        .filter_map(|arg| {
            if let FnArg::Typed(pt) = arg {
                if let Pat::Ident(pi) = &*pt.pat {
                    return Some((&pi.ident, &*pt.ty));
                }
            }
            None
        })
        .collect();

    let extra_arg_names: Vec<_> = extra_args.iter().map(|(n, _)| *n).collect();
    let (extra_arg_types, has_ref_args) = args_meta(&extra_args);
    let extra_arg_types = &extra_arg_types;

    // Dispatch arm.
    let dispatch_arm = quote! {
        #disc_match_arm_pattern => __handlers::#fn_name(__program_id, &mut __cursor, __ix_data, __num),
    };

    // Handler wrapper.
    let wrapper = if extra_arg_names.is_empty() {
        quote! {
            #[inline(always)]
            pub fn #fn_name<'a>(
                __program_id: &'a anchor_lang_v2::Address,
                __cursor: &'a mut anchor_lang_v2::AccountCursor,
                __ix_data: &'a [u8],
                __num_accounts: usize,
            ) -> u64 {
                #[cfg(not(feature = "no-log-ix-name"))]
                anchor_lang_v2::msg!(#fn_name_log);

                #[inline(always)]
                fn __anchor_assert_no_ix_args(_: ()) {}

                match anchor_lang_v2::run_handler::<#accounts_type, #return_ty>(
                    __program_id,
                    __cursor,
                    __ix_data,
                    __num_accounts,
                    |__ctx, __ix_args| {
                        __anchor_assert_no_ix_args(__ix_args);
                        #mod_name::#fn_name(__ctx)
                    },
                ) {
                    Ok(__result) => {
                        #set_return_data
                        0
                    },
                    Err(__e) => __e.into(),
                }
            }
        }
    } else {
        let tuple_ty = quote! { (#(#extra_arg_types,)*) };
        let args_deser = emit_args_deser(&extra_args, "__Args", false);
        let deser_args = args_deser.deser;
        quote! {
            #[inline(always)]
            pub fn #fn_name<'a>(
                __program_id: &'a anchor_lang_v2::Address,
                __cursor: &'a mut anchor_lang_v2::AccountCursor,
                __ix_data: &'a [u8],
                __num_accounts: usize,
            ) -> u64 {
                #[cfg(not(feature = "no-log-ix-name"))]
                anchor_lang_v2::msg!(#fn_name_log);

                trait __AnchorIxArgCoerce<'ix> {
                    fn __coerce(self, __ix_data: &'ix [u8]) -> anchor_lang_v2::Result<#tuple_ty>;
                }

                impl<'ix> __AnchorIxArgCoerce<'ix> for () {
                    #[inline(always)]
                    fn __coerce(self, __ix_data: &'ix [u8]) -> anchor_lang_v2::Result<#tuple_ty> {
                        #deser_args
                        Ok((#(#extra_arg_names,)*))
                    }
                }

                impl<'ix> __AnchorIxArgCoerce<'ix> for #tuple_ty {
                    #[inline(always)]
                    fn __coerce(self, _ix_data: &'ix [u8]) -> anchor_lang_v2::Result<#tuple_ty> {
                        Ok(self)
                    }
                }

                match anchor_lang_v2::run_handler::<#accounts_type, #return_ty>(
                    __program_id,
                    __cursor,
                    __ix_data,
                    __num_accounts,
                    |__ctx, __ix_args| {
                        let (#(#extra_arg_names,)*) =
                            <_ as __AnchorIxArgCoerce<'a>>::__coerce(__ix_args, __ix_data)?;
                        #mod_name::#fn_name(__ctx, #(#extra_arg_names),*)
                    },
                ) {
                    Ok(__result) => {
                        #set_return_data
                        0
                    },
                    Err(__e) => __e.into(),
                }
            }
        }
    };

    // Client-side instruction struct.
    let ix_struct_name = syn::Ident::new(&to_camel_case(&fn_name_str), fn_name.span());
    let (ix_lt_decl, ix_lt_use) = if has_ref_args {
        (quote! { <'ix> }, quote! { <'ix> })
    } else {
        (quote! {}, quote! {})
    };
    let instruction_struct = quote! {
        #[derive(anchor_lang_v2::wincode::SchemaWrite)]
        pub struct #ix_struct_name #ix_lt_decl {
            #(pub #extra_arg_names: #extra_arg_types,)*
        }
        impl #ix_lt_decl anchor_lang_v2::Discriminator for #ix_struct_name #ix_lt_use {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_literal_bytes),*];
        }
        impl #ix_lt_decl anchor_lang_v2::InstructionData for #ix_struct_name #ix_lt_use {
            fn data(&self) -> alloc::vec::Vec<u8> {
                let mut data = alloc::vec::Vec::with_capacity(256);
                data.extend_from_slice(Self::DISCRIMINATOR);
                anchor_lang_v2::wincode::config::serialize_into(
                    &mut data,
                    self,
                    anchor_lang_v2::BORSH_CONFIG,
                )
                    .expect("instruction serialization failed");
                data
            }
        }
        impl #ix_lt_decl #ix_struct_name #ix_lt_use {
            pub fn to_instruction(
                self,
                accounts: impl anchor_lang_v2::ToAccountMetas,
            ) -> anchor_lang_v2::solana_program::instruction::Instruction {
                anchor_lang_v2::solana_program::instruction::Instruction::new_with_bytes(
                    #program_id,
                    &<Self as anchor_lang_v2::InstructionData>::data(&self),
                    accounts.to_account_metas(None),
                )
            }
        }
    };

    // Client accounts re-export.
    let client_mod = syn::Ident::new(
        &format!(
            "__client_accounts_{}",
            accounts_type.to_string().to_lowercase()
        ),
        fn_name.span(),
    );
    let resolved_type = syn::Ident::new(&format!("{accounts_type}Resolved"), accounts_type.span());
    let accounts_reexport = quote! {
        pub use super::#client_mod::#accounts_type;
        pub use super::#client_mod::#resolved_type;
    };

    // CPI accounts re-export — `__cpi_accounts_<lowercase>` is emitted by
    // `#[derive(Accounts)]` at the same scope as the program's outputs.
    let cpi_mod = syn::Ident::new(
        &format!(
            "__cpi_accounts_{}",
            accounts_type.to_string().to_lowercase()
        ),
        fn_name.span(),
    );
    let cpi_accounts_reexport = quote! {
        pub use super::super::#cpi_mod::#accounts_type;
    };

    // CPI wrapper function — mirrors the handler's argument list (sans
    // `ctx: &mut Context<_>`), packs them into the client-side
    // `instruction::<Camel>` struct, and forwards to `CpiContext::invoke`.
    // Lifetime story: `'a` is the CPI handle lifetime; `'ix` matches the
    // instruction struct's optional ref-args lifetime.
    let cpi_wrapper = {
        let (lt_decl, ix_lt_use_local) = if has_ref_args {
            (quote! { <'a, 'ix> }, quote! { <'ix> })
        } else {
            (quote! { <'a> }, quote! {})
        };
        let (ret_ty, ret_value) = if returns_value {
            (
                quote! { -> anchor_lang_v2::Result<Return<#return_ty>> },
                quote! {
                    Ok(Return {
                        program: *__ctx.program,
                        phantom: core::marker::PhantomData,
                    })
                },
            )
        } else {
            (quote! {}, quote! {})
        };
        quote! {
            pub fn #fn_name #lt_decl(
                __ctx: anchor_lang_v2::CpiContext<'a, accounts::#accounts_type<'a>>,
                #(#extra_arg_names: #extra_arg_types,)*
            ) #ret_ty {
                let __ix = super::instruction::#ix_struct_name #ix_lt_use_local {
                    #(#extra_arg_names,)*
                };
                let __data = <
                    super::instruction::#ix_struct_name #ix_lt_use_local
                    as anchor_lang_v2::InstructionData
                >::data(&__ix);
                __ctx.invoke(&__data);
                #ret_value
            }
        }
    };

    // Instruction-level docs come from `///` comments on the handler fn.
    // Leading-comma format so the IDL instruction JSON splice site can
    // hold a fixed `"{name}"{docs_or_empty},"discriminator":...` shape.
    let handler_docs = idl::extract_doc_lines(&handler.attrs);
    let idl_docs_json = if handler_docs.is_empty() {
        String::new()
    } else {
        format!(",\"docs\":{}", idl::docs_to_json_array(&handler_docs))
    };

    HandlerCodegen {
        dispatch_arm,
        error: None,
        wrapper,
        instruction_struct,
        accounts_reexport,
        cpi_wrapper,
        cpi_accounts_reexport,
        accounts_type_name: accounts_type.to_string(),
        idl_name: fn_name_str,
        idl_disc: idl::disc_json(&disc_bytes_for_idl),
        idl_args: idl::build_args_json(&extra_args),
        idl_returns_json,
        idl_docs_json,
        idl_accounts_type: quote! { #accounts_type },
        arg_types: extra_args.iter().map(|(_, t)| (*t).clone()).collect(),
        return_type,
    }
}

fn impl_program(module: &ItemMod, config: &ProgramConfig) -> TokenStream2 {
    let mod_name = &module.ident;
    let mod_vis = &module.vis;
    let content = match &module.content {
        Some((_, items)) => items,
        None => {
            return syn::Error::new(
                module.ident.span(),
                "`#[program]` module must have an inline body",
            )
            .to_compile_error()
        }
    };

    let mut handlers = Vec::new();
    let mut other_items = Vec::new();
    for item in content {
        if let syn::Item::Fn(func) = item {
            if matches!(&func.vis, syn::Visibility::Public(_)) {
                handlers.push(func);
                continue;
            }
        }
        other_items.push(item);
    }

    // --- Parse #[discrim = N] attributes ---
    // Executable programs preserve the existing all-or-nothing 1-byte custom
    // discriminator mode. Interface-only code can carry exact IDL discriminator
    // bytes with `#[discrim = [..]]` because it does not emit dispatch.
    let discrim_attrs: Vec<Option<DiscrimAttr>> = match handlers
        .iter()
        .map(|h| parse_discrim_attr(h))
        .collect::<Result<Vec<_>, _>>()
    {
        Ok(v) => v,
        Err(e) => return e.to_compile_error(),
    };

    let has_any_discrim = discrim_attrs.iter().any(|d| d.is_some());
    let has_all_discrim = discrim_attrs.iter().all(|d| d.is_some());
    if config.mode == ProgramMode::Executable && has_any_discrim && !has_all_discrim {
        // Point at the first handler missing #[discrim = N] for clarity.
        let missing = handlers
            .iter()
            .zip(discrim_attrs.iter())
            .find(|(_, d)| d.is_none())
            .map(|(h, _)| h.sig.ident.span())
            .unwrap_or_else(proc_macro2::Span::call_site);
        return syn::Error::new(
            missing,
            "if any instruction in `#[program]` uses `#[discrim = N]`, all must",
        )
        .to_compile_error();
    }
    let use_byte_disc = config.mode == ProgramMode::Executable && has_any_discrim;

    if config.mode == ProgramMode::Executable && has_any_discrim {
        let mut seen: std::collections::HashMap<u8, proc_macro2::Span> =
            std::collections::HashMap::new();
        for (i, d) in discrim_attrs.iter().enumerate() {
            let d = d
                .as_ref()
                .expect("all-or-nothing discrim check guarantees every entry is Some");
            if d.bytes.len() != 1 {
                return syn::Error::new(
                    d.span,
                    "executable `#[program]` custom discriminators must be one byte; use `#[program(interface, ...)]` for arbitrary IDL discriminator bytes",
                )
                .to_compile_error();
            }
            let byte = d.bytes[0];
            let span = d.span;
            if let Some(_first_span) = seen.insert(byte, span) {
                return syn::Error::new(
                    span,
                    format!(
                        "duplicate `#[discrim = {}]` on instruction `{}`",
                        byte, handlers[i].sig.ident
                    ),
                )
                .to_compile_error();
            }
        }
    } else if config.mode == ProgramMode::Interface {
        let mut seen: std::collections::HashMap<Vec<u8>, proc_macro2::Span> =
            std::collections::HashMap::new();
        for (i, d) in discrim_attrs.iter().enumerate() {
            let Some(d) = d else { continue };
            if let Some(_first_span) = seen.insert(d.bytes.clone(), d.span) {
                return syn::Error::new(
                    d.span,
                    format!(
                        "duplicate `#[discrim = ...]` on instruction `{}`",
                        handlers[i].sig.ident
                    ),
                )
                .to_compile_error();
            }
        }
    }
    let discrim_attrs: Vec<Option<Vec<u8>>> = discrim_attrs
        .iter()
        .map(|d| d.as_ref().map(|d| d.bytes.clone()))
        .collect();

    let codegen: Vec<HandlerCodegen> = handlers
        .iter()
        .enumerate()
        .map(|(i, h)| {
            process_handler(
                h,
                mod_name,
                use_byte_disc,
                discrim_attrs[i].as_deref(),
                &config.program_id,
            )
        })
        .collect();
    let handler_errors: Vec<_> = codegen.iter().filter_map(|c| c.error.as_ref()).collect();
    if !handler_errors.is_empty() {
        return quote! {
            #(#handler_errors)*
        };
    }

    let dispatch_arms: Vec<_> = codegen.iter().map(|c| &c.dispatch_arm).collect();
    let handler_wrappers: Vec<_> = codegen.iter().map(|c| &c.wrapper).collect();
    let instruction_structs: Vec<_> = codegen.iter().map(|c| &c.instruction_struct).collect();
    // Dedupe `accounts` re-exports: multiple handlers sharing the same
    // Accounts struct would otherwise emit duplicate `pub use` statements.
    let accounts_reexports: Vec<_> = {
        let mut seen = std::collections::HashSet::new();
        codegen
            .iter()
            .filter(|c| seen.insert(c.accounts_type_name.clone()))
            .map(|c| &c.accounts_reexport)
            .collect()
    };
    let cpi_accounts_reexports: Vec<_> = {
        let mut seen = std::collections::HashSet::new();
        codegen
            .iter()
            .filter(|c| seen.insert(c.accounts_type_name.clone()))
            .map(|c| &c.cpi_accounts_reexport)
            .collect()
    };
    let cpi_wrappers: Vec<_> = codegen.iter().map(|c| &c.cpi_wrapper).collect();
    let idl_ix_names: Vec<_> = codegen.iter().map(|c| &c.idl_name).collect();
    let idl_ix_discs: Vec<_> = codegen.iter().map(|c| &c.idl_disc).collect();
    let idl_ix_args: Vec<_> = codegen.iter().map(|c| &c.idl_args).collect();
    let idl_ix_returns: Vec<_> = codegen.iter().map(|c| &c.idl_returns_json).collect();
    let idl_ix_docs: Vec<_> = codegen.iter().map(|c| &c.idl_docs_json).collect();
    let idl_accounts_types: Vec<_> = codegen.iter().map(|c| &c.idl_accounts_type).collect();
    // One `Vec<Type>` per handler, for the ix-arg transitive IDL walker.
    // Nested `#(...)*` inside the quote! sees this as a Vec<Vec<Type>> and
    // expands the outer level over handlers, the inner level over each
    // handler's arg types. `arg_types` includes every handler parameter
    // past `ctx: &mut Context<...>`, so primitives / &T / Vec / etc. all
    // flow through (with the blanket no-op impls for non-`IdlType` types).
    let ix_arg_types_per_handler: Vec<&Vec<Type>> = codegen.iter().map(|c| &c.arg_types).collect();
    let all_ix_arg_types: Vec<_> = codegen.iter().map(|c| &c.arg_types).collect();
    let ix_return_type_registers: Vec<_> = codegen
        .iter()
        .filter_map(|c| c.return_type.as_ref())
        .map(|return_type| {
            quote! {
                <#return_type as anchor_lang_v2::IdlAccountType>::__register_idl_deps(
                    &mut accounts_entries,
                    &mut types_entries,
                );
            }
        })
        .collect();

    // Generate disc parsing code based on mode.
    // Returns u64 error code on failure (not Err) since __anchor_dispatch
    // returns u64 directly.
    // Build a const expression for the minimum ix_data length across all
    // instructions: disc_size + min(serialized args size per ix). Uses
    // `size_of` on a tuple of arg types — only when ALL args are owned
    // fixed-size types (no references, no dynamic-size). Falls back to 0
    // for instructions with references or complex types.
    fn is_fixed_size_primitive(ty: &syn::Type) -> bool {
        match ty {
            syn::Type::Path(p) if p.path.segments.len() == 1 => {
                let name = p.path.segments[0].ident.to_string();
                matches!(
                    name.as_str(),
                    "u8" | "u16"
                        | "u32"
                        | "u64"
                        | "u128"
                        | "i8"
                        | "i16"
                        | "i32"
                        | "i64"
                        | "i128"
                        | "bool"
                )
            }
            _ => false,
        }
    }
    let min_args_size_expr = if all_ix_arg_types.is_empty() {
        quote! { 0usize }
    } else {
        let per_ix_sizes: Vec<_> = all_ix_arg_types
            .iter()
            .map(|types| {
                if types.is_empty() || !types.iter().all(is_fixed_size_primitive) {
                    quote! { 0usize }
                } else {
                    quote! { core::mem::size_of::<(#(#types,)*)>() }
                }
            })
            .collect();
        quote! { {
            const __SIZES: &[usize] = &[#(#per_ix_sizes),*];
            const fn __const_min(s: &[usize]) -> usize {
                let mut m = s[0];
                let mut i = 1;
                while i < s.len() { if s[i] < m { m = s[i]; } i += 1; }
                m
            }
            __const_min(__SIZES)
        } }
    };
    let disc_size: usize = if use_byte_disc { 1 } else { 8 };

    let disc_parse = if use_byte_disc {
        quote! {
            const __MIN_IX_DATA_LEN: usize = #disc_size + #min_args_size_expr;
            if __ix_data_len < __MIN_IX_DATA_LEN {
                return anchor_lang_v2::Error::from(
                    anchor_lang_v2::ErrorCode::InstructionFallbackNotFound,
                ).into();
            }
            let __disc: u8 = *__ix_data_ptr;
            let __ix_data: &[u8] =
                ::core::slice::from_raw_parts(__ix_data_ptr.add(1), __ix_data_len - 1);
        }
    } else {
        quote! {
            if __ix_data_len < 8 {
                return anchor_lang_v2::Error::from(
                    anchor_lang_v2::ErrorCode::InstructionFallbackNotFound,
                ).into();
            }
            let __disc: u64 = u64::from_le_bytes(
                *(__ix_data_ptr as *const [u8; 8])
            );
            let __ix_data: &[u8] =
                ::core::slice::from_raw_parts(__ix_data_ptr.add(8), __ix_data_len - 8);
        }
    };
    let event_cpi_dispatch = quote! {
        // Reserve the full event-CPI tag before user dispatch. A custom
        // 1-byte discriminator can overlap the first tag byte, but it
        // must not intercept self-CPI event instructions.
        if __ix_data_len >= 8 {
            let __event_disc: u64 = u64::from_le_bytes(
                *(__ix_data_ptr as *const [u8; 8])
            );
            if __event_disc == anchor_lang_v2::event::EVENT_IX_TAG {
                if __num < 1 {
                    return anchor_lang_v2::Error::from(
                        anchor_lang_v2::ErrorCode::AccountNotEnoughKeys,
                    ).into();
                }
                let mut __cursor = anchor_lang_v2::AccountCursor::new(__input, __lookup);
                let __event_authority = __cursor.next();
                if !__event_authority.is_signer() {
                    return anchor_lang_v2::Error::from(
                        anchor_lang_v2::ErrorCode::ConstraintSigner,
                    ).into();
                }
                let (__expected_event_authority, _) =
                    anchor_lang_v2::find_program_address(
                        &[b"__event_authority"],
                        __program_id,
                    );
                if !anchor_lang_v2::address_eq(
                    __event_authority.address(),
                    &__expected_event_authority,
                ) {
                    return anchor_lang_v2::Error::from(
                        anchor_lang_v2::ErrorCode::ConstraintSeeds,
                    ).into();
                }
                return 0;
            }
        }
    };

    // Strip #[discrim = N] attributes from handler outputs so rustc
    // doesn't complain about an unknown attribute.
    let handlers: Vec<_> = handlers
        .iter()
        .map(|func| {
            let mut func = (*func).clone();
            func.attrs.retain(|attr| {
                if let syn::Meta::NameValue(nv) = &attr.meta {
                    !nv.path.is_ident("discrim")
                } else {
                    true
                }
            });
            func
        })
        .collect();

    let interface_account_reexports = if config.mode == ProgramMode::Interface {
        quote! { pub use super::*; }
    } else {
        quote! {}
    };

    let cpi_cfg =
        (config.mode != ProgramMode::Interface).then(|| quote! { #[cfg(feature = "cpi")] });
    let client_interface = quote! {
        /// Client-side instruction structs for off-chain use.
        pub mod instruction {
            extern crate alloc;
            use super::*;
            use anchor_lang_v2::Discriminator as _;
            #(#instruction_structs)*
        }

        /// Client-side accounts structs (re-exports) for off-chain use.
        pub mod accounts {
            #interface_account_reexports
            #(#accounts_reexports)*
        }

        /// CPI module — gated on the `cpi` feature for normal programs and
        /// emitted unconditionally for interface modules.
        /// `accounts::*` re-exports the auto-generated CPI accounts structs
        /// (one per `#[derive(Accounts)]` Accounts struct, emitted as
        /// `__cpi_accounts_<lowercase>` at this same scope). The free
        /// functions wrap each instruction handler: pack args into the
        /// matching `instruction::<Camel>` struct, serialize, and dispatch
        /// through `CpiContext::invoke`.
        #cpi_cfg
        pub mod cpi {
            extern crate alloc;
            use super::*;
            use anchor_lang_v2::InstructionData as _;

            pub struct Return<T> {
                program: anchor_lang_v2::Address,
                phantom: core::marker::PhantomData<T>,
            }

            impl<T> Return<T>
            where
                T: for<'de> anchor_lang_v2::wincode::SchemaRead<
                    'de,
                    anchor_lang_v2::BorshConfig,
                    Dst = T,
                >,
            {
                pub fn get(&self) -> T {
                    let __return_data =
                        anchor_lang_v2::pinocchio::cpi::get_return_data().unwrap();
                    assert!(
                        anchor_lang_v2::address_eq(__return_data.program_id(), &self.program),
                        "return data program id mismatch"
                    );
                    anchor_lang_v2::wincode::config::deserialize(
                        __return_data.as_slice(),
                        anchor_lang_v2::BORSH_CONFIG,
                    )
                        .unwrap()
                }
            }

            pub mod accounts {
                #(#cpi_accounts_reexports)*
            }

            #(#cpi_wrappers)*
        }
    };

    if config.mode == ProgramMode::Interface {
        return quote! {
            #mod_vis mod #mod_name {
                #(#other_items)*
                #(#handlers)*
            }

            #client_interface
        };
    }

    quote! {
        #mod_vis mod #mod_name {
            #(#other_items)*
            #(#handlers)*
        }

        // Custom 2-arg (r1, r2) entrypoint using SIMD-0321 convention.
        #[cfg(not(feature = "no-entrypoint"))]
        anchor_lang_v2::pinocchio::default_allocator!();
        #[cfg(not(feature = "no-entrypoint"))]
        anchor_lang_v2::pinocchio::default_panic_handler!();

        /// Matches Solana's transaction-wide account cap (u8 index space).
        /// The lookup array holds `[AccountView; 256]` = ~2 KiB of frame
        /// used for duplicate-account resolution during cursor walks.
        const __ANCHOR_MAX_ACCOUNTS: usize = 256;

        /// Core dispatch: program-id check, discriminator parse, account
        /// cursor setup, handler dispatch. Exported as the `entrypoint`
        /// symbol unless `no-entrypoint` is active. Custom entrypoints
        /// can call this directly via its Rust path.
        ///
        /// The BPF loader passes:
        ///   r1 = MM_INPUT_START (first byte of the serialized parameter region)
        ///   r2 = VM address of the instruction data bytes (SIMD-0321)
        ///
        /// The `[r2 - 8 .. r2]` slot holds the `u64` ix_data length and the
        /// 32 bytes at `[r2 + len .. +32]` hold the program_id, per agave's
        /// aligned serialization layout (see `solana-program-runtime
        /// ::serialization::serialize_parameters_aligned`).
        // Default path: export as the BPF loader's `entrypoint` symbol so
        // this IS the program entrypoint.
        //
        // `no-entrypoint` path: export as `__anchor_dispatch` so a custom
        // entrypoint (e.g. `global_asm!` writing its own `.globl entrypoint`)
        // can either (a) call this from Rust via its module path, or
        // (b) tail-call it from asm via the unmangled linker symbol.
        //
        // Both exports are gated on `target_os = "solana"`: the symbol names
        // only matter to the SBF linker, and un-mangling on host would cause
        // duplicate-symbol errors whenever two `no-entrypoint` crates end up
        // in the same host-link (e.g. `cargo test` pulling in multiple `cpi`
        // deps).
        #[cfg_attr(
            all(target_os = "solana", not(feature = "no-entrypoint")),
            export_name = "entrypoint"
        )]
        #[cfg_attr(all(target_os = "solana", feature = "no-entrypoint"), no_mangle)]
        pub unsafe extern "C" fn __anchor_dispatch(
            __input: *mut u8,
            __ix_data_ptr: *const u8,
        ) -> u64 {
            let mut __lookup: [::core::mem::MaybeUninit<anchor_lang_v2::AccountView>;
                __ANCHOR_MAX_ACCOUNTS] =
                [const { ::core::mem::MaybeUninit::uninit() }; __ANCHOR_MAX_ACCOUNTS];

            __anchor_dispatch_internal(
                __input,
                __ix_data_ptr,
                __lookup.as_mut_ptr() as *mut anchor_lang_v2::AccountView,
            )
        }

        #[inline(never)]
        unsafe fn __anchor_dispatch_internal(
            __input: *mut u8,
            __ix_data_ptr: *const u8,
            __lookup: *mut anchor_lang_v2::AccountView,
        ) -> u64 {
            let __ix_data_len = *(__ix_data_ptr.sub(8) as *const u64) as usize;
            let __program_id: &anchor_lang_v2::Address =
                &*(__ix_data_ptr.add(__ix_data_len) as *const anchor_lang_v2::Address);

            if let Err(__e) = anchor_lang_v2::check_program_id(__program_id, &crate::ID) {
                return __e.into();
            }

            let __num = *(__input as *const u64) as usize;
            #event_cpi_dispatch

            // Parse the discriminator.
            #disc_parse

            let mut __cursor = anchor_lang_v2::AccountCursor::new(__input, __lookup);

            // Each dispatch arm returns u64 directly (0 = success).
            match __disc {
                #(#dispatch_arms)*
                _ => anchor_lang_v2::Error::from(
                    anchor_lang_v2::ErrorCode::InstructionFallbackNotFound,
                ).into(),
            }
        }

        mod __handlers {
            use super::*;
            use anchor_lang_v2::TryAccounts as _;
            #(#handler_wrappers)*
        }

        #client_interface

        // IDL generation: prints structured output consumed by `anchor idl build`.
        // The CLI runs `cargo test __anchor_private_print_idl --features idl-build`
        // and parses the marker-delimited sections from stdout.
        #[cfg(all(test, feature = "idl-build"))]
        mod __anchor_private_idl {
            use super::*;

            #[test]
            fn __anchor_private_print_idl_address() {
                println!("--- IDL begin address ---");
                let addr = crate::ID;
                // Print base58 address
                println!("{}", anchor_lang_v2::Address::from(addr));
                println!("--- IDL end address ---");
            }

            #[test]
            fn __anchor_private_print_idl_program() {
                let instructions = vec![
                    #(
                        format!(
                            "{{\"name\":\"{}\"{},\"discriminator\":{},\"accounts\":{},\"args\":{}{} }}",
                            #idl_ix_names,
                            #idl_ix_docs,
                            #idl_ix_discs,
                            #idl_accounts_types::__idl_accounts(),
                            #idl_ix_args,
                            #idl_ix_returns,
                        )
                    ),*
                ];

                // Collect accounts + types from every Accounts struct field
                // via the transitive dep walker, plus from every handler's
                // ix arg types. `__register_idl_deps` pushes pre-split
                // strings — `__IDL_ACCOUNT_ENTRY` into the accounts buffer,
                // `__IDL_TYPE_DEF` into the types buffer — so the print
                // test just dedupes + joins. No runtime JSON parsing.
                //
                // View wrappers (Signer, Program<T>, Sysvar<T>, …) use
                // the trait's default no-op `__register_idl_deps`, so they
                // contribute nothing. Primitive / collection / `&T`
                // blanket impls are no-op forwarders (`idl_build.rs`),
                // so only user-derived `#[account]` / `#[event]` /
                // `IdlType` structs contribute.
                //
                // Walking ix arg types matters because a user struct
                // referenced only as a `#[program]` fn argument (e.g.
                // `args: MixedArgs<'_>`) otherwise lands in
                // `instructions[].args` as a bare `{defined:{name:...}}`
                // reference with no matching `types[]` entry.
                let mut accounts_entries: Vec<&'static str> = Vec::new();
                let mut types_entries: Vec<&'static str> = Vec::new();
                #(
                    #idl_accounts_types::__idl_register_deps(
                        &mut accounts_entries,
                        &mut types_entries,
                    );
                )*
                #(
                    #(
                        <#ix_arg_types_per_handler as anchor_lang_v2::IdlAccountType>::__register_idl_deps(
                            &mut accounts_entries,
                            &mut types_entries,
                        );
                    )*
                )*
                #(#ix_return_type_registers)*
                accounts_entries.sort();
                accounts_entries.dedup();
                types_entries.sort();
                types_entries.dedup();

                let crate_name = env!("CARGO_CRATE_NAME").replace('-', "_");
                // Pull `description` / `repository` from the program crate's
                // Cargo.toml via `option_env!`. Cargo sets `CARGO_PKG_*` when
                // it invokes the compiler; unset or empty strings get dropped
                // from the JSON (the spec marks both fields
                // `skip_serializing_if` in `IdlMetadata`). `dependencies` stays
                // unimplemented — it needs a workspace traversal v1 handles
                // via a separate tool.
                let description = option_env!("CARGO_PKG_DESCRIPTION")
                    .filter(|s| !s.is_empty());
                let repository = option_env!("CARGO_PKG_REPOSITORY")
                    .filter(|s| !s.is_empty());
                let mut metadata_extras = anchor_lang_v2::__alloc::string::String::new();
                if let Some(d) = description {
                    // Escape embedded quotes/backslashes so the JSON stays valid.
                    let escaped = d.replace('\\', "\\\\").replace('"', "\\\"");
                    metadata_extras.push_str(&anchor_lang_v2::__alloc::format!(
                        ",\"description\":\"{}\"",
                        escaped,
                    ));
                }
                if let Some(r) = repository {
                    let escaped = r.replace('\\', "\\\\").replace('"', "\\\"");
                    metadata_extras.push_str(&anchor_lang_v2::__alloc::format!(
                        ",\"repository\":\"{}\"",
                        escaped,
                    ));
                }
                let idl = format!(
                    "{{\"address\":\"\",\"metadata\":{{\"name\":\"{}\",\"version\":\"0.1.0\",\"spec\":\"0.1.0\"{}}},\"instructions\":[{}],\"accounts\":[{}],\"types\":[{}]}}",
                    crate_name,
                    metadata_extras,
                    instructions.join(","),
                    accounts_entries.join(","),
                    types_entries.join(","),
                );
                println!("--- IDL begin program ---");
                println!("{}", idl);
                println!("--- IDL end program ---");
            }
        }
    }
}

// ---------------------------------------------------------------------------
// #[event]
// ---------------------------------------------------------------------------

/// Attribute macro that marks a struct as an event.
///
/// Two modes:
///
/// **Default (`#[event]`, wincode).** Derives `wincode::SchemaWrite` and
/// serializes via `wincode::config::serialize_into` with `BORSH_CONFIG`, so
/// the on-chain wire format is byte-compatible with borsh while keeping
/// wincode's faster encoding path. Supports arbitrary layouts, including
/// `Vec`/`String`/`Option`/enums, and is materially cheaper than borsh on
/// SBF (see `cu-bench` — roughly 3–10× fewer CUs depending on shape). This
/// is the right default for almost every event.
///
/// **`#[event(bytemuck)]`.** Emits `#[repr(C)]` + a raw `copy_nonoverlapping`
/// of the struct bytes. Fastest of the two for fixed-size events, but the
/// struct must contain only fixed-size, non-fat-pointer fields (no
/// `Vec`/`String`/`Box`/`Option`/enums/maps) and must have zero `repr(C)`
/// padding. Both invariants are enforced at compile time. Opt into this only
/// when the event is on a hot path and profiling shows wincode's per-field
/// encoding is the bottleneck.
///
/// Both modes share the same discriminator and `Event::data()` contract,
/// so `emit!` works identically. The IDL carries a `serialization` tag so TS
/// clients know how to decode.
///
/// # Examples
///
/// ```ignore
/// // Default: wincode (borsh-wire-compatible), supports dynamic fields.
/// #[event]
/// pub struct DepositRecorded {
///     pub ledger: [u8; 32],
///     pub amount: u64,
///     pub memo: String,
/// }
///
/// // Opt-in bytemuck: cheapest on fixed-size hot-path events.
/// #[event(bytemuck)]
/// pub struct TickUpdate {
///     pub price: u64,
///     pub ts: u64,
/// }
/// ```
#[proc_macro_attribute]
pub fn event(attr: TokenStream, item: TokenStream) -> TokenStream {
    let mode = match parse_event_mode(attr) {
        Ok(mode) => mode,
        Err(err) => return err.to_compile_error().into(),
    };

    let input = parse_macro_input!(item as DeriveInput);
    let name = input.ident.clone();
    let name_str = name.to_string();
    let vis = &input.vis;
    let attrs = &input.attrs;
    let fields = match &input.data {
        Data::Struct(data) => &data.fields,
        _ => {
            return syn::Error::new(name.span(), "`#[event]` only supports structs")
                .to_compile_error()
                .into()
        }
    };

    use sha2::Digest;
    let hash = sha2::Sha256::digest(format!("event:{name_str}").as_bytes());
    let disc_bytes = &hash[..8];
    let disc_literals: Vec<_> = disc_bytes.iter().map(|b| quote! { #b }).collect();

    let discriminator_impl = quote! {
        impl anchor_lang_v2::Discriminator for #name {
            const DISCRIMINATOR: &'static [u8] = &[#(#disc_literals),*];
        }
    };

    // Build the `--- IDL begin event ---` payload. `idl/src/build.rs` expects
    // a JSON object with `event: IdlEvent` (name + discriminator) plus
    // `types: Vec<IdlTypeDef>` (the full struct definition). The default
    // wincode mode tags its IDL entry as borsh-serialized (the wire format is
    // borsh-compatible via `BORSH_CONFIG`); bytemuck adds
    // `{serialization:"bytemuck",repr:{kind:"c"}}`.
    let type_kind = match mode {
        EventMode::Wincode => idl::TypeKind::Borsh,
        EventMode::Bytemuck => idl::TypeKind::BytemuckRepr,
    };
    let struct_docs = idl::extract_doc_lines(attrs);
    let event_type_strings = if let Fields::Named(named) = fields {
        idl::build_type_strings(&name_str, disc_bytes, &struct_docs, &named.named, type_kind)
    } else {
        idl::build_type_strings(
            &name_str,
            disc_bytes,
            &struct_docs,
            &syn::punctuated::Punctuated::new(),
            type_kind,
        )
    };
    let event_type_def = event_type_strings.type_def;
    let event_disc_json = idl::disc_json(disc_bytes);
    let event_header_json = format!(
        "{{\"event\":{{\"name\":\"{}\",\"discriminator\":{}}},\"types\":[",
        name_str, event_disc_json,
    );
    // Field types for the transitive type walk. The event itself pushes
    // `type_def_json` into the types accumulator via its `__IDL_TYPE_DEF`
    // const; field-type deps fan out from there so plain user structs
    // referenced by `pub inner: Inner` land in `types[]` too.
    let idl_field_tys: Vec<&Type> = match fields {
        Fields::Named(named) => named.named.iter().map(|f| &f.ty).collect(),
        Fields::Unnamed(unnamed) => unnamed.unnamed.iter().map(|f| &f.ty).collect(),
        Fields::Unit => Vec::new(),
    };
    let idl_fn_name = quote::format_ident!(
        "__anchor_private_print_idl_event_{}",
        name_str.to_lowercase()
    );
    // The event's own `IdlAccountType` impl (emitted further down) owns
    // `__IDL_TYPE_DEF = Some(#type_def_json)` and a `__register_idl_deps` that
    // pushes both self and every field-type dep. The test here just walks
    // that deps list at runtime and assembles the `--- IDL begin event ---`
    // payload — so `types[]` picks up nested user structs without the test
    // having to know about them.
    let idl_event_print = quote! {
        #[cfg(all(test, feature = "idl-build"))]
        #[test]
        fn #idl_fn_name() {
            // Events don't appear in the program-level `accounts[]` array
            // (the event's own discriminator lives in `event_header_json`
            // below), so the accounts buffer here is a sink that's never
            // serialized — we still pass it because `__register_idl_deps`
            // takes both buffers, and a nested `#[account]` data type
            // referenced from an event payload would push into it.
            let mut __accounts: anchor_lang_v2::__alloc::vec::Vec<&'static str> =
                anchor_lang_v2::__alloc::vec::Vec::new();
            let mut __types: anchor_lang_v2::__alloc::vec::Vec<&'static str> =
                anchor_lang_v2::__alloc::vec::Vec::new();
            <#name as anchor_lang_v2::IdlAccountType>::__register_idl_deps(
                &mut __accounts,
                &mut __types,
            );
            __types.sort();
            __types.dedup();
            let mut __payload = anchor_lang_v2::__alloc::string::String::from(#event_header_json);
            let mut __first = true;
            for __t in &__types {
                if !__first { __payload.push(','); }
                __first = false;
                __payload.push_str(__t);
            }
            __payload.push_str("]}");
            println!("--- IDL begin event ---");
            println!("{}", __payload);
            println!("--- IDL end event ---");
        }
    };
    // Emit the `IdlAccountType` impl for the event itself. Walks field
    // types for transitive dep registration. Events don't surface in the
    // program-level `accounts[]` array — `__IDL_ACCOUNT_ENTRY` defaults
    // to `None`. Their discriminator lives in `event_header_json` (the
    // `--- IDL begin event ---` payload prefix), not in `__IDL_TYPE_DEF`.
    let idl_account_type_impl = quote! {
        #[cfg(feature = "idl-build")]
        #[doc(hidden)]
        impl anchor_lang_v2::IdlAccountType for #name {
            const __IDL_TYPE_DEF: Option<&'static str> = Some(#event_type_def);
            fn __register_idl_deps(
                accounts: &mut ::anchor_lang_v2::__alloc::vec::Vec<&'static str>,
                types: &mut ::anchor_lang_v2::__alloc::vec::Vec<&'static str>,
            ) {
                if let Some(t) = <Self as anchor_lang_v2::IdlAccountType>::__IDL_TYPE_DEF {
                    types.push(t);
                }
                #(
                    <#idl_field_tys as anchor_lang_v2::IdlAccountType>::__register_idl_deps(accounts, types);
                )*
            }
        }
    };

    match mode {
        EventMode::Wincode => TokenStream::from(quote! {
            // `#[derive(wincode::SchemaWrite)]` lays down the per-field encoder.
            // No `repr(C)` — wincode is layout-agnostic (it walks the derived
            // schema, not the in-memory byte layout) so the compiler is free
            // to pick whichever Rust layout is best.
            #[derive(anchor_lang_v2::wincode::SchemaWrite)]
            #(#attrs)*
            #vis struct #name #fields

            #discriminator_impl

            impl anchor_lang_v2::Event for #name {
                fn data(&self) -> anchor_lang_v2::__alloc::vec::Vec<u8> {
                    let disc = <Self as anchor_lang_v2::Discriminator>::DISCRIMINATOR;
                    // 256-byte preallocation matches instruction-data
                    // emission. Wincode has no `encoded_size()` yet, so this
                    // is a best-guess that avoids a reallocation for typical
                    // event shapes.
                    let mut buf = anchor_lang_v2::__alloc::vec::Vec::with_capacity(
                        disc.len() + 256,
                    );
                    buf.extend_from_slice(disc);
                    anchor_lang_v2::wincode::config::serialize_into(
                        &mut buf,
                        self,
                        anchor_lang_v2::BORSH_CONFIG,
                    )
                        .expect("`#[event]` wincode serialization cannot fail for \
                                 derived SchemaWrite types");
                    buf
                }
            }

            #idl_account_type_impl

            #idl_event_print
        }),
        EventMode::Bytemuck => {
            let field_types: Vec<_> = fields.iter().map(|f| &f.ty).collect();

            // Targeted diagnostics for common non-Pod field types. Fires
            // *before* the generic `assert_pod::<T>` bound so users hit a
            // field-specific migration hint instead of the opaque
            // `Vec<u8>: Pod is not satisfied` error. Mirrors the pattern in
            // `#[account]` zero-copy codegen. Borsh mode is suggested here
            // because (unlike `#[account]`) events have a correct dynamic
            // fallback — see `diagnose_non_pod_event_field`.
            let field_diagnostics: Vec<_> = fields
                .iter()
                .filter_map(|field| {
                    let field_name = field
                        .ident
                        .as_ref()
                        .map(|i| i.to_string())
                        .unwrap_or_default();
                    let msg = diagnose_non_pod_event_field(&field.ty, &field_name)?;
                    Some(quote! { ::core::compile_error!(#msg); })
                })
                .collect();

            // Transitive Pod bound per field. Catches any fat-pointer or
            // uninit-byte-containing type, including ones hidden inside user-
            // defined structs — `bytemuck::Pod` is recursively checked at the
            // bound site, so `struct User { v: Vec<u8> }` fails here even
            // though the derive macro can't see through `User`.
            //
            // Padding check: `repr(C)` inserts alignment padding between
            // fields of differing alignment. Padding bytes are uninitialized,
            // which violates `Pod`'s all-bytes-initialized requirement and
            // would also silently drift from a borsh-decoded client view.
            // The assertion tells the author how to fix it.
            //
            // Finally, `unsafe impl Pod + Zeroable for Self` so consumers can
            // `bytemuck::from_bytes` the logged payload directly — mirrors
            // `#[account]`'s zero-copy output shape.
            TokenStream::from(quote! {
                #[repr(C)]
                #[derive(::core::clone::Clone, ::core::marker::Copy)]
                #(#attrs)*
                #vis struct #name #fields

                // Targeted diagnostics fire first so users see a specific
                // migration hint (e.g. "drop `bytemuck` for dynamic strings")
                // instead of bytemuck's opaque `Pod not satisfied`.
                #(#field_diagnostics)*

                // Transitive Pod bound per field — catches fat pointers even
                // when hidden inside an opaque user-defined struct (the
                // `Pod` trait propagates through nested types).
                const _: fn() = || {
                    fn assert_pod<T: anchor_lang_v2::bytemuck::Pod>() {}
                    #( assert_pod::<#field_types>(); )*
                };

                // `repr(C)` padding is target-dependent: on SBF `u128` is
                // align-8, so a `{Address (align 1), u64, u128}` struct has
                // no padding; on x86_64 hosts `u128` is align-16, inserting
                // a phantom 8-byte gap before the `u128`. Gating on the
                // Solana target means `cargo check` accepts the struct
                // based on BPF layout (the only layout that actually
                // ships) and `cargo build-sbf` still enforces the no-
                // padding invariant.
                #[cfg(target_os = "solana")]
                const _: () = ::core::assert!(
                    ::core::mem::size_of::<#name>()
                        == 0 #( + ::core::mem::size_of::<#field_types>() )*,
                    "`#[event]` struct has `repr(C)` alignment padding — \
                     reorder fields from largest to smallest alignment (u128/u64 \
                     first, then u32, then u16, then u8/bool), or drop the \
                     `bytemuck` flag and use the default `#[event]` (wincode) \
                     for arbitrary layouts"
                );

                // SAFETY: `bytemuck::Pod` requires four invariants. Each is
                // proven by a compile-time check earlier in this block:
                //
                //   (1) `#[repr(C)]`                     — enforced by the
                //       `#[repr(C)]` attribute emitted above.
                //   (2) Every field is `Pod`             — enforced by the
                //       `assert_pod::<T>()` ghost fn. Failure is
                //       `T: Pod is not satisfied`, which transitively
                //       rejects fat pointers (`Vec`, `String`, `Box`, `&T`),
                //       uninit-byte types (`bool`, enums, `Option`), and
                //       any user struct that itself isn't `Pod`.
                //   (3) No interior padding bytes        — enforced by the
                //       `size_of::<Self>() == Σ size_of::<Field>()` assert
                //       under `cfg(target_os = "solana")`. Padding bytes
                //       are `MaybeUninit`, which would be read by
                //       `bytemuck::bytes_of` / `bytemuck::from_bytes` and
                //       constitute UB — the assert precludes the case.
                //   (4) `Copy` + `'static`               — `Copy` is
                //       derived above; `'static` is required by
                //       `assert_pod::<T: 'static>` transitively.
                //
                // The cfg-gated padding assert deliberately only evaluates
                // on the Solana target. `repr(C)` padding is target-
                // dependent: on SBF `u128` is align-8, so `{Address (align
                // 1), u64, u128}` is perfectly packed; on x86_64 hosts
                // `u128` is align-16, so that same layout has a phantom
                // 8-byte gap during `cargo check`. This is not a soundness
                // hole — the event bytes only get memcpy'd into
                // `sol_log_data` on the actual deployment target, where
                // the assert does run.
                //
                // Not using `#[derive(Pod)]` because bytemuck's own
                // padding check is unconditional (not target-gated) and
                // would reject `u128`-carrying events on host compile even
                // though their on-chain layout is sound.
                unsafe impl anchor_lang_v2::bytemuck::Pod for #name {}
                unsafe impl anchor_lang_v2::bytemuck::Zeroable for #name {}

                #discriminator_impl

                impl anchor_lang_v2::Event for #name {
                    fn data(&self) -> anchor_lang_v2::__alloc::vec::Vec<u8> {
                        const SIZE: usize = ::core::mem::size_of::<#name>();
                        let disc = <Self as anchor_lang_v2::Discriminator>::DISCRIMINATOR;
                        let mut buf = anchor_lang_v2::__alloc::vec::Vec::with_capacity(
                            disc.len() + SIZE,
                        );
                        buf.extend_from_slice(disc);
                        let start = buf.len();
                        buf.resize(start + SIZE, 0);
                        unsafe {
                            ::core::ptr::copy_nonoverlapping(
                                self as *const Self as *const u8,
                                buf.as_mut_ptr().add(start),
                                SIZE,
                            );
                        }
                        buf
                    }
                }

                #idl_account_type_impl

                #idl_event_print
            })
        }
    }
}

enum EventMode {
    Wincode,
    Bytemuck,
}

/// Parse the `#[account]` attribute's optional mode argument.
///
/// Accepts: `#[account]` (default → zero-copy Pod) or `#[account(borsh)]`.
/// Under the hood the borsh mode encodes/decodes via wincode using
/// `BORSH_CONFIG`, so the on-chain bytes still match what a borsh library
/// would produce, while paying for wincode's faster encode/decode path.
fn parse_account_mode(attr: TokenStream) -> Result<bool, syn::Error> {
    if attr.is_empty() {
        return Ok(false);
    }
    let attr2: proc_macro2::TokenStream = attr.into();
    let ident: syn::Ident = syn::parse2(attr2.clone()).map_err(|_| {
        syn::Error::new_spanned(
            &attr2,
            "expected `#[account]` or `#[account(borsh)]` — no other arguments are supported",
        )
    })?;
    if ident == "borsh" {
        Ok(true)
    } else {
        Err(syn::Error::new_spanned(
            ident,
            "unknown `#[account]` mode — only `borsh` is accepted",
        ))
    }
}

fn parse_event_mode(attr: TokenStream) -> Result<EventMode, syn::Error> {
    if attr.is_empty() {
        return Ok(EventMode::Wincode);
    }
    let attr2: proc_macro2::TokenStream = attr.into();
    let ident: syn::Ident = syn::parse2(attr2.clone()).map_err(|_| {
        syn::Error::new_spanned(
            &attr2,
            "expected `#[event]` or `#[event(bytemuck)]` — no other arguments are supported",
        )
    })?;
    if ident == "bytemuck" {
        Ok(EventMode::Bytemuck)
    } else {
        Err(syn::Error::new_spanned(
            ident,
            "unknown `#[event]` mode — only `bytemuck` is accepted",
        ))
    }
}

/// Targeted diagnostics for common non-Pod field types on
/// `#[event(bytemuck)]` structs. Bytemuck mode is strict by design —
/// `Vec`/`String`/`Option`/etc. can't round-trip through a `copy_nonoverlapping`
/// of the struct bytes. The hint steers authors to drop the `bytemuck` flag
/// (getting the wincode default, which handles these fine). Returns `None`
/// for types we can't recognize by name — the `assert_pod::<T>` bound
/// catches those generically via `bytemuck::Pod`.
fn diagnose_non_pod_event_field(ty: &Type, field_name: &str) -> Option<String> {
    let Type::Path(tp) = ty else { return None };
    let seg = tp.path.segments.last()?;
    let ident = seg.ident.to_string();
    match ident.as_str() {
        "Vec" => Some(format!(
            "event field `{field_name}` uses `Vec`, which is a fat pointer — the \
             `#[event(bytemuck)]` memcpy path would emit the `(ptr, len, cap)` bits instead of \
             the elements. Use `[T; N]` for a fixed-size array, or drop the `bytemuck` attribute \
             to use the default wincode encoding, which handles `Vec` natively."
        )),
        "String" => Some(format!(
            "event field `{field_name}` uses `String`, which is a fat pointer — the \
             `#[event(bytemuck)]` memcpy path would emit the `(ptr, len, cap)` bits instead of \
             the UTF-8 bytes. Use `[u8; N]` for a bounded buffer, or drop the `bytemuck` \
             attribute to use the default wincode encoding, which handles `String` natively."
        )),
        "Option" => Some(format!(
            "event field `{field_name}` uses `Option`, whose niche-or-tag layout isn't guaranteed \
             to match the client decoder. Use a sentinel value (e.g. an all-zero `[u8; 32]` for \
             \"no address\"), or drop the `bytemuck` attribute to use the default wincode \
             encoding."
        )),
        "Box" | "Rc" | "Arc" | "Cow" | "Weak" => Some(format!(
            "event field `{field_name}` uses `{ident}`, which is a heap/shared pointer — its \
             bytes are a pointer, not the referenced data. Inline the value directly (`T` instead \
             of `{ident}<T>`), or drop the `bytemuck` attribute to use the default wincode \
             encoding."
        )),
        "HashMap" | "HashSet" | "BTreeMap" | "BTreeSet" | "BinaryHeap" | "LinkedList"
        | "VecDeque" => Some(format!(
            "event field `{field_name}` uses `{ident}`, which allocates on the heap. Drop the \
             `bytemuck` attribute to use the default wincode encoding, which handles dynamic \
             collections."
        )),
        "bool" => Some(format!(
            "event field `{field_name}` is `bool`. `bytemuck` disallows `bool` as Pod because \
             only `0x00` and `0x01` are valid — any other byte is UB. Use a `u8` and treat `0` / \
             non-zero as the boolean, or drop the `bytemuck` attribute to use the default wincode \
             encoding."
        )),
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// emit!
// ---------------------------------------------------------------------------

/// Logs an event that can be subscribed to by clients.
///
/// Uses the `sol_log_data` syscall which emits a `Program data: <Base64>` log.
///
/// # Example
///
/// ```ignore
/// emit!(DepositRecorded { ledger: *ctx.accounts.ledger.account().address(), amount });
/// ```
#[proc_macro]
pub fn emit(input: TokenStream) -> TokenStream {
    let data: proc_macro2::TokenStream = input.into();
    TokenStream::from(quote! {
        {
            anchor_lang_v2::sol_log_data(&[&anchor_lang_v2::Event::data(&#data)]);
        }
    })
}

/// Emits an event via self-CPI so indexers can recover it from instruction data
/// instead of logs. Requires `#[event_cpi]` on the handler's accounts struct
/// and a local handler variable named `ctx`, matching Anchor v1.
#[proc_macro]
pub fn emit_cpi(input: TokenStream) -> TokenStream {
    let event_struct: proc_macro2::TokenStream = input.into();
    TokenStream::from(quote! {
        {
            struct __AnchorEventCpiAccounts<'a> {
                event_authority: anchor_lang_v2::CpiHandle<'a>,
            }

            impl<'a> anchor_lang_v2::ToCpiAccounts<'a> for __AnchorEventCpiAccounts<'a> {
                fn to_instruction_accounts(
                    &self,
                ) -> anchor_lang_v2::__alloc::vec::Vec<
                    anchor_lang_v2::pinocchio::instruction::InstructionAccount<'a>
                > {
                    let mut __accounts = anchor_lang_v2::__alloc::vec::Vec::with_capacity(1);
                    __accounts.push(
                        anchor_lang_v2::pinocchio::instruction::InstructionAccount::readonly_signer(
                            self.event_authority.address(),
                        ),
                    );
                    __accounts
                }

                fn to_cpi_handles(
                    &self,
                ) -> anchor_lang_v2::__alloc::vec::Vec<anchor_lang_v2::CpiHandle<'a>> {
                    let mut __handles = anchor_lang_v2::__alloc::vec::Vec::with_capacity(1);
                    __handles.push(self.event_authority);
                    __handles
                }
            }

            let __event_authority =
                anchor_lang_v2::AnchorAccount::cpi_handle(&ctx.accounts.event_authority);
            let __event_data = anchor_lang_v2::Event::data(&#event_struct);
            let mut __ix_data = anchor_lang_v2::__alloc::vec::Vec::with_capacity(
                anchor_lang_v2::event::EVENT_IX_TAG_LE.len() + __event_data.len(),
            );
            __ix_data.extend_from_slice(anchor_lang_v2::event::EVENT_IX_TAG_LE);
            __ix_data.extend_from_slice(&__event_data);

            let __event_authority_bump = [ctx.bumps.event_authority];
            let __event_authority_seeds: &[&[u8]] =
                &[b"__event_authority", __event_authority_bump.as_ref()];
            let __event_authority_signers: &[&[&[u8]]] = &[__event_authority_seeds];
            anchor_lang_v2::CpiContext::new_with_signer(
                ctx.program_id,
                __AnchorEventCpiAccounts { event_authority: __event_authority },
                __event_authority_signers,
            ).invoke(&__ix_data);
        }
    })
}

/// Adds the self-CPI event authority accounts expected by `emit_cpi!`.
///
/// The injected shape intentionally mirrors Anchor v1's user-facing API, but
/// uses a normal v2 seeds constraint so `ctx.bumps.event_authority` is available
/// without making `declare_id!` synthesize an extra constant.
#[proc_macro_attribute]
pub fn event_cpi(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let accounts_struct = parse_macro_input!(input as ItemStruct);
    let ItemStruct {
        attrs,
        vis,
        struct_token,
        ident,
        generics,
        fields,
        semi_token,
    } = accounts_struct;

    if semi_token.is_some() {
        return syn::Error::new(
            ident.span(),
            "`#[event_cpi]` only supports accounts structs with named fields",
        )
        .to_compile_error()
        .into();
    }

    let fields = match fields {
        Fields::Named(fields) => fields.named,
        _ => {
            return syn::Error::new(
                ident.span(),
                "`#[event_cpi]` only supports accounts structs with named fields",
            )
            .to_compile_error()
            .into()
        }
    };

    TokenStream::from(quote! {
        #(#attrs)*
        #vis #struct_token #ident #generics {
            #fields

            /// CHECK: Only the event authority can invoke self-CPI
            #[account(seeds = [b"__event_authority"], bump)]
            pub event_authority: anchor_lang_v2::accounts::UncheckedAccount,
            /// CHECK: Kept for v1-compatible account ordering and IDL shape
            pub program: anchor_lang_v2::accounts::UncheckedAccount,
        }
    })
}

// ---------------------------------------------------------------------------
// #[access_control]
// ---------------------------------------------------------------------------

/// Executes the given access control method before running the decorated
/// instruction handler. Any method in scope of the attribute can be invoked
/// with any arguments from the associated instruction handler.
///
/// # Example
///
/// ```ignore
/// #[program]
/// mod errors {
///     use super::*;
///
///     #[access_control(Create::validate(&ctx, bump_seed))]
///     pub fn create(ctx: &mut Context<Create>, bump_seed: u8) -> Result<()> {
///         ctx.accounts.my_account.bump_seed = bump_seed;
///         Ok(())
///     }
/// }
///
/// impl Create {
///     pub fn validate(ctx: &Context<Create>, bump_seed: u8) -> Result<()> {
///         // ... custom validation ...
///         Ok(())
///     }
/// }
/// ```
///
/// This pattern is useful for invariants that do not fit naturally in an
/// account attribute, or for checks that should live next to ordinary Rust
/// validation code.
#[proc_macro_attribute]
pub fn access_control(args: TokenStream, input: TokenStream) -> TokenStream {
    access_control::expand(args, input)
}

// ---------------------------------------------------------------------------
// #[constant]
// ---------------------------------------------------------------------------

/// Marker attribute for `pub const` items that should appear in the generated
/// IDL. Does nothing at runtime. When the `idl-build` feature is enabled, a
/// companion test function emits the constant's metadata for `anchor idl build`.
///
/// # Example
///
/// ```ignore
/// #[constant]
/// pub const SEED: &str = "anchor";
/// ```
#[proc_macro_attribute]
pub fn constant(_attr: TokenStream, input: TokenStream) -> TokenStream {
    constant::expand(input)
}

// ---------------------------------------------------------------------------
// #[derive(InitSpace)]
// ---------------------------------------------------------------------------

/// Implements [`anchor_lang_v2::Space`] on the decorated struct or enum so
/// users can write `space = 8 + MyAccount::INIT_SPACE` in `#[account(init)]`.
///
/// Variable-size fields (`String`, `Vec<T>`) require a `#[max_len(N)]` helper
/// attribute to specify the reserved capacity.
///
/// # Example
///
/// ```ignore
/// #[account(borsh)]
/// #[derive(InitSpace)]
/// pub struct Profile {
///     pub owner: Address,
///     #[max_len(32)]
///     pub name: String,
/// }
/// ```
#[proc_macro_derive(InitSpace, attributes(max_len))]
pub fn derive_init_space(item: TokenStream) -> TokenStream {
    init_space::expand(item)
}

// ---------------------------------------------------------------------------
// #[pod_wrapper]
// ---------------------------------------------------------------------------

/// Generates a `Pod`-compatible companion type for an `#[repr(u8)]` enum.
///
/// Rust enums are not `bytemuck::Pod` — only declared discriminants round-trip
/// safely, whereas `Pod` requires every bit pattern to be valid. Storing an
/// enum inside a zero-copy `#[account]` struct is therefore unsound: a corrupt
/// byte becomes an invalid enum value and instant UB on pattern match.
///
/// `#[pod_wrapper]` emits a `#[repr(transparent)] struct Pod{Enum}(pub u8)`
/// with `Pod + Zeroable` impls, per-variant associated constants, and `From` /
/// `PartialEq` bridges so existing `engine.market_mode == MarketMode::Live`
/// comparisons still compile after swapping a field from `Enum` to `PodEnum`.
/// It is an attribute macro — not a derive — so the companion can also carry
/// trait impls (e.g. `IdlAccountType`) that derives cannot express.
///
/// # Example
///
/// ```ignore
/// #[pod_wrapper]
/// #[derive(Copy, Clone, PartialEq, Eq, Debug)]
/// #[repr(u8)]
/// pub enum MarketMode { Live = 0, Resolved = 1 }
///
/// // generated: PodMarketMode::Live, PodMarketMode::Resolved
/// // generated: From<MarketMode> / From<PodMarketMode> (panics on invalid byte)
/// // generated: PartialEq<MarketMode> / PartialEq<PodMarketMode> bridges
/// ```
///
/// # Requirements
///
/// * The annotated item must be an `enum`.
/// * The enum must carry `#[repr(u8)]` so the stored width is explicit.
/// * Every variant must be a bare unit variant (no payload).
#[proc_macro_attribute]
pub fn pod_wrapper(_attr: TokenStream, item: TokenStream) -> TokenStream {
    pod_wrapper::expand(item)
}

// ---------------------------------------------------------------------------
// #[error_code]
// ---------------------------------------------------------------------------

/// Port of v1's `#[error_code]` without the runtime `AnchorError` heap
/// allocations. Emits `impl From<E> for Error` returning
/// `Error::Custom(variant_index + offset)`. `#[msg("text")]` is IDL-only.
///
/// # Example
///
/// ```ignore
/// #[error_code]
/// pub enum MyError {
///     #[msg("invalid threshold")]
///     InvalidThreshold,
///     TooManySigners,
/// }
///
/// // usage:
/// return Err(MyError::InvalidThreshold.into());
/// ```
///
/// Supports `#[error_code(offset = N)]` for the first code (default 6000).
#[proc_macro_attribute]
pub fn error_code(args: TokenStream, input: TokenStream) -> TokenStream {
    error_code::expand(args, input)
}

/// Parse the optional `#[discrim = ...]` attribute on a handler fn.
/// Returns `Ok(Some(...))` if present, `Ok(None)` if absent,
/// or `Err` with a properly-spanned diagnostic on malformed input.
fn parse_discrim_attr(handler: &syn::ItemFn) -> syn::Result<Option<DiscrimAttr>> {
    for attr in &handler.attrs {
        if let syn::Meta::NameValue(nv) = &attr.meta {
            if nv.path.is_ident("discrim") {
                let span = nv.value.span();
                if let syn::Expr::Lit(syn::ExprLit {
                    lit: syn::Lit::Int(lit),
                    ..
                }) = &nv.value
                {
                    let byte = lit.base10_parse::<u8>().map_err(|_| {
                        syn::Error::new(
                            lit.span(),
                            "`#[discrim = N]` value must fit in a u8 (0..=255)",
                        )
                    })?;
                    return Ok(Some(DiscrimAttr {
                        bytes: vec![byte],
                        span,
                    }));
                }
                if let syn::Expr::Array(array) = &nv.value {
                    if array.elems.is_empty() {
                        return Err(syn::Error::new(
                            array.span(),
                            "`#[discrim = [...]]` must contain at least one byte",
                        ));
                    }

                    let mut bytes = Vec::with_capacity(array.elems.len());
                    for elem in &array.elems {
                        let syn::Expr::Lit(syn::ExprLit {
                            lit: syn::Lit::Int(lit),
                            ..
                        }) = elem
                        else {
                            return Err(syn::Error::new(
                                elem.span(),
                                "`#[discrim = [...]]` values must be integer literals",
                            ));
                        };
                        bytes.push(lit.base10_parse::<u8>().map_err(|_| {
                            syn::Error::new(
                                lit.span(),
                                "`#[discrim = [...]]` values must fit in a u8 (0..=255)",
                            )
                        })?);
                    }

                    return Ok(Some(DiscrimAttr { bytes, span }));
                }
                return Err(syn::Error::new(
                    span,
                    "`#[discrim = ...]` value must be an integer literal or byte array literal",
                ));
            }
        }
    }
    Ok(None)
}

fn extract_context_accounts_ident(arg: &FnArg) -> syn::Result<Ident> {
    let ty = match arg {
        FnArg::Typed(pt) => &*pt.ty,
        _ => {
            return Err(syn::Error::new(
                arg.span(),
                "first parameter must be `ctx: &mut Context<T>`",
            ))
        }
    };

    let Type::Reference(reference) = ty else {
        if let Type::Path(context_path) = ty {
            if let Some(context_segment) = context_path.path.segments.last() {
                if context_segment.ident == "Context" {
                    if let syn::PathArguments::AngleBracketed(args) = &context_segment.arguments {
                        if args.args.len() != 1 {
                            return Err(syn::Error::new(
                                args.span(),
                                "Anchor v2 handlers take `ctx: &mut Context<Accounts>`. The v1 multi-lifetime form `Context<'_, '_, 'info, 'info, Accounts<'info>>` is not supported; use `ctx: &mut Context<Buy>`.",
                            ));
                        }
                    }
                    return Err(syn::Error::new(
                        ty.span(),
                        "handler context must be passed by mutable reference: use `ctx: &mut Context<T>`",
                    ));
                }
            }
        }
        return Err(syn::Error::new(
            ty.span(),
            "first parameter must be `ctx: &mut Context<T>`",
        ));
    };
    if reference.mutability.is_none() {
        return Err(syn::Error::new(
            reference.span(),
            "handler context must be mutable: use `ctx: &mut Context<T>`",
        ));
    }

    let Type::Path(context_path) = reference.elem.as_ref() else {
        return Err(syn::Error::new(
            reference.elem.span(),
            "could not parse handler context - expected `Context<YourAccountsStruct>`",
        ));
    };
    let Some(context_segment) = context_path.path.segments.last() else {
        return Err(syn::Error::new(
            context_path.span(),
            "could not parse handler context - expected `Context<YourAccountsStruct>`",
        ));
    };
    if context_segment.ident != "Context" {
        return Err(syn::Error::new(
            context_segment.ident.span(),
            "first parameter must be `ctx: &mut Context<T>`",
        ));
    }

    let syn::PathArguments::AngleBracketed(args) = &context_segment.arguments else {
        return Err(syn::Error::new(
            context_segment.span(),
            "missing accounts type: expected `Context<YourAccountsStruct>`",
        ));
    };
    if args.args.len() != 1 {
        return Err(syn::Error::new(
            args.span(),
            "Anchor v2 `Context` takes exactly one accounts type. Use `ctx: &mut Context<Buy>` instead of the v1 form `Context<'_, '_, 'info, 'info, Buy<'info>>`.",
        ));
    }

    let Some(syn::GenericArgument::Type(Type::Path(accounts_path))) = args.args.first() else {
        return Err(syn::Error::new(
            args.args
                .first()
                .map(Spanned::span)
                .unwrap_or_else(|| args.span()),
            "Context generic must be an accounts struct type, for example `Context<Buy>`",
        ));
    };
    let Some(accounts_segment) = accounts_path.path.segments.last() else {
        return Err(syn::Error::new(
            accounts_path.span(),
            "Context generic must be an accounts struct type, for example `Context<Buy>`",
        ));
    };
    if !matches!(accounts_segment.arguments, syn::PathArguments::None) {
        return Err(syn::Error::new(
            accounts_segment.arguments.span(),
            "Anchor v2 account structs in handler contexts do not take `'info`; use `Context<Buy>` instead of `Context<Buy<'info>>`.",
        ));
    }

    Ok(accounts_segment.ident.clone())
}

/// Converts `snake_case` to `CamelCase` (e.g. `execute_transfer` → `ExecuteTransfer`).
fn to_camel_case(s: &str) -> String {
    s.split('_')
        .map(|part| {
            let mut chars = part.chars();
            match chars.next() {
                Some(c) => c.to_uppercase().to_string() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn instruction_attrs_parse() {
        let attrs: Vec<syn::Attribute> = vec![syn::parse_quote!(
            #[instruction(first: u8, second: u16)]
        )];

        let parsed = parse_instruction_attrs(&attrs).unwrap();

        assert_eq!(
            parsed,
            vec![
                (syn::parse_quote!(first), syn::parse_quote!(u8)),
                (syn::parse_quote!(second), syn::parse_quote!(u16)),
            ]
        );
    }

    #[test]
    fn instruction_attrs_rejects_malformed_entries() {
        let attrs: Vec<syn::Attribute> = vec![syn::parse_quote!(
            #[instruction(first: u8, malformed, second: u16)]
        )];

        let err = parse_instruction_attrs(&attrs).unwrap_err();

        assert!(
            err.to_string().contains("expected `:`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn process_handler_emits_ixarg_fallback_for_missing_instruction_attr() {
        let handler: syn::ItemFn = syn::parse_quote! {
            pub fn do_it(ctx: &mut Context<MyAccounts>, amount: u64, step: u8) -> Result<()> {
                let _ = ctx;
                let _ = amount;
                let _ = step;
                Ok(())
            }
        };
        let mod_name: syn::Ident = syn::parse_quote!(my_program);
        let program_id: syn::Expr = syn::parse_quote!(crate::ID);

        let generated = process_handler(&handler, &mod_name, false, None, &program_id);
        let wrapper = generated.wrapper.to_string();

        assert!(
            wrapper.contains("__AnchorIxArgCoerce"),
            "expected fallback coercion trait in wrapper: {wrapper}"
        );
        assert!(
            wrapper.contains("impl < 'ix > __AnchorIxArgCoerce < 'ix > for ()"),
            "expected missing-#[instruction] fallback impl in wrapper: {wrapper}"
        );
    }

    #[test]
    fn program_attrs_parse_interface_program_id() {
        let config = parse_program_config_tokens(quote!(interface, program_id = super::ID))
            .expect("program attrs should parse");
        let program_id = &config.program_id;

        assert!(config.mode == ProgramMode::Interface);
        assert_eq!(quote!(#program_id).to_string(), "super :: ID");
    }

    #[test]
    fn program_attrs_reject_program_id_without_interface() {
        let err = match parse_program_config_tokens(quote!(program_id = super::ID)) {
            Ok(_) => panic!("program_id without interface unexpectedly parsed"),
            Err(err) => err,
        };

        assert!(
            err.to_string().contains("only supported with"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn program_interface_mode_emits_client_surface_only() {
        let module: syn::ItemMod = syn::parse_quote! {
            pub mod external_program {
                use super::*;

                #[discrim = [1, 2, 3, 4]]
                pub fn do_it(ctx: &mut Context<MyAccounts>, amount: u64) -> Result<()> {
                    let _ = (ctx, amount);
                    unreachable!()
                }
            }
        };
        let config = ProgramConfig {
            mode: ProgramMode::Interface,
            program_id: syn::parse_quote!(super::ID),
        };

        let generated = impl_program(&module, &config).to_string();

        assert!(
            generated.contains("pub mod instruction"),
            "expected instruction module: {generated}"
        );
        assert!(
            generated.contains("pub mod accounts"),
            "expected accounts module: {generated}"
        );
        assert!(
            generated.contains("pub mod cpi"),
            "expected cpi module: {generated}"
        );
        assert!(
            generated.contains("super :: ID"),
            "expected custom program id in instruction builder: {generated}"
        );
        assert!(
            generated.contains("1u8") && generated.contains("2u8"),
            "expected explicit discriminator bytes in instruction builder: {generated}"
        );
        assert!(
            !generated.contains("__anchor_dispatch"),
            "interface mode must not emit executable dispatch: {generated}"
        );
        assert!(
            !generated.contains("default_allocator"),
            "interface mode must not emit entrypoint runtime: {generated}"
        );
    }
}
