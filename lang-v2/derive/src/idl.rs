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
    syn::{Expr, Lit, Type},
};

/// Convert a Rust type to its IDL JSON representation (as a `serde_json`
/// value ready to splice into a containing `Value`). See [`rust_type_to_idl`]
/// for the stringified convenience wrapper.
pub fn rust_type_to_idl_value(ty: &Type) -> Value {
    type_str_to_idl_value(&quote!(#ty).to_string().replace(' ', ""))
}

/// String-returning convenience wrapper around [`rust_type_to_idl_value`].
/// Kept for callers that splice the result into runtime `format!()` templates.
pub fn rust_type_to_idl(ty: &Type) -> String {
    rust_type_to_idl_value(ty).to_string()
}

/// Convert a stringified Rust type to an IDL JSON value.
fn type_str_to_idl_value(s: &str) -> Value {
    let s = strip_ref_and_lifetime(s);
    let s = s.as_str();
    match s {
        "u8" | "u16" | "u32" | "u64" | "u128" | "i8" | "i16" | "i32" | "i64" | "i128" | "bool" => {
            Value::String(s.to_owned())
        }
        // Pod wrappers drop alignment to 1 for zero-copy accounts but
        // the byte representation matches the primitive (LE). Report
        // as primitives so the TS coder's default borsh path decodes
        // without a `types[]` entry. `PodVec<T, N>` stays defined —
        // its layout is non-trivial.
        "PodU16" => Value::String("u16".into()),
        "PodU32" => Value::String("u32".into()),
        "PodU64" => Value::String("u64".into()),
        "PodU128" => Value::String("u128".into()),
        "PodI16" => Value::String("i16".into()),
        "PodI32" => Value::String("i32".into()),
        "PodI64" => Value::String("i64".into()),
        "PodI128" => Value::String("i128".into()),
        "PodBool" => Value::String("bool".into()),
        "String" | "string" | "str" => Value::String("string".into()),
        "Pubkey" | "Address" | "pubkey" => Value::String("pubkey".into()),
        "bytes" | "[u8]" => Value::String("bytes".into()),
        // `&[T]` slice (no `;`) → vec<T>
        _ if s.starts_with('[') && s.ends_with(']') && !s.contains(';') => {
            let inner = &s[1..s.len() - 1];
            json!({ "vec": type_str_to_idl_value(inner) })
        }
        // `[T; N]` array
        _ if s.starts_with('[') && s.ends_with(']') && s.contains(';') => {
            let inner = &s[1..s.len() - 1];
            if let Some((ty_part, n_part)) = inner.split_once(';') {
                let ty_json = type_str_to_idl_value(ty_part);
                // Const expressions that aren't plain integer literals fall
                // through to 0 as a placeholder — matches the prior behavior.
                let size = n_part.trim().parse::<usize>().unwrap_or(0);
                json!({ "array": [ty_json, size] })
            } else {
                json!({ "defined": { "name": s } })
            }
        }
        _ if s.starts_with("Vec<") => {
            let inner = s
                .strip_prefix("Vec<")
                .and_then(|s| s.strip_suffix('>'))
                .expect("syn-generated type string has balanced angle brackets");
            json!({ "vec": type_str_to_idl_value(inner) })
        }
        _ if s.starts_with("Option<") => {
            let inner = s
                .strip_prefix("Option<")
                .and_then(|s| s.strip_suffix('>'))
                .expect("syn-generated type string has balanced angle brackets");
            json!({ "option": type_str_to_idl_value(inner) })
        }
        _ if s.starts_with("Box<") => {
            let inner = s
                .strip_prefix("Box<")
                .and_then(|s| s.strip_suffix('>'))
                .expect("syn-generated type string has balanced angle brackets");
            type_str_to_idl_value(inner)
        }
        other => json!({ "defined": { "name": strip_type_generics(other) } }),
    }
}

/// Drop the `<...>` suffix on a user-defined type name.
///
/// `MixedArgs<'_>` / `MixedArgs<'info>` → `MixedArgs`.
/// `PodVec<PodU64, 16>` → `PodVec`.
///
/// The IDL spec's `IdlType::Defined { name, generics }` models generic
/// references structurally (spec:284+), but the v2 derive doesn't yet emit
/// the `generics` payload. Meanwhile, `#[derive(IdlType)]` registers types
/// under the bare ident (no `<...>`), so leaking the generic suffix into
/// the reference produces a `{"defined":{"name":"Foo<'_>"}}` that never
/// resolves against the `types[]` entry named `"Foo"`. Strip here so the
/// two sides agree — downstream TS clients used to patch this at runtime
/// (`tests/shared.ts::loadIdl`).
///
/// Limitation: multiple instantiations of the same generic type
/// (`PodVec<PodU64, 16>` + `PodVec<PodU32, 8>`) collapse to the same
/// `"PodVec"` defined name. Fine for today's single-instantiation
/// programs; a proper fix needs full generics emission.
fn strip_type_generics(name: &str) -> &str {
    match name.find('<') {
        Some(idx) => &name[..idx],
        None => name,
    }
}

/// Strip a leading `&` reference and any `'lifetime` annotation from a type
/// string, so `&'a [u64]` → `[u64]`. Leaves nested types alone.
fn strip_ref_and_lifetime(s: &str) -> String {
    let s = s.trim();
    let s = s.strip_prefix('&').unwrap_or(s).trim_start();
    let s = if let Some(rest) = s.strip_prefix('\'') {
        match rest.find(|c: char| c.is_whitespace() || c == '[' || c == ',') {
            Some(pos) => rest[pos..].trim_start().to_owned(),
            None => String::new(),
        }
    } else {
        s.to_owned()
    };
    s.trim_start_matches("mut ").trim().to_owned()
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
    pub is_optional: bool,
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
    /// Holds either a resolvable constant path / const-fn call (which the
    /// Anchor CLI can turn into a base58 pubkey) or a dotted field path
    /// like `data.authority` that clients walk at resolution time.
    pub address_override: Option<&'a str>,
    /// Set when this field is a `Nested<Inner>`, carrying the inner
    /// struct type. The emission splices the inner struct's own
    /// `__idl_accounts()` into the outer array instead of producing a
    /// single account entry for the `Nested` wrapper, so the IDL's
    /// `accounts[]` list flattens the nested block in source order —
    /// matching how the runtime actually consumes accounts.
    pub nested_inner_ty: Option<&'a Type>,
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
            // `Nested<Inner>` flattens at IDL time. Ask the inner struct
            // for its own `__idl_accounts()` string, strip the outer
            // `[` / `]`, and splice the element sequence in place. The
            // outer's join-with-`,` loop then produces a single flat
            // array in source order.
            if let Some(inner) = f.nested_inner_ty {
                return quote! {
                    {
                        let __inner = <#inner>::__idl_accounts();
                        // Strip the bracketing `[`/`]` produced by the
                        // inner emission. Use char-indexed slicing
                        // rather than `trim_matches`, which would also
                        // eat balanced brackets from inside string
                        // literals (there are none today, but the
                        // narrow form is future-proof).
                        let __bytes = __inner.as_bytes();
                        if __bytes.len() >= 2
                            && __bytes[0] == b'['
                            && __bytes[__bytes.len() - 1] == b']'
                        {
                            __inner[1..__inner.len() - 1].to_string()
                        } else {
                            __inner
                        }
                    }
                };
            }
            let name = f.name;
            let writable_json = if f.writable { ",\"writable\":true" } else { "" };
            let optional_json = if f.is_optional {
                ",\"optional\":true"
            } else {
                ""
            };
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
                    let __pda_json: anchor_lang_v2::__alloc::string::String = {
                        let __body: anchor_lang_v2::__alloc::string::String = #ts;
                        anchor_lang_v2::__alloc::format!(",\"pda\":{}", __body)
                    };
                },
                None => quote! {
                    let __pda_json: anchor_lang_v2::__alloc::string::String =
                        anchor_lang_v2::__alloc::string::String::new();
                },
            };
            // `#[account(address = <expr>)]` override, pre-formatted as a
            // JSON fragment. When set, takes precedence over the wrapper
            // type's `__IDL_ADDRESS` — emitted at macro time so no runtime
            // branch is needed to pick one.
            let address_override_json = f
                .address_override
                .map(|s| format!(",\"address\":\"{s}\""))
                .unwrap_or_default();
            let init_signer = f.init_signer;
            if let Some(ty) = f.field_ty {
                let addr_json_expr = if f.address_override.is_some() {
                    quote! {
                        let __addr_json: anchor_lang_v2::__alloc::string::String =
                            anchor_lang_v2::__alloc::string::String::from(#address_override_json);
                    }
                } else {
                    quote! {
                        let __addr = <#ty as anchor_lang_v2::IdlAccountType>::__IDL_ADDRESS;
                        let __addr_json: anchor_lang_v2::__alloc::string::String = match __addr {
                            Some(a) => anchor_lang_v2::__alloc::format!(",\"address\":\"{}\"", a),
                            None => anchor_lang_v2::__alloc::string::String::new(),
                        };
                    }
                };
                quote! {
                    {
                        // Trait-const OR compile-time init_signer flag.
                        // Kept separate so a Signer + init-without-seeds
                        // combo still renders exactly one `"signer":true`.
                        let __signer = <#ty as anchor_lang_v2::IdlAccountType>::__IDL_IS_SIGNER
                            || #init_signer;
                        let __signer_json: &str = if __signer { ",\"signer\":true" } else { "" };
                        #addr_json_expr
                        #pda_json_expr
                        anchor_lang_v2::__alloc::format!(
                            "{{\"name\":\"{}\"{}{}{}{}{}{}{}}}",
                            #name,
                            #writable_json,
                            __signer_json,
                            __addr_json,
                            #optional_json,
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
                quote! {
                    {
                        #pda_json_expr
                        anchor_lang_v2::__alloc::format!(
                            "{{\"name\":\"{}\"{}{}{}{}{}{}{}}}",
                            #name,
                            #writable_json,
                            #signer_json,
                            #address_override_json,
                            #optional_json,
                            #relations_json,
                            #docs_json,
                            __pda_json,
                        )
                    }
                }
            }
        })
        .collect();

    quote! {
        /// **Opaque / unstable.** Returns the IDL JSON for this Accounts
        /// struct's account list. Implementation detail of the IDL build
        /// pipeline; do not rely on the shape or call this directly.
        #[doc(hidden)]
        pub fn __idl_accounts() -> anchor_lang_v2::__alloc::string::String {
            let __parts: anchor_lang_v2::__alloc::vec::Vec<
                anchor_lang_v2::__alloc::string::String
            > = anchor_lang_v2::__alloc::vec![#(#parts),*];
            let mut __s = anchor_lang_v2::__alloc::string::String::from("[");
            let mut __first = true;
            for __p in &__parts {
                // A `Nested<Inner>` whose inner has zero fields contributes
                // an empty part — skip it so we don't emit `,,` or a leading
                // comma.
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
pub fn build_args_json(args: &[(&syn::Ident, &Type)]) -> String {
    let arr: Vec<Value> = args
        .iter()
        .map(|(name, ty)| {
            json!({
                "name": name.to_string(),
                "type": rust_type_to_idl_value(ty),
            })
        })
        .collect();
    Value::Array(arr).to_string()
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
    /// `bytemuck` Pod + `repr(C)`. Both fields show up in the JSON.
    BytemuckRepr,
}

/// Pre-split IDL type strings emitted by the derive at macro-expansion time.
///
/// The runtime print test no longer parses JSON — it concatenates these
/// strings directly. That lets `lang-v2` avoid a runtime `serde_json`
/// dependency or local `idl-build` feature; derive output still emits
/// `feature = "idl-build"` cfgs into user crates.
pub struct IdlTypeStrings {
    /// `{"name":"X","discriminator":[…]}` for the program-level
    /// `accounts[]` array (spec:137-140). `None` when the discriminator
    /// is empty — i.e. plain `#[derive(IdlType)]` types that only
    /// contribute to `types[]`.
    pub account_entry: Option<String>,
    /// `IdlTypeDef` JSON (spec:176-188) — `name`, optional `docs`, the
    /// `serialization` / `repr` pair, and the inner `type` object. Never
    /// carries `discriminator`; that field belongs only on the
    /// accounts entry.
    pub type_def: String,
}

/// Build pre-split IDL type strings from struct fields.
///
/// `kind` selects the `serialization` / `repr` metadata emitted onto the
/// type definition. Zero-copy `#[account]` and `#[event(bytemuck)]` pass
/// `TypeKind::BytemuckRepr`; borsh/wincode shapes pass `TypeKind::Borsh`
/// (the default, which suppresses both fields).
pub fn build_type_strings(
    name: &str,
    disc: &[u8],
    docs: &[String],
    fields: &syn::punctuated::Punctuated<syn::Field, syn::token::Comma>,
    kind: TypeKind,
) -> IdlTypeStrings {
    let mut type_def_obj = build_type_def_header(name, docs, kind);
    let field_values: Vec<Value> = fields.iter().map(named_field_value).collect();
    type_def_obj.insert(
        "type".into(),
        json!({ "kind": "struct", "fields": field_values }),
    );
    IdlTypeStrings {
        account_entry: build_account_entry(name, disc),
        type_def: Value::Object(type_def_obj).to_string(),
    }
}

/// Build pre-split IDL type strings from enum variants. Matches the spec's
/// `IdlTypeDefTy::Enum { variants }` shape (idl/spec/src/lib.rs:237-248).
/// Each variant is emitted as `{"name": ..., "fields"?: ...}` where `fields`
/// is either a named-field object array (struct-like variants), a tuple of
/// types (tuple-like variants), or omitted entirely (unit variants) —
/// consistent with `IdlDefinedFields`'s `#[serde(untagged)]` shape.
///
/// Only `TypeKind::Borsh` is really meaningful for enums today — bytemuck
/// enums are rare. We accept the same `kind` parameter for shape symmetry
/// with `build_type_strings`.
pub fn build_enum_type_strings(
    name: &str,
    disc: &[u8],
    docs: &[String],
    variants: &syn::punctuated::Punctuated<syn::Variant, syn::token::Comma>,
    kind: TypeKind,
) -> IdlTypeStrings {
    let mut type_def_obj = build_type_def_header(name, docs, kind);
    let variant_values: Vec<Value> = variants
        .iter()
        .map(|v| {
            let mut obj = serde_json::Map::new();
            obj.insert("name".into(), Value::String(v.ident.to_string()));
            match &v.fields {
                syn::Fields::Unit => {}
                syn::Fields::Named(named) => {
                    let fields: Vec<Value> = named.named.iter().map(named_field_value).collect();
                    obj.insert("fields".into(), Value::Array(fields));
                }
                syn::Fields::Unnamed(unnamed) => {
                    let tys: Vec<Value> = unnamed
                        .unnamed
                        .iter()
                        .map(|f| rust_type_to_idl_value(&f.ty))
                        .collect();
                    obj.insert("fields".into(), Value::Array(tys));
                }
            }
            Value::Object(obj)
        })
        .collect();
    type_def_obj.insert(
        "type".into(),
        json!({ "kind": "enum", "variants": variant_values }),
    );
    IdlTypeStrings {
        account_entry: build_account_entry(name, disc),
        type_def: Value::Object(type_def_obj).to_string(),
    }
}

/// Compose the program-level `accounts[]` entry. Returns `None` when the
/// discriminator is empty (plain `IdlType` types that don't appear in
/// `accounts[]`).
fn build_account_entry(name: &str, disc: &[u8]) -> Option<String> {
    if disc.is_empty() {
        return None;
    }
    Some(
        json!({
            "name": name,
            "discriminator": disc_json_value(disc),
        })
        .to_string(),
    )
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
) -> serde_json::Map<String, Value> {
    let mut out = serde_json::Map::new();
    out.insert("name".into(), Value::String(name.to_owned()));
    if !docs.is_empty() {
        out.insert("docs".into(), docs_value(docs));
    }
    match kind {
        TypeKind::Borsh => {}
        TypeKind::BytemuckRepr => {
            out.insert("serialization".into(), Value::String("bytemuck".into()));
            out.insert("repr".into(), json!({ "kind": "c" }));
        }
    }
    out
}

/// Build a named `IdlField` value — `{name, type, docs?}` — for a single
/// `syn::Field`. Used by both struct field and enum-variant struct-field
/// emission.
fn named_field_value(f: &syn::Field) -> Value {
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
    obj.insert("type".into(), rust_type_to_idl_value(&f.ty));
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

/// Classified seed expression. Only explicitly recognized shapes carry
/// structured IDL metadata; unsupported regular seed expressions are emitted
/// as opaque `{"kind":"expr"}` placeholders. Program seed expressions can use
/// the runtime variant because `seeds::program` must evaluate to address bytes.
#[derive(Clone)]
pub enum SeedJson {
    /// Pre-serialized JSON object — known at macro time.
    Static(String),
    /// Token expression evaluating to `alloc::string::String` at IDL-build
    /// time.
    Runtime(TokenStream2),
}

impl SeedJson {
    /// Token expression evaluating to `String` at IDL-build time.
    pub fn into_string_expr(self) -> TokenStream2 {
        match self {
            SeedJson::Static(s) => quote! {
                anchor_lang_v2::__alloc::string::String::from(#s)
            },
            SeedJson::Runtime(ts) => ts,
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
/// Anything else (a constant ref like `MY_PREFIX`, a const-fn call like
/// `<Marker as Id>::id()`, wrapped account/arg field access, etc.) is opaque:
/// `{"kind":"expr"}`. Runtime constraints still evaluate the original Rust
/// expression; this only avoids over-promising IDL/client metadata.
pub fn classify_seed(expr: &Expr, field_names: &[String], ix_arg_names: &[String]) -> SeedJson {
    classify_seed_inner(expr, field_names, ix_arg_names)
}

/// Classify `seeds::program = <expr>` into an IDL seed. Unlike arbitrary PDA
/// seed expressions, this expression is semantically a program id, so opaque
/// expressions are evaluated during IDL build and emitted as const bytes.
pub fn classify_program_seed(
    expr: &Expr,
    field_names: &[String],
    ix_arg_names: &[String],
) -> SeedJson {
    let seed = classify_seed_inner(expr, field_names, ix_arg_names);
    match seed {
        SeedJson::Static(ref s) if s == r#"{"kind":"expr"}"# => SeedJson::Runtime(quote! {
            anchor_lang_v2::idl_build::__idl_const_seed_json(#expr)
        }),
        _ => seed,
    }
}

fn static_seed(value: Value) -> SeedJson {
    SeedJson::Static(value.to_string())
}

fn classify_seed_inner(expr: &Expr, field_names: &[String], ix_arg_names: &[String]) -> SeedJson {
    // Peel `&<inner>` wrappers — they're common in seed expressions and
    // always transparent to classification.
    let mut cur = expr;
    while let Expr::Reference(r) = cur {
        cur = &r.expr;
    }

    if let Expr::Lit(lit) = cur {
        match &lit.lit {
            Lit::ByteStr(bs) => return static_seed(const_seed_value(&bs.value())),
            Lit::Str(s) => return static_seed(const_seed_value(s.value().as_bytes())),
            Lit::Byte(b) => return static_seed(const_seed_value(&[b.value()])),
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
            return static_seed(const_seed_value(&b));
        }
    }

    if let Some(root) = account_seed_root_ident(cur) {
        if field_names.contains(&root) {
            return static_seed(account_seed_value(&root));
        }
    }

    if let Some(root) = arg_seed_root_ident(cur) {
        if ix_arg_names.contains(&root) {
            return static_seed(arg_seed_value(&root));
        }
    }

    if let Some(s) = string_as_bytes(cur) {
        return static_seed(const_seed_value(s.value().as_bytes()));
    }

    static_seed(json!({ "kind": "expr" }))
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
pub fn pda_object_emission(seeds: &[SeedJson], program: Option<&SeedJson>) -> TokenStream2 {
    let seed_pushes: Vec<TokenStream2> = seeds
        .iter()
        .enumerate()
        .map(|(i, s)| {
            let expr = s.clone().into_string_expr();
            let comma = if i == 0 { "" } else { "," };
            quote! {
                __pda.push_str(#comma);
                __pda.push_str(&{ #expr });
            }
        })
        .collect();
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
            let mut __pda = anchor_lang_v2::__alloc::string::String::from("{\"seeds\":[");
            #(#seed_pushes)*
            __pda.push(']');
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
        }
    }

    fn expect_runtime(seed: SeedJson) -> String {
        match seed {
            SeedJson::Static(s) => panic!("expected Runtime seed, got Static: {s}"),
            SeedJson::Runtime(ts) => ts.to_string(),
        }
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
    fn byte_array_with_non_u8_is_opaque_expr() {
        let s = expect_static(classify(syn::parse_quote!([999, 2]), &[], &[]));
        assert_eq!(s, r#"{"kind":"expr"}"#);
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
    fn account_field_method_chain_is_opaque_expr() {
        let s = expect_static(classify(
            syn::parse_quote!(manager.next_oracle_id.to_le_bytes()),
            &["manager"],
            &[],
        ));
        assert_eq!(s, r#"{"kind":"expr"}"#);
    }

    #[test]
    fn nested_account_address_chain_is_opaque_expr() {
        let s = expect_static(classify(
            syn::parse_quote!(manager.account().address().as_ref()),
            &["manager"],
            &[],
        ));
        assert_eq!(s, r#"{"kind":"expr"}"#);
    }

    #[test]
    fn wrapped_account_ref_is_opaque_expr() {
        let s = expect_static(classify(
            syn::parse_quote!(u64::from(manager.next_oracle_id).to_le_bytes()),
            &["manager"],
            &[],
        ));
        assert_eq!(s, r#"{"kind":"expr"}"#);
    }

    #[test]
    fn wrapped_arg_ref_is_opaque_expr() {
        let s = expect_static(classify(
            syn::parse_quote!(u64::from(next_oracle_id).to_le_bytes()),
            &[],
            &["next_oracle_id"],
        ));
        assert_eq!(s, r#"{"kind":"expr"}"#);
    }

    #[test]
    fn string_literal_as_bytes_method_is_static_const() {
        let s = expect_static(classify(syn::parse_quote!("hi".as_bytes()), &[], &[]));
        assert_eq!(s, r#"{"kind":"const","value":[104,105]}"#);
    }

    #[test]
    fn const_path_is_opaque_expr() {
        let s = expect_static(classify(syn::parse_quote!(MY_PREFIX), &[], &[]));
        assert_eq!(s, r#"{"kind":"expr"}"#);
    }

    #[test]
    fn marker_id_call_is_opaque_expr() {
        let s = expect_static(classify(syn::parse_quote!(System::id()), &[], &[]));
        assert_eq!(s, r#"{"kind":"expr"}"#);
    }

    #[test]
    fn program_marker_id_call_flows_through_runtime_const_seed() {
        let fields = Vec::new();
        let args = Vec::new();
        let ts = expect_runtime(classify_program_seed(
            &syn::parse_quote!(System::id()),
            &fields,
            &args,
        ));
        assert!(ts.contains("__idl_const_seed_json"), "got: {ts}");
        assert!(ts.contains("System :: id"), "got: {ts}");
    }

    #[test]
    fn unknown_marker_id_call_is_opaque_expr() {
        let s = expect_static(classify(syn::parse_quote!(MyCustomProgram::id()), &[], &[]));
        assert_eq!(s, r#"{"kind":"expr"}"#);
    }

    #[test]
    fn pda_object_emission_assembles_seeds_array_in_order() {
        let seeds = vec![
            SeedJson::Static(r#"{"kind":"const","value":[1]}"#.to_string()),
            SeedJson::Static(r#"{"kind":"account","path":"user"}"#.to_string()),
        ];
        let ts = pda_object_emission(&seeds, None).to_string();
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
        let ts = pda_object_emission(&seeds, Some(&prog)).to_string();
        // The program override gets its own runtime push under the
        // "program" key.
        assert!(ts.contains(r#",\"program\":"#), "missing program key: {ts}");
    }
}
