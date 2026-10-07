//! IDL generation helpers.
//!
//! All macro-time JSON construction goes through `serde_json::Value` and
//! serializes once at the boundary. Hand-rolled `format!()` string splicing
//! is a footgun — an unescaped quote in a doc comment or a malformed
//! `Custom(String)` shape would silently produce invalid JSON, and the
//! failure surfaces far downstream as "unknown variant" or parser
//! crashes in TS clients. Using `serde_json::json!()` and `Value` gets
//! escaping, composition, and round-trip fidelity for free.
//!
//! The one exception is [`build_accounts_emission`]: it generates a runtime
//! `__idl_accounts()` function that assembles JSON at test time (inside the
//! program crate, not the macro), and pulling `serde_json` into the user's
//! program is not worth it — those format! calls are controlled and tested.

use {
    proc_macro2::TokenStream as TokenStream2,
    quote::quote,
    serde_json::{json, Value},
    syn::{
        punctuated::Punctuated, spanned::Spanned, visit::Visit, Expr, GenericParam, Generics, Lit,
        PathArguments, Token, Type, TypePath,
    },
};

const DYNAMIC_TYPE_KEY: &str = "__anchor_private_type_reference";

#[derive(Default)]
struct TypeLowerer<'a> {
    generics: Option<&'a Generics>,
    references: Vec<TokenStream2>,
}

impl<'a> TypeLowerer<'a> {
    fn with_generics(generics: &'a Generics) -> Self {
        Self {
            generics: Some(generics),
            references: Vec::new(),
        }
    }
    fn lower(&mut self, ty: &Type) -> Value {
        let marker = json!({ DYNAMIC_TYPE_KEY: self.references.len() });
        self.references.push(type_reference_expr(ty, self.generics));
        marker
    }
    fn finish(self, value: Value) -> TokenStream2 {
        let mut remaining = value.to_string();
        if self.references.is_empty() {
            return quote! { #remaining };
        }
        let mut steps = Vec::new();
        for (index, reference) in self.references.iter().enumerate() {
            let marker = json!({ DYNAMIC_TYPE_KEY: index }).to_string();
            let (before, after) = remaining
                .split_once(&marker)
                .expect("IDL reference marker exists");
            steps.push(quote! { __s.push_str(#before); __s.push_str(&(#reference)); });
            remaining = after.to_owned();
        }
        quote! {{
            let mut __s = anchor_lang::__alloc::string::String::new();
            #(#steps)*
            __s.push_str(#remaining);
            anchor_lang::__alloc::boxed::Box::leak(__s.into_boxed_str()) as &'static str
        }}
    }
}

fn generic_ident<'a>(path: &syn::Path, generics: Option<&'a Generics>) -> Option<&'a GenericParam> {
    let ident = path.get_ident()?;
    generics?.params.iter().find(|param| match param {
        GenericParam::Type(ty) => ty.ident == *ident,
        GenericParam::Const(value) => value.ident == *ident,
        _ => false,
    })
}

fn contains_generic(ty: &Type, generics: Option<&Generics>) -> bool {
    struct Finder<'a> {
        generics: Option<&'a Generics>,
        found: bool,
    }
    impl<'ast> Visit<'ast> for Finder<'_> {
        fn visit_type_path(&mut self, path: &'ast TypePath) {
            self.found |=
                path.qself.is_none() && generic_ident(&path.path, self.generics).is_some();
            syn::visit::visit_type_path(self, path);
        }
        fn visit_expr_path(&mut self, path: &'ast syn::ExprPath) {
            self.found |=
                path.qself.is_none() && generic_ident(&path.path, self.generics).is_some();
            syn::visit::visit_expr_path(self, path);
        }
    }
    let mut finder = Finder {
        generics,
        found: false,
    };
    finder.visit_type(ty);
    finder.found
}

fn type_reference_expr(ty: &Type, generics: Option<&Generics>) -> TokenStream2 {
    match ty {
        Type::Reference(reference) => return type_reference_expr(&reference.elem, generics),
        Type::Group(group) => return type_reference_expr(&group.elem, generics),
        Type::Paren(paren) => return type_reference_expr(&paren.elem, generics),
        Type::Array(array) => {
            let element = type_reference_expr(&array.elem, generics);
            let length = peel_expr(&array.len);
            let generic_len = match length {
                Expr::Path(path) if path.qself.is_none() => generic_ident(&path.path, generics)
                    .and_then(|param| match param {
                        GenericParam::Const(value) => Some(value.ident.to_string()),
                        _ => None,
                    }),
                _ => None,
            };
            let count = if let Some(name) = generic_len {
                let json = json!({ "generic": name }).to_string();
                quote! { anchor_lang::__alloc::string::String::from(#json) }
            } else {
                quote! { anchor_lang::__alloc::format!("{}", (#length) as usize) }
            };
            return quote! { anchor_lang::__alloc::format!("{{\"array\":[{},{}]}}", #element, #count) };
        }
        Type::Path(path) if path.qself.is_none() => {
            if let Some(GenericParam::Type(param)) = generic_ident(&path.path, generics) {
                let json = json!({ "generic": param.ident.to_string() }).to_string();
                return quote! { anchor_lang::__alloc::string::String::from(#json) };
            }
        }
        _ => {}
    }
    let type_override = |ty: &Type| {
        if contains_generic(ty, generics) {
            let reference = type_reference_expr(ty, generics);
            quote! { Some(anchor_lang::idl_build::IdlGenericArg::Type(#reference)) }
        } else {
            quote! { None }
        }
    };
    let overrides: Vec<_> = match ty {
        Type::Path(path) => path.path.segments.last().map(|segment| match &segment.arguments {
            PathArguments::AngleBracketed(args) => args.args.iter().filter_map(|arg| match arg {
                syn::GenericArgument::Type(Type::Path(path)) if path.qself.is_none() && matches!(generic_ident(&path.path, generics), Some(GenericParam::Const(_))) => {
                    let value = quote!(#path).to_string();
                    Some(quote! { Some(anchor_lang::idl_build::IdlGenericArg::Const(anchor_lang::__alloc::string::String::from(#value))) })
                }
                syn::GenericArgument::Type(ty) => Some(type_override(ty)),
                syn::GenericArgument::Const(expr) => {
                    let contains = matches!(expr, Expr::Path(path) if generic_ident(&path.path, generics).is_some());
                    Some(if contains {
                        let value = quote!(#expr).to_string();
                        quote! { Some(anchor_lang::idl_build::IdlGenericArg::Const(anchor_lang::__alloc::string::String::from(#value))) }
                    } else { quote! { None } })
                }
                _ => None,
            }).collect(),
            _ => Vec::new(),
        }).unwrap_or_default(),
        Type::Tuple(tuple) => tuple.elems.iter().map(type_override).collect(),
        Type::Slice(slice) => vec![type_override(&slice.elem)],
        _ => Vec::new(),
    };
    quote! { <#ty as anchor_lang::IdlAccountType>::__idl_type_reference(&[#(#overrides),*]) }
}

pub fn rust_type_to_idl(ty: &Type) -> TokenStream2 {
    let mut lowerer = TypeLowerer::default();
    let value = lowerer.lower(ty);
    lowerer.finish(value)
}

fn peel_expr(expr: &Expr) -> &Expr {
    match expr {
        Expr::Group(group) => peel_expr(&group.expr),
        Expr::Paren(paren) => peel_expr(&paren.expr),
        _ => expr,
    }
}

/// Metadata for a defined type, using its definition name and resolved generic
/// arguments. No concrete field-type names are interpreted by the macro.
pub fn type_reference_impl(name: &str, generics: &Generics) -> TokenStream2 {
    let args: Vec<_> = generics.params.iter().filter_map(|param| match param {
        GenericParam::Type(ty) => {
            let ident = &ty.ident;
            Some(quote! { anchor_lang::idl_build::IdlGenericArg::Type(<#ident as anchor_lang::IdlAccountType>::__idl_type_reference(&[])) })
        }
        GenericParam::Const(value) => {
            let ident = &value.ident;
            Some(quote! { anchor_lang::idl_build::IdlGenericArg::Const(anchor_lang::__alloc::format!("{}", #ident)) })
        }
        _ => None,
    }).collect();
    quote! {
        const __IDL_TYPE_NAME: Option<&'static str> = Some(#name);
        fn __idl_type_reference(overrides: &[Option<anchor_lang::idl_build::IdlGenericArg>]) -> anchor_lang::__alloc::string::String {
            let mut args = [#(#args),*];
            anchor_lang::idl_build::__idl_override_args(&mut args, overrides);
            anchor_lang::idl_build::__idl_defined_reference(#name, &args)
        }
    }
}

/// Per-field input to the runtime `__idl_accounts()` emission. See
/// [`build_accounts_emission`].
pub struct AccountsJsonField<'a> {
    pub name: &'a str,
    pub writable: bool,
    pub init_signer: bool,
    /// True when the field type is `Option<T>`. Surfaces as
    /// `"optional":true` in the emitted JSON (matches
    /// `IdlInstructionAccount.optional` in `idl/spec/src/lib.rs:89`).
    /// Names of sibling fields whose `has_one` chain targets this field.
    /// Emitted as `"relations":[...]`. Matches v1's semantics: relations
    /// live on the *target* account (the account being referenced), not
    /// the source — see `lang/syn/src/idl/accounts.rs::get_relations`.
    pub relations: Vec<&'a str>,
    /// `#[doc = "..."]` lines on the field, in source order. Emitted as
    /// `"docs":[...]`.
    pub docs: &'a [String],
    /// Token expression evaluating at IDL-build time to the `pda: {...}`
    /// object JSON body (no leading comma). `None` when the field has no
    /// `seeds = [...]` attr. Built by [`pda_object_emission`].
    pub pda_json: Option<TokenStream2>,
    /// The wrapper `Type` (post-`Option` unwrap) whose trait consts we
    /// dispatch on at runtime. Should match `AccountField::idl_field_ty`.
    pub field_ty: &'a Option<Type>,
    /// Stringified RHS of `#[account(address = <expr>)]`. When `Some`,
    /// takes precedence over `IdlAccountType::__IDL_ADDRESS` at emission.
    /// Holds only dotted field paths like `data.authority` that clients walk
    /// at resolution time.
    pub address_override: Option<&'a str>,
    /// Runtime-resolved static address expression. Evaluated inside
    /// `__idl_accounts()` and rendered as base58.
    pub address_override_expr: Option<&'a TokenStream2>,
    /// Resolved field type used to compose nested account groups.
    pub composition_ty: &'a Type,
}

/// Build a `fn __idl_accounts() -> alloc::string::String` body that assembles
/// the accounts JSON at runtime by reading `<Ty as IdlAccountType>::
/// __IDL_IS_SIGNER / __IDL_ADDRESS`. Compile-time-known flags (writable,
/// init-signer) are baked into the format literals so no runtime work is
/// done for them.
///
/// Runtime assembly (rather than a `&'static str` baked at macro time) is
/// the one concession needed to let the wrapper type's trait const drive
/// per-field signer / address — the const values aren't visible to the
/// macro. Cost is paid once when `anchor idl build` invokes the test.
pub fn build_accounts_emission(fields: &[AccountsJsonField<'_>]) -> TokenStream2 {
    let parts: Vec<TokenStream2> = fields
        .iter()
        .map(|f| {
            let name = f.name;
            let writable_json = if f.writable { ",\"writable\":true" } else { "" };
            let relations_json = if f.relations.is_empty() {
                String::new()
            } else {
                let list: Vec<String> = f.relations.iter().map(|r| format!("\"{r}\"")).collect();
                format!(",\"relations\":[{}]", list.join(","))
            };
            let docs_json = if f.docs.is_empty() {
                String::new()
            } else {
                format!(",\"docs\":{}", docs_to_json_array(f.docs))
            };
            // Static-only fields still pay one `String::from` allocation —
            // immaterial in the test-only IDL build path.
            let pda_json_expr = match &f.pda_json {
                Some(ts) => quote! {
                    let __pda_json: anchor_lang::__alloc::string::String = {
                        let __body: anchor_lang::__alloc::string::String = #ts;
                        anchor_lang::__alloc::format!(",\"pda\":{}", __body)
                    };
                },
                None => quote! {
                    let __pda_json: anchor_lang::__alloc::string::String =
                        anchor_lang::__alloc::string::String::new();
                },
            };
            let init_signer = f.init_signer;
            let leaf = if let Some(ty) = f.field_ty {
                let addr_json_expr = if let Some(address_expr) = f.address_override_expr {
                    quote! {
                        let __addr: anchor_lang::Address =
                            ::core::convert::Into::into(#address_expr);
                        let __addr_string = anchor_lang::__alloc::format!("{}", __addr);
                        let __addr_json: anchor_lang::__alloc::string::String =
                            anchor_lang::__alloc::format!(
                                ",\"address\":{}",
                                anchor_lang::idl_build::__idl_json_string(&__addr_string),
                            );
                    }
                } else if let Some(address_override) = f.address_override {
                    quote! {
                        let __addr_json: anchor_lang::__alloc::string::String =
                            anchor_lang::__alloc::format!(
                                ",\"address\":{}",
                                anchor_lang::idl_build::__idl_json_string(#address_override),
                            );
                    }
                } else {
                    quote! {
                        let __addr = <#ty as anchor_lang::IdlAccountType>::__IDL_ADDRESS;
                        let __addr_json: anchor_lang::__alloc::string::String = match __addr {
                            Some(a) => anchor_lang::__alloc::format!(
                                ",\"address\":{}",
                                anchor_lang::idl_build::__idl_json_string(a),
                            ),
                            None => anchor_lang::__alloc::string::String::new(),
                        };
                    }
                };
                quote! {
                    {
                        // Trait-const OR compile-time init_signer flag.
                        // Kept separate so a Signer + init-without-seeds
                        // combo still renders exactly one `"signer":true`.
                        let __signer = <#ty as anchor_lang::IdlAccountType>::__IDL_IS_SIGNER
                            || #init_signer;
                        let __signer_json: &str = if __signer { ",\"signer\":true" } else { "" };
                        let __optional_json: &str = if <#ty as anchor_lang::IdlAccountType>::__IDL_IS_OPTIONAL { ",\"optional\":true" } else { "" };
                        #addr_json_expr
                        #pda_json_expr
                        anchor_lang::__alloc::format!(
                            "{{\"name\":\"{}\"{}{}{}{}{}{}{}}}",
                            #name,
                            #writable_json,
                            __signer_json,
                            __addr_json,
                            __optional_json,
                            #relations_json,
                            #docs_json,
                            __pda_json,
                        )
                    }
                }
            } else {
                // Defensive fallback for non-`Type::Path` field types —
                // can't resolve the trait, so we emit only compile-time
                // flags. Never triggers for valid Accounts structs.
                let signer_json = if init_signer { ",\"signer\":true" } else { "" };
                let addr_json_expr = if let Some(address_override) = f.address_override {
                    quote! {
                        let __addr_json: anchor_lang::__alloc::string::String =
                            anchor_lang::__alloc::format!(
                                ",\"address\":{}",
                                anchor_lang::idl_build::__idl_json_string(#address_override),
                            );
                    }
                } else {
                    quote! {
                        let __addr_json: anchor_lang::__alloc::string::String =
                            anchor_lang::__alloc::string::String::new();
                    }
                };
                quote! {
                    {
                        #addr_json_expr
                        #pda_json_expr
                        anchor_lang::__alloc::format!(
                            "{{\"name\":\"{}\"{}{}{}{}{}{}{}}}",
                            #name,
                            #writable_json,
                            #signer_json,
                            __addr_json,
                            "",
                            #relations_json,
                            #docs_json,
                            __pda_json,
                        )
                    }
                }
            };
            let ty = f.composition_ty;
            quote! {
                if let Some(__inner) = <#ty as anchor_lang::IdlAccountType>::__idl_nested_accounts() {
                    __inner.strip_prefix('[').and_then(|s| s.strip_suffix(']')).unwrap_or(&__inner).to_string()
                } else { #leaf }
            }
        })
        .collect();

    quote! {
        /// **Opaque / unstable.** Returns the IDL JSON for this Accounts
        /// struct's account list. Implementation detail of the IDL build
        /// pipeline; do not rely on the shape or call this directly.
        #[doc(hidden)]
        pub fn __idl_accounts() -> anchor_lang::__alloc::string::String {
            let __parts: anchor_lang::__alloc::vec::Vec<
                anchor_lang::__alloc::string::String
            > = anchor_lang::__alloc::vec![#(#parts),*];
            let mut __s = anchor_lang::__alloc::string::String::from("[");
            let mut __first = true;
            for __p in &__parts {
                // A cfg-disabled field contributes an empty part. Skip it
                // so we don't emit `,,` or a leading comma.
                if __p.is_empty() { continue; }
                if !__first { __s.push(','); }
                __first = false;
                __s.push_str(__p);
            }
            __s.push(']');
            __s
        }
    }
}

/// Build IDL instruction args JSON from handler parameters.
pub fn build_args_json(args: &[(&syn::Ident, &Type)]) -> TokenStream2 {
    let mut lowerer = TypeLowerer::default();
    let arr: Vec<Value> = args
        .iter()
        .map(|(name, ty)| {
            json!({
                "name": name.to_string(),
                "type": lowerer.lower(ty),
            })
        })
        .collect();
    lowerer.finish(Value::Array(arr))
}

/// Build discriminator JSON array from hash bytes.
pub fn disc_json(disc_bytes: &[u8]) -> String {
    disc_json_value(disc_bytes).to_string()
}

fn disc_json_value(disc_bytes: &[u8]) -> Value {
    Value::Array(disc_bytes.iter().map(|b| json!(*b)).collect())
}

/// Borsh / bytemuck mode tag passed down from the `#[account]` / `#[event]`
/// call sites. The spec (`idl/spec/src/lib.rs:180-216`) models this as the
/// pair `(IdlSerialization, Option<IdlRepr>)`, but the two fields are tightly
/// coupled — bytemuck always pairs with `repr(C)` in our codegen, borsh
/// carries no repr — so we collapse them into a single enum and expand both
/// fields at emission time.
///
/// Default `#[event]` (wincode under the hood) is also tagged `Borsh` here:
/// the wire format is borsh-compatible via `BORSH_CONFIG`, so off-chain
/// consumers decode it as borsh.
#[derive(Clone, Copy)]
pub enum TypeKind {
    /// Default borsh layout. Spec `skip_serializing_if`s both fields at the
    /// default value, so nothing extra gets emitted.
    Borsh,
    /// `bytemuck` Pod + `repr(C)` plus any layout modifiers preserved from
    /// the source item's `#[repr(...)]` attributes.
    BytemuckRepr(BytemuckRepr),
}

#[derive(Clone, Copy, Default)]
pub struct BytemuckRepr {
    pub packed: bool,
    pub align: Option<usize>,
}

pub fn bytemuck_repr_from_attrs(attrs: &[syn::Attribute]) -> syn::Result<BytemuckRepr> {
    let mut repr = BytemuckRepr::default();

    for attr in attrs.iter().filter(|attr| attr.path().is_ident("repr")) {
        let metas = attr.parse_args_with(Punctuated::<syn::Meta, Token![,]>::parse_terminated)?;
        for meta in metas {
            match meta {
                syn::Meta::Path(path) if path.is_ident("packed") => {
                    repr.packed = true;
                }
                syn::Meta::List(list) if list.path.is_ident("packed") => {
                    let packed = list.parse_args::<syn::LitInt>()?;
                    let packed = packed.base10_parse::<usize>()?;
                    if packed != 1 {
                        return Err(syn::Error::new(
                            list.span(),
                            "Anchor IDL only supports `#[repr(..., packed)]` or `#[repr(..., \
                             packed(1))]`; other packed widths would produce a lossy IDL layout",
                        ));
                    }
                    repr.packed = true;
                }
                syn::Meta::List(list) if list.path.is_ident("align") => {
                    let align = list.parse_args::<syn::LitInt>()?;
                    repr.align = Some(align.base10_parse()?);
                }
                _ => {}
            }
        }
    }

    Ok(repr)
}

pub fn build_struct_type_def_emission(
    name: &str,
    docs: &[String],
    fields: &syn::Fields,
    kind: TypeKind,
    generics: &Generics,
) -> TokenStream2 {
    let (header, suffix) = type_def_header_parts(name, docs, kind, generics, "struct");
    let field_pushes: Vec<_> = match fields {
        syn::Fields::Named(named) => named
            .named
            .iter()
            .map(|field| field_push_stmt(field, generics))
            .collect(),
        syn::Fields::Unnamed(unnamed) => unnamed
            .unnamed
            .iter()
            .map(|field| {
                let field_json = unnamed_field_emission(field, generics);
                let cfg_attrs = crate::cfg_attrs(&field.attrs);
                quote! {
                    #(#cfg_attrs)*
                    __entries.push(#field_json);
                }
            })
            .collect(),
        syn::Fields::Unit => Vec::new(),
    };
    build_joined_type_def_emission(header, suffix, &field_pushes)
}

pub fn build_enum_type_def_emission(
    name: &str,
    docs: &[String],
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::token::Comma>,
    kind: TypeKind,
    generics: &Generics,
) -> TokenStream2 {
    let (header, suffix) = type_def_header_parts(name, docs, kind, generics, "enum");
    let variant_pushes: Vec<_> = variants
        .iter()
        .map(|variant| variant_push_stmt(variant, generics))
        .collect();
    build_joined_type_def_emission(header, suffix, &variant_pushes)
}

fn build_joined_type_def_emission(
    header: String,
    suffix: String,
    entries: &[TokenStream2],
) -> TokenStream2 {
    quote! {
        {
            let mut __entries: anchor_lang::__alloc::vec::Vec<&'static str> =
                anchor_lang::__alloc::vec::Vec::new();
            #(#entries)*
            let mut __s = anchor_lang::__alloc::string::String::from(#header);
            let mut __first = true;
            for __entry in &__entries {
                if !__first {
                    __s.push(',');
                }
                __first = false;
                __s.push_str(__entry);
            }
            __s.push_str(#suffix);
            ::core::option::Option::Some(
                anchor_lang::__alloc::boxed::Box::leak(__s.into_boxed_str()) as &'static str
            )
        }
    }
}

fn build_joined_entry_emission(
    header: String,
    entries: &[TokenStream2],
    suffix: String,
) -> TokenStream2 {
    quote! {
        {
            let mut __entries: anchor_lang::__alloc::vec::Vec<&'static str> =
                anchor_lang::__alloc::vec::Vec::new();
            #(#entries)*
            let mut __s = anchor_lang::__alloc::string::String::from(#header);
            let mut __first = true;
            for __entry in &__entries {
                if !__first {
                    __s.push(',');
                }
                __first = false;
                __s.push_str(__entry);
            }
            __s.push_str(#suffix);
            anchor_lang::__alloc::boxed::Box::leak(__s.into_boxed_str()) as &'static str
        }
    }
}

fn type_def_header_parts(
    name: &str,
    docs: &[String],
    kind: TypeKind,
    generics: &Generics,
    kind_name: &str,
) -> (String, String) {
    const FIELD_MARKER: &str = "__anchor_private_fields__";
    let entries_key = if kind_name == "enum" {
        "variants"
    } else {
        "fields"
    };
    let mut type_def_obj = build_type_def_header(name, docs, kind, generics);
    type_def_obj.insert(
        "type".into(),
        json!({ "kind": kind_name, entries_key: [FIELD_MARKER] }),
    );
    let header = Value::Object(type_def_obj).to_string();
    let marker = Value::String(FIELD_MARKER.to_owned()).to_string();
    let (prefix, suffix) = header
        .split_once(&marker)
        .expect("type definition header should contain the field marker");
    (prefix.to_owned(), suffix.to_owned())
}

fn field_push_stmt(field: &syn::Field, generics: &Generics) -> TokenStream2 {
    let field_json = named_field_emission(field, generics);
    let cfg_attrs = crate::cfg_attrs(&field.attrs);
    quote! {
        #(#cfg_attrs)*
        __entries.push(#field_json);
    }
}

fn variant_push_stmt(variant: &syn::Variant, generics: &Generics) -> TokenStream2 {
    let variant_json = variant_emission(variant, generics);
    let cfg_attrs = crate::cfg_attrs(&variant.attrs);
    quote! {
        #(#cfg_attrs)*
        __entries.push(#variant_json);
    }
}

fn named_field_emission(field: &syn::Field, generics: &Generics) -> TokenStream2 {
    let mut lowerer = TypeLowerer::with_generics(generics);
    let value = named_field_value(field, &mut lowerer);
    lowerer.finish(value)
}

fn unnamed_field_emission(field: &syn::Field, generics: &Generics) -> TokenStream2 {
    let mut lowerer = TypeLowerer::with_generics(generics);
    let value = lowerer.lower(&field.ty);
    lowerer.finish(value)
}

fn variant_emission(variant: &syn::Variant, generics: &Generics) -> TokenStream2 {
    let name = variant.ident.to_string();
    match &variant.fields {
        syn::Fields::Unit => {
            let variant_json = json!({ "name": name }).to_string();
            quote! { #variant_json }
        }
        syn::Fields::Named(named) => {
            const FIELD_MARKER: &str = "__anchor_private_variant_fields__";
            let header = json!({ "name": name, "fields": [FIELD_MARKER] }).to_string();
            let marker = Value::String(FIELD_MARKER.to_owned()).to_string();
            let (header, suffix) = header
                .split_once(&marker)
                .expect("variant header should contain the field marker");
            let field_pushes: Vec<_> = named
                .named
                .iter()
                .map(|field| field_push_stmt(field, generics))
                .collect();
            build_joined_entry_emission(header.to_owned(), &field_pushes, suffix.to_owned())
        }
        syn::Fields::Unnamed(unnamed) => {
            const FIELD_MARKER: &str = "__anchor_private_variant_fields__";
            let header = json!({ "name": name, "fields": [FIELD_MARKER] }).to_string();
            let marker = Value::String(FIELD_MARKER.to_owned()).to_string();
            let (header, suffix) = header
                .split_once(&marker)
                .expect("variant header should contain the field marker");
            let field_pushes: Vec<_> = unnamed
                .unnamed
                .iter()
                .map(|field| {
                    let field_json = unnamed_field_emission(field, generics);
                    let cfg_attrs = crate::cfg_attrs(&field.attrs);
                    quote! {
                        #(#cfg_attrs)*
                        __entries.push(#field_json);
                    }
                })
                .collect();
            build_joined_entry_emission(header.to_owned(), &field_pushes, suffix.to_owned())
        }
    }
}

/// Shared header construction for the `IdlTypeDef` payload. Emits
/// `name`, optional `docs`, and the `serialization` / `repr` pair derived
/// from `kind`. The caller appends the `type` object matching
/// `IdlTypeDefTy::{Struct, Enum, Type}`. Notably *no* `discriminator` —
/// that field only belongs on the accounts entry.
fn build_type_def_header(
    name: &str,
    docs: &[String],
    kind: TypeKind,
    generics: &Generics,
) -> serde_json::Map<String, Value> {
    let mut out = serde_json::Map::new();
    out.insert("name".into(), Value::String(name.to_owned()));
    if !docs.is_empty() {
        out.insert("docs".into(), docs_value(docs));
    }
    let generics = generic_definitions(generics);
    if !generics.is_empty() {
        out.insert("generics".into(), Value::Array(generics));
    }
    match kind {
        TypeKind::Borsh => {}
        TypeKind::BytemuckRepr(repr) => {
            out.insert("serialization".into(), Value::String("bytemuck".into()));
            let mut repr_json = serde_json::Map::new();
            repr_json.insert("kind".into(), Value::String("c".into()));
            if repr.packed {
                repr_json.insert("packed".into(), Value::Bool(true));
            }
            if let Some(align) = repr.align {
                repr_json.insert("align".into(), json!(align));
            }
            out.insert("repr".into(), Value::Object(repr_json));
        }
    }
    out
}

fn generic_definitions(generics: &Generics) -> Vec<Value> {
    generics
        .params
        .iter()
        .filter_map(|param| match param {
            GenericParam::Type(param) => {
                Some(json!({ "kind": "type", "name": param.ident.to_string() }))
            }
            GenericParam::Const(param) => {
                let ty = &param.ty;
                Some(json!({
                    "kind": "const",
                    "name": param.ident.to_string(),
                    "type": quote!(#ty).to_string().replace(' ', ""),
                }))
            }
            GenericParam::Lifetime(_) => None,
        })
        .collect()
}

/// Build a named `IdlField` value — `{name, type, docs?}` — for a single
/// `syn::Field`. Used by both struct field and enum-variant struct-field
/// emission.
fn named_field_value(f: &syn::Field, lowerer: &mut TypeLowerer<'_>) -> Value {
    let fname = f
        .ident
        .as_ref()
        .expect("named fields always have idents")
        .to_string();
    let mut obj = serde_json::Map::new();
    obj.insert("name".into(), Value::String(fname));
    let field_docs = extract_doc_lines(&f.attrs);
    if !field_docs.is_empty() {
        obj.insert("docs".into(), docs_value(&field_docs));
    }
    obj.insert("type".into(), lowerer.lower(&f.ty));
    Value::Object(obj)
}

// ---------------------------------------------------------------------------
// Docs extraction
// ---------------------------------------------------------------------------

/// Extract `#[doc = "..."]` lines from a list of attributes. `/// foo`
/// desugars to `#[doc = " foo"]` — the compiler inserts a single leading
/// space that we strip so IDL consumers don't see extra indentation.
pub fn extract_doc_lines(attrs: &[syn::Attribute]) -> Vec<String> {
    attrs
        .iter()
        .filter_map(|attr| {
            if !attr.path().is_ident("doc") {
                return None;
            }
            if let syn::Meta::NameValue(nv) = &attr.meta {
                if let Expr::Lit(lit) = &nv.value {
                    if let Lit::Str(s) = &lit.lit {
                        let v = s.value();
                        return Some(v.strip_prefix(' ').map(str::to_owned).unwrap_or(v));
                    }
                }
            }
            None
        })
        .collect()
}

/// Serialize a list of doc lines into a JSON array string.
pub fn docs_to_json_array(docs: &[String]) -> String {
    docs_value(docs).to_string()
}

fn docs_value(docs: &[String]) -> Value {
    Value::Array(docs.iter().map(|d| Value::String(d.clone())).collect())
}

// ---------------------------------------------------------------------------
// Seed classification (Part E — `pda: {...}` emission)
// ---------------------------------------------------------------------------

/// Classified seed expression. Only supported IDL seed shapes survive to the
/// final JSON: statically-known shapes stay `Static`, constant-only runtime
/// expressions become `Runtime`, and runtime-only unsupported expressions are
/// filtered out as `Unsupported`.
#[derive(Clone)]
pub enum SeedJson {
    /// Pre-serialized JSON object — known at macro time.
    Static(String),
    /// Token expression evaluating to `alloc::string::String` at IDL-build
    /// time.
    Runtime(TokenStream2),
    /// Expression depends on runtime-only values and cannot be represented in
    /// the current IDL seed spec.
    Unsupported,
}

#[derive(Clone)]
pub enum SeedListJson {
    /// Per-seed JSON objects already split into the spec's supported variants.
    Listed(Vec<SeedJson>),
    /// Token expression evaluating to a full JSON seed array string (including
    /// the surrounding `[...]`) at IDL-build time.
    Runtime(TokenStream2),
}

impl SeedJson {
    /// Token expression evaluating to `String` at IDL-build time.
    pub fn into_string_expr(self) -> TokenStream2 {
        match self {
            SeedJson::Static(s) => quote! {
                anchor_lang::__alloc::string::String::from(#s)
            },
            SeedJson::Runtime(ts) => ts,
            SeedJson::Unsupported => {
                unreachable!("unsupported seed must be filtered out before IDL emission")
            }
        }
    }
}

/// Classify a single seed expression into one of the `IdlSeed` variants
/// (spec:111-134).
///
/// Statically recognized shapes (returned as `SeedJson::Static`):
/// - byte literal (`b"counter"`)              → `{"kind":"const","value":[...]}`
/// - byte-array literal (`[1, 2, 3]`)         → `{"kind":"const","value":[...]}`
/// - string literal (`"counter"`)             → `{"kind":"const","value":[<bytes>]}`
/// - `"literal".as_bytes()`                   → `{"kind":"const","value":[...]}`
/// - account field ref (`user` bare ident,
///   `user.key().as_ref()`, `user.address().as_ref()`,
///   `user.as_ref()`) with `user` in `field_names`
///   → `{"kind":"account","path":"user"}`
/// - instruction arg ref (`nonce` bare ident,
///   `nonce.to_le_bytes()`, `nonce.as_ref()`)
///   with `nonce` in `ix_arg_names`
///   → `{"kind":"arg","path":"nonce"}`
///
/// Constant-only expressions that aren't structurally recognized are evaluated
/// at IDL-build time into `{"kind":"const","value":[...]}`. Expressions that
/// depend on runtime account/arg values remain `Unsupported` and should cause
/// PDA metadata omission rather than invalid IDL output.
pub fn classify_seed(expr: &Expr, field_names: &[String], ix_arg_names: &[String]) -> SeedJson {
    if let Some(seed) = classify_seed_inner(expr, field_names, ix_arg_names) {
        seed
    } else if expr_contains_macro(expr)
        || expr_references_runtime_seed_inputs(expr, field_names, ix_arg_names)
    {
        SeedJson::Unsupported
    } else {
        runtime_seed(expr)
    }
}

pub fn classify_seed_list(
    expr: &Expr,
    field_names: &[String],
    ix_arg_names: &[String],
) -> Option<SeedListJson> {
    match expr {
        Expr::Array(arr) => {
            let seeds: Vec<_> = arr
                .elems
                .iter()
                .map(|seed| classify_seed(seed, field_names, ix_arg_names))
                .collect();
            seeds
                .iter()
                .all(|seed| !matches!(seed, SeedJson::Unsupported))
                .then_some(SeedListJson::Listed(seeds))
        }
        _ => (!expr_references_runtime_seed_inputs(expr, field_names, ix_arg_names))
            .then_some(SeedListJson::Runtime(runtime_seeds(expr))),
    }
}

/// Classify `seeds::program = <expr>` into an IDL seed. Unlike arbitrary PDA
/// seed expressions, this expression is semantically a program id, so opaque
/// expressions are evaluated during IDL build and emitted as const bytes.
pub fn classify_program_seed(
    expr: &Expr,
    field_names: &[String],
    ix_arg_names: &[String],
) -> Option<SeedJson> {
    let seed = classify_seed(expr, field_names, ix_arg_names);
    match seed {
        SeedJson::Unsupported => None,
        other => Some(other),
    }
}

pub fn expr_contains_macro(expr: &Expr) -> bool {
    struct MacroFinder {
        found: bool,
    }

    impl<'ast> Visit<'ast> for MacroFinder {
        fn visit_expr_macro(&mut self, _expr: &'ast syn::ExprMacro) {
            self.found = true;
        }
    }

    let mut finder = MacroFinder { found: false };
    finder.visit_expr(expr);
    finder.found
}

pub fn expr_references_local_binding(
    expr: &Expr,
    field_names: &[String],
    ix_arg_names: &[String],
) -> bool {
    struct LocalBindingFinder<'a> {
        field_names: &'a [String],
        ix_arg_names: &'a [String],
        found: bool,
    }

    impl LocalBindingFinder<'_> {
        fn is_local_name(&self, ident: &str) -> bool {
            self.field_names.iter().any(|name| name == ident)
                || self.ix_arg_names.iter().any(|name| name == ident)
        }
    }

    impl<'ast> Visit<'ast> for LocalBindingFinder<'_> {
        fn visit_expr_path(&mut self, expr: &'ast syn::ExprPath) {
            if self.found {
                return;
            }
            if expr.qself.is_none()
                && expr.path.leading_colon.is_none()
                && expr.path.segments.len() == 1
                && expr.path.segments[0].arguments.is_empty()
                && self.is_local_name(&expr.path.segments[0].ident.to_string())
            {
                self.found = true;
                return;
            }
            syn::visit::visit_expr_path(self, expr);
        }
    }

    let mut finder = LocalBindingFinder {
        field_names,
        ix_arg_names,
        found: false,
    };
    finder.visit_expr(expr);
    finder.found
}

fn static_seed(value: Value) -> SeedJson {
    SeedJson::Static(value.to_string())
}

fn runtime_seed(expr: &Expr) -> SeedJson {
    SeedJson::Runtime(quote! {
        anchor_lang::idl_build::__idl_const_seed_json(#expr)
    })
}

fn runtime_seeds(expr: &Expr) -> TokenStream2 {
    quote! {
        anchor_lang::idl_build::__idl_const_seeds_json(#expr)
    }
}

fn classify_seed_inner(
    expr: &Expr,
    field_names: &[String],
    ix_arg_names: &[String],
) -> Option<SeedJson> {
    // Peel `&<inner>` wrappers — they're common in seed expressions and
    // always transparent to classification.
    let mut cur = expr;
    while let Expr::Reference(r) = cur {
        cur = &r.expr;
    }

    if let Expr::Lit(lit) = cur {
        match &lit.lit {
            Lit::ByteStr(bs) => return Some(static_seed(const_seed_value(&bs.value()))),
            Lit::Str(s) => return Some(static_seed(const_seed_value(s.value().as_bytes()))),
            Lit::Byte(b) => return Some(static_seed(const_seed_value(&[b.value()]))),
            _ => {}
        }
    }

    // Array literal with fully-u8 elements: [1, 2, 3]
    if let Expr::Array(arr) = cur {
        let mut bytes: Option<Vec<u8>> = Some(Vec::with_capacity(arr.elems.len()));
        for e in &arr.elems {
            if let Expr::Lit(syn::ExprLit {
                lit: Lit::Int(i), ..
            }) = e
            {
                if let Ok(v) = i.base10_parse::<u8>() {
                    bytes.as_mut().unwrap().push(v);
                    continue;
                }
            }
            bytes = None;
            break;
        }
        if let Some(b) = bytes {
            return Some(static_seed(const_seed_value(&b)));
        }
    }

    if let Some(root) = account_seed_root_ident(cur) {
        if field_names.contains(&root) {
            return Some(static_seed(account_seed_value(&root)));
        }
    }

    if let Some(root) = arg_seed_root_ident(cur) {
        if ix_arg_names.contains(&root) {
            return Some(static_seed(arg_seed_value(&root)));
        }
    }

    if let Some(s) = string_as_bytes(cur) {
        return Some(static_seed(const_seed_value(s.value().as_bytes())));
    }

    None
}

/// Return the bare root ident for simple, client-derivable seed expressions.
/// This intentionally does not walk arbitrary field/index chains because
/// `account.data.to_le_bytes()` is account data, not the account pubkey.
pub fn receiver_root_ident_str(expr: &Expr) -> Option<String> {
    account_seed_root_ident(expr).or_else(|| arg_seed_root_ident(expr))
}

fn peel_seed_wrappers(mut expr: &Expr) -> &Expr {
    loop {
        match expr {
            Expr::Paren(p) => expr = &p.expr,
            Expr::Reference(r) => expr = &r.expr,
            _ => return expr,
        }
    }
}

fn bare_ident(expr: &Expr) -> Option<String> {
    let Expr::Path(ep) = peel_seed_wrappers(expr) else {
        return None;
    };
    if ep.qself.is_none()
        && ep.path.segments.len() == 1
        && ep.path.leading_colon.is_none()
        && ep.path.segments[0].arguments.is_empty()
    {
        Some(ep.path.segments[0].ident.to_string())
    } else {
        None
    }
}

fn method_receiver<'a>(expr: &'a Expr, method: &str) -> Option<&'a Expr> {
    let Expr::MethodCall(mc) = peel_seed_wrappers(expr) else {
        return None;
    };
    (mc.method == method && mc.args.is_empty()).then_some(&*mc.receiver)
}

fn expr_references_runtime_seed_inputs(
    expr: &Expr,
    field_names: &[String],
    ix_arg_names: &[String],
) -> bool {
    struct RuntimeSeedInputVisitor<'a> {
        names: Vec<&'a str>,
        found: bool,
    }

    impl<'ast> Visit<'ast> for RuntimeSeedInputVisitor<'_> {
        fn visit_expr_path(&mut self, node: &'ast syn::ExprPath) {
            if self.found {
                return;
            }
            if let Some(ident) = node.path.get_ident() {
                let ident = ident.to_string();
                if self.names.iter().any(|name| *name == ident) {
                    self.found = true;
                    return;
                }
            }
            syn::visit::visit_expr_path(self, node);
        }
    }

    let mut visitor = RuntimeSeedInputVisitor {
        names: field_names
            .iter()
            .chain(ix_arg_names.iter())
            .map(String::as_str)
            .collect(),
        found: false,
    };
    visitor.visit_expr(expr);
    visitor.found
}

fn account_seed_root_ident(expr: &Expr) -> Option<String> {
    let expr = peel_seed_wrappers(expr);
    if let Some(root) = bare_ident(expr) {
        return Some(root);
    }

    let as_ref_receiver = method_receiver(expr, "as_ref").unwrap_or(expr);
    if let Some(root) = bare_ident(as_ref_receiver) {
        return Some(root);
    }

    method_receiver(as_ref_receiver, "address")
        .or_else(|| method_receiver(as_ref_receiver, "key"))
        .and_then(bare_ident)
}

fn arg_seed_root_ident(expr: &Expr) -> Option<String> {
    let expr = peel_seed_wrappers(expr);
    if let Some(root) = bare_ident(expr) {
        return Some(root);
    }

    method_receiver(expr, "as_ref")
        .or_else(|| method_receiver(expr, "to_le_bytes"))
        .and_then(bare_ident)
}

fn string_as_bytes(expr: &Expr) -> Option<&syn::LitStr> {
    let receiver = method_receiver(expr, "as_bytes")?;
    if let Expr::Lit(syn::ExprLit {
        lit: Lit::Str(s), ..
    }) = peel_seed_wrappers(receiver)
    {
        Some(s)
    } else {
        None
    }
}

fn const_seed_value(bytes: &[u8]) -> Value {
    json!({ "kind": "const", "value": bytes })
}

fn account_seed_value(path: &str) -> Value {
    json!({ "kind": "account", "path": path })
}

fn arg_seed_value(path: &str) -> Value {
    json!({ "kind": "arg", "path": path })
}

/// Assemble the `pda: {...}` object body from a field's classified seeds
/// plus optional program override. Returns a token expression that
/// evaluates to the JSON object string at IDL-build time (without the
/// leading `,"pda":` — that's spliced by `build_accounts_emission`).
///
/// Static seeds become string-literal pushes. The whole expression assembles a
/// single `String` via `push_str`, avoiding intermediate `Vec` /
/// `serde_json::Value` round-trips.
pub fn pda_object_emission(seeds: &SeedListJson, program: Option<&SeedJson>) -> TokenStream2 {
    let seeds_expr = match seeds {
        SeedListJson::Listed(seeds) => {
            let seed_pushes: Vec<TokenStream2> = seeds
                .iter()
                .enumerate()
                .map(|(i, s)| {
                    let expr = s.clone().into_string_expr();
                    let comma = if i == 0 { "" } else { "," };
                    quote! {
                        __seeds.push_str(#comma);
                        __seeds.push_str(&{ #expr });
                    }
                })
                .collect();
            quote! {
                {
                    let mut __seeds = anchor_lang::__alloc::string::String::from("[");
                    #(#seed_pushes)*
                    __seeds.push(']');
                    __seeds
                }
            }
        }
        SeedListJson::Runtime(ts) => quote! { { #ts } },
    };
    let program_part = match program {
        None => quote! {},
        Some(p) => {
            let expr = p.clone().into_string_expr();
            quote! {
                __pda.push_str(",\"program\":");
                __pda.push_str(&{ #expr });
            }
        }
    };
    quote! {
        {
            let mut __pda = anchor_lang::__alloc::string::String::from("{\"seeds\":");
            __pda.push_str(&{ #seeds_expr });
            #program_part
            __pda.push('}');
            __pda
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Pull the inner JSON string out of a `Static` seed for assertion.
    /// Panics if the seed was classified as `Runtime`.
    fn expect_static(seed: SeedJson) -> String {
        match seed {
            SeedJson::Static(s) => s,
            SeedJson::Runtime(ts) => {
                panic!("expected Static seed, got Runtime: {}", ts);
            }
            SeedJson::Unsupported => panic!("expected Static seed, got Unsupported"),
        }
    }

    fn expect_runtime(seed: SeedJson) -> String {
        match seed {
            SeedJson::Static(s) => panic!("expected Runtime seed, got Static: {s}"),
            SeedJson::Runtime(ts) => ts.to_string(),
            SeedJson::Unsupported => panic!("expected Runtime seed, got Unsupported"),
        }
    }

    fn expect_unsupported(seed: SeedJson) {
        assert!(
            matches!(seed, SeedJson::Unsupported),
            "expected Unsupported seed"
        );
    }

    fn classify(expr: syn::Expr, fields: &[&str], args: &[&str]) -> SeedJson {
        let fields: Vec<String> = fields.iter().map(|s| (*s).to_string()).collect();
        let args: Vec<String> = args.iter().map(|s| (*s).to_string()).collect();
        classify_seed(&expr, &fields, &args)
    }

    #[test]
    fn byte_string_literal_is_static_const() {
        let s = expect_static(classify(syn::parse_quote!(b"counter"), &[], &[]));
        assert_eq!(
            s,
            r#"{"kind":"const","value":[99,111,117,110,116,101,114]}"#
        );
    }

    #[test]
    fn string_literal_is_static_const_with_utf8_bytes() {
        let s = expect_static(classify(syn::parse_quote!("ab"), &[], &[]));
        assert_eq!(s, r#"{"kind":"const","value":[97,98]}"#);
    }

    #[test]
    fn generated_references_delegate_to_the_resolved_type() {
        for ty in [
            syn::parse_quote!(Alias),
            syn::parse_quote!(custom::Vec<u64>),
            syn::parse_quote!(Buf<MAX>),
        ] {
            let generated = rust_type_to_idl(&ty).to_string();
            assert!(generated.contains("IdlAccountType"));
            assert!(generated.contains("__idl_type_reference"));
        }
    }

    #[test]
    fn packed_one_repr_modifier_sets_packed_flag() {
        let attrs: Vec<syn::Attribute> = vec![syn::parse_quote!(#[repr(C, packed(1))])];
        let repr = bytemuck_repr_from_attrs(&attrs).expect("packed(1) should parse");
        assert!(repr.packed);
        assert_eq!(repr.align, None);
    }

    #[test]
    fn packed_greater_than_one_repr_modifier_is_rejected() {
        let attrs: Vec<syn::Attribute> = vec![syn::parse_quote!(#[repr(C, packed(2))])];
        let err = match bytemuck_repr_from_attrs(&attrs) {
            Ok(_) => panic!("packed(2) should be rejected"),
            Err(err) => err,
        };
        assert!(err.to_string().contains("Anchor IDL only supports"));
        assert!(err.to_string().contains("lossy IDL layout"));
    }

    #[test]
    fn byte_literal_is_static_one_byte_const() {
        let s = expect_static(classify(syn::parse_quote!(b'A'), &[], &[]));
        assert_eq!(s, r#"{"kind":"const","value":[65]}"#);
    }

    #[test]
    fn byte_array_literal_is_static_const() {
        let s = expect_static(classify(syn::parse_quote!([1u8, 2, 3]), &[], &[]));
        assert_eq!(s, r#"{"kind":"const","value":[1,2,3]}"#);
    }

    #[test]
    fn byte_array_with_non_u8_is_evaluated_at_idl_build_time() {
        let ts = expect_runtime(classify(syn::parse_quote!([999, 2]), &[], &[]));
        assert!(ts.contains("__idl_const_seed_json"), "got: {ts}");
    }

    #[test]
    fn ampersand_wrapper_is_peeled() {
        // Common shape — `&b"foo"` — same shape as the bare literal.
        let s = expect_static(classify(syn::parse_quote!(&b"x"), &[], &[]));
        assert_eq!(s, r#"{"kind":"const","value":[120]}"#);
    }

    #[test]
    fn bare_field_ident_classifies_as_account() {
        let s = expect_static(classify(syn::parse_quote!(user), &["user"], &[]));
        assert_eq!(s, r#"{"kind":"account","path":"user"}"#);
    }

    #[test]
    fn bare_arg_ident_classifies_as_arg() {
        let s = expect_static(classify(syn::parse_quote!(nonce), &[], &["nonce"]));
        assert_eq!(s, r#"{"kind":"arg","path":"nonce"}"#);
    }

    #[test]
    fn field_ref_takes_precedence_over_arg_ref() {
        // Same identifier in both lists: documented behavior is field wins.
        let s = expect_static(classify(syn::parse_quote!(name), &["name"], &["name"]));
        assert_eq!(s, r#"{"kind":"account","path":"name"}"#);
    }

    #[test]
    fn method_chain_resolves_account_root() {
        // `user.address().as_ref()` — the canonical Pubkey-seed shape.
        let s = expect_static(classify(
            syn::parse_quote!(user.address().as_ref()),
            &["user"],
            &[],
        ));
        assert_eq!(s, r#"{"kind":"account","path":"user"}"#);
    }

    #[test]
    fn account_as_ref_resolves_account_root() {
        let s = expect_static(classify(syn::parse_quote!(user.as_ref()), &["user"], &[]));
        assert_eq!(s, r#"{"kind":"account","path":"user"}"#);
    }

    #[test]
    fn method_chain_resolves_arg_root() {
        let s = expect_static(classify(
            syn::parse_quote!(nonce.to_le_bytes()),
            &[],
            &["nonce"],
        ));
        assert_eq!(s, r#"{"kind":"arg","path":"nonce"}"#);
    }

    #[test]
    fn account_field_method_chain_is_unsupported() {
        expect_unsupported(classify(
            syn::parse_quote!(manager.next_oracle_id.to_le_bytes()),
            &["manager"],
            &[],
        ));
    }

    #[test]
    fn nested_account_address_chain_is_unsupported() {
        expect_unsupported(classify(
            syn::parse_quote!(manager.account().address().as_ref()),
            &["manager"],
            &[],
        ));
    }

    #[test]
    fn wrapped_account_ref_is_unsupported() {
        expect_unsupported(classify(
            syn::parse_quote!(u64::from(manager.next_oracle_id).to_le_bytes()),
            &["manager"],
            &[],
        ));
    }

    #[test]
    fn wrapped_arg_ref_is_opaque_expr() {
        expect_unsupported(classify(
            syn::parse_quote!(u64::from(next_oracle_id).to_le_bytes()),
            &[],
            &["next_oracle_id"],
        ));
    }

    #[test]
    fn string_literal_as_bytes_method_is_static_const() {
        let s = expect_static(classify(syn::parse_quote!("hi".as_bytes()), &[], &[]));
        assert_eq!(s, r#"{"kind":"const","value":[104,105]}"#);
    }

    #[test]
    fn const_path_is_evaluated_at_idl_build_time() {
        let ts = expect_runtime(classify(syn::parse_quote!(MY_PREFIX), &[], &[]));
        assert!(ts.contains("__idl_const_seed_json"), "got: {ts}");
    }

    #[test]
    fn marker_id_call_is_evaluated_at_idl_build_time() {
        let ts = expect_runtime(classify(syn::parse_quote!(System::id()), &[], &[]));
        assert!(ts.contains("__idl_const_seed_json"), "got: {ts}");
    }

    #[test]
    fn program_marker_id_call_flows_through_runtime_const_seed() {
        let fields = Vec::new();
        let args = Vec::new();
        let ts = expect_runtime(
            classify_program_seed(&syn::parse_quote!(System::id()), &fields, &args)
                .expect("program marker should be representable in the IDL"),
        );
        assert!(ts.contains("__idl_const_seed_json"), "got: {ts}");
        assert!(ts.contains("System :: id"), "got: {ts}");
    }

    #[test]
    fn local_field_program_seed_is_omitted() {
        let fields = vec!["config".to_string()];
        let args = Vec::new();
        assert!(
            classify_program_seed(&syn::parse_quote!(config.program_id), &fields, &args).is_none()
        );
    }

    #[test]
    fn macro_wrapped_local_field_program_seed_is_omitted() {
        let fields = vec!["config".to_string()];
        let args = Vec::new();
        assert!(classify_program_seed(
            &syn::parse_quote!(wrap!(config.program_id)),
            &fields,
            &args
        )
        .is_none());
    }

    #[test]
    fn local_binding_detector_sees_sibling_field_access() {
        let fields = vec!["config".to_string()];
        let args = Vec::new();
        assert!(expr_references_local_binding(
            &syn::parse_quote!(config.program_id),
            &fields,
            &args,
        ));
        assert!(!expr_references_local_binding(
            &syn::parse_quote!(System::id()),
            &fields,
            &args,
        ));
    }

    #[test]
    fn macro_detector_sees_expr_macro() {
        assert!(expr_contains_macro(&syn::parse_quote!(wrap!(
            config.program_id
        ))));
        assert!(!expr_contains_macro(&syn::parse_quote!(System::id())));
    }

    #[test]
    fn unknown_marker_id_call_is_evaluated_at_idl_build_time() {
        let ts = expect_runtime(classify(syn::parse_quote!(MyCustomProgram::id()), &[], &[]));
        assert!(ts.contains("__idl_const_seed_json"), "got: {ts}");
    }

    #[test]
    fn pda_object_emission_assembles_seeds_array_in_order() {
        let seeds = vec![
            SeedJson::Static(r#"{"kind":"const","value":[1]}"#.to_string()),
            SeedJson::Static(r#"{"kind":"account","path":"user"}"#.to_string()),
        ];
        let ts = pda_object_emission(&SeedListJson::Listed(seeds), None).to_string();
        // Both seed bodies are spliced in source order with the comma
        // separator between them.
        assert!(
            ts.contains(r#"{\"kind\":\"const\",\"value\":[1]}"#),
            "first seed missing: {ts}"
        );
        assert!(
            ts.contains(r#"{\"kind\":\"account\",\"path\":\"user\"}"#),
            "second seed missing: {ts}"
        );
        assert!(
            !ts.contains(r#""program""#),
            "no program override expected: {ts}"
        );
    }

    #[test]
    fn pda_object_emission_includes_program_when_set() {
        let seeds = vec![SeedJson::Static(
            r#"{"kind":"const","value":[1]}"#.to_string(),
        )];
        let prog = SeedJson::Static(r#"{"kind":"const","value":[2]}"#.to_string());
        let ts = pda_object_emission(&SeedListJson::Listed(seeds), Some(&prog)).to_string();
        // The program override gets its own runtime push under the
        // "program" key.
        assert!(ts.contains(r#",\"program\":"#), "missing program key: {ts}");
    }
}
