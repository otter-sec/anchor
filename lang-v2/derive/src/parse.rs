use {
    proc_macro2::TokenStream as TokenStream2,
    quote::{quote, quote_spanned},
    syn::spanned::Spanned,
    syn::{ext::IdentExt, parse::ParseStream, Attribute, Expr, Ident, Token, Type},
};

/// snake_case → PascalCase + `Constraint` suffix, for looking up the
/// marker type on the `AccountConstraint` trait. Shared by the top-level
/// `namespace::key = value` parser and the `update(...)` parser.
fn constraint_key_ident(key: &str) -> String {
    let mut out = String::with_capacity(key.len() + "Constraint".len());
    let mut upper_next = true;
    for ch in key.chars() {
        if ch == '_' {
            upper_next = true;
        } else if upper_next {
            out.extend(ch.to_uppercase());
            upper_next = false;
        } else {
            out.push(ch);
        }
    }
    out.push_str("Constraint");
    out
}

/// A namespaced constraint like `token::mint = expr`.
pub struct NamespacedConstraint {
    /// e.g. "token"
    pub namespace: String,
    /// e.g. "MintConstraint" (PascalCased key + `Constraint` suffix, used
    /// to locate the marker on the `AccountConstraint` trait).
    pub key: String,
    /// e.g. "mint" (original lowercase key, used as init param field name)
    pub raw_key: String,
    /// The RHS expression.
    pub value: Expr,
    /// True if the RHS is a simple ident (field reference → call .account()).
    /// False if it's a literal or complex expression (pass directly).
    pub is_field_ref: bool,
    /// True if parsed from inside an `update(...)` wrapper. Update
    /// entries dispatch through `AccountConstraint::update` instead of
    /// `check`, and skip the init-param thread-through.
    pub is_update: bool,
}

pub struct AccountAttrs {
    pub is_mut: bool,
    pub is_signer: bool,
    pub is_init: bool,
    pub is_init_if_needed: bool,
    pub is_zeroed: bool,
    pub is_executable: bool,
    pub is_dup: bool,
    /// None = no bump attr, Some(None) = `bump` without value, Some(Some(expr)) = `bump = expr`
    pub bump: Option<Option<Expr>>,
    pub payer: Option<Ident>,
    pub space: Option<Expr>,
    pub seeds: Option<Expr>,
    /// Override program_id for PDA derivation: `seeds::program = expr`
    pub seeds_program: Option<Expr>,
    /// `(keyword_span, target, error_expr)`. `keyword_span` is the span of
    /// the `has_one` keyword itself so the deprecation warning emitted by
    /// the codegen can underline the original attribute.
    pub has_one: Vec<(proc_macro2::Span, Ident, Option<Expr>)>,
    pub address: Option<Expr>,
    pub address_error: Option<Expr>,
    pub owner: Option<Expr>,
    pub owner_error: Option<Expr>,
    pub close: Option<Ident>,
    /// Arbitrary boolean constraints in source order. Each entry is `(expr,
    /// optional custom-error expr)`. Both `constraint = expr [@ err]` and
    /// the parenthesized `constraint(expr [@ err])` push here, and any
    /// number of either spelling may appear in a single `#[account(...)]` —
    /// they are emitted as checks in the order written.
    pub raw_constraints: Vec<(Expr, Option<Expr>)>,
    pub realloc: Option<Expr>,
    pub realloc_payer: Option<Ident>,
    pub realloc_zero: bool,
    /// Namespaced constraints: token::mint, mint::authority, etc.
    pub namespaced: Vec<NamespacedConstraint>,
}

struct AssociatedTokenInit {
    mint: Ident,
    authority: Ident,
    token_program: Ident,
}

/// PDA metadata produced by seed classification. Each entry is a pre-built
/// Per-seed metadata in either pre-serialized JSON form (static cases) or
/// as a runtime token expression (const-evaluatable fallback). `program`
/// mirrors the optional `seeds::program = expr` override.
#[derive(Clone)]
pub struct IdlPdaMeta {
    pub seeds: Vec<crate::idl::SeedJson>,
    pub program: Option<crate::idl::SeedJson>,
}

pub fn parse_account_attrs(attrs: &[Attribute]) -> syn::Result<AccountAttrs> {
    let mut result = AccountAttrs {
        is_mut: false,
        is_signer: false,
        is_init: false,
        is_init_if_needed: false,
        is_zeroed: false,
        is_executable: false,
        is_dup: false,
        bump: None,
        payer: None,
        space: None,
        seeds: None,
        seeds_program: None,
        has_one: Vec::new(),
        address: None,
        address_error: None,
        owner: None,
        owner_error: None,
        close: None,
        raw_constraints: Vec::new(),
        realloc: None,
        realloc_payer: None,
        realloc_zero: false,
        namespaced: Vec::new(),
    };

    for attr in attrs {
        if !attr.path().is_ident("account") {
            continue;
        }
        attr.parse_args_with(|input: ParseStream| {
            while !input.is_empty() {
                let ident = Ident::parse_any(input)?;
                match ident.to_string().as_str() {
                    "mut" => result.is_mut = true,
                    "init" => {
                        result.is_init = true;
                        result.is_mut = true;
                    }
                    "init_if_needed" => {
                        result.is_init_if_needed = true;
                        result.is_mut = true;
                    }
                    "zeroed" => {
                        result.is_zeroed = true;
                        result.is_mut = true;
                    }
                    "bump" => {
                        if input.peek(Token![=]) {
                            input.parse::<Token![=]>()?;
                            result.bump = Some(Some(input.parse()?));
                        } else {
                            result.bump = Some(None);
                        }
                    }
                    "signer" => result.is_signer = true,
                    "executable" => result.is_executable = true,
                    "dup" => {
                        return Err(syn::Error::new(
                            ident.span(),
                            "`dup` bypasses duplicate-account safety checks and must be \
                             explicitly marked unsafe: use `unsafe(dup)`",
                        ));
                    }
                    "unsafe" => {
                        let content;
                        syn::parenthesized!(content in input);
                        let inner: Ident = content.parse()?;
                        match inner.to_string().as_str() {
                            "dup" => {
                                result.is_dup = true;
                                result.is_mut = true;
                            }
                            _ => {
                                return Err(syn::Error::new(
                                    inner.span(),
                                    format!("unknown unsafe constraint `{inner}`"),
                                ));
                            }
                        }
                    }
                    "update" => {
                        // `update(ns::key = val, ns2::key2 = val2, ...)` —
                        // each inner entry is a namespaced constraint that
                        // dispatches through `AccountConstraint::update`
                        // instead of `check`. Implies `mut` since update
                        // hooks mutate the account.
                        let content;
                        syn::parenthesized!(content in input);
                        result.is_mut = true;
                        while !content.is_empty() {
                            let ns_ident: Ident = Ident::parse_any(&content)?;
                            content.parse::<Token![::]>()?;
                            let key_ident: Ident = Ident::parse_any(&content)?;
                            content.parse::<Token![=]>()?;
                            let is_field_ref = content.peek(syn::Ident);
                            let value: Expr = content.parse()?;
                            let raw_key = key_ident.to_string();
                            let key = constraint_key_ident(&raw_key);
                            result.namespaced.push(NamespacedConstraint {
                                namespace: ns_ident.to_string(),
                                key,
                                raw_key,
                                value,
                                is_field_ref,
                                is_update: true,
                            });
                            if !content.is_empty() {
                                content.parse::<Token![,]>()?;
                            }
                        }
                    }
                    "payer" => {
                        input.parse::<Token![=]>()?;
                        result.payer = Some(input.parse()?);
                    }
                    "space" => {
                        input.parse::<Token![=]>()?;
                        result.space = Some(input.parse()?);
                    }
                    "seeds" if input.peek(Token![=]) => {
                        input.parse::<Token![=]>()?;
                        result.seeds = Some(input.parse()?);
                    }
                    // `seeds::program = expr` falls through to the
                    // namespaced-path handler below. Adding an explicit
                    // `seeds` arm without a peek check would eat the `seeds`
                    // ident and then fail to parse the following `::`.
                    "has_one" => {
                        let keyword_span = ident.span();
                        input.parse::<Token![=]>()?;
                        let target: Ident = input.parse()?;
                        let err = if input.peek(Token![@]) {
                            input.parse::<Token![@]>()?;
                            Some(input.parse()?)
                        } else {
                            None
                        };
                        result.has_one.push((keyword_span, target, err));
                    }
                    "address" => {
                        input.parse::<Token![=]>()?;
                        result.address = Some(input.parse()?);
                        if input.peek(Token![@]) {
                            input.parse::<Token![@]>()?;
                            result.address_error = Some(input.parse()?);
                        }
                    }
                    "owner" => {
                        input.parse::<Token![=]>()?;
                        result.owner = Some(input.parse()?);
                        if input.peek(Token![@]) {
                            input.parse::<Token![@]>()?;
                            result.owner_error = Some(input.parse()?);
                        }
                    }
                    "realloc" => {
                        input.parse::<Token![=]>()?;
                        result.realloc = Some(input.parse()?);
                        result.is_mut = true;
                    }
                    "realloc_payer" => {
                        input.parse::<Token![=]>()?;
                        result.realloc_payer = Some(input.parse()?);
                    }
                    "realloc_zero" => {
                        input.parse::<Token![=]>()?;
                        let val: syn::LitBool = input.parse()?;
                        result.realloc_zero = val.value;
                    }
                    "close" => {
                        input.parse::<Token![=]>()?;
                        result.close = Some(input.parse()?);
                    }
                    "constraint" => {
                        // Two accepted spellings, both pushing to the same
                        // ordered list:
                        //   - `constraint = expr [@ err]`     (legacy)
                        //   - `constraint(expr [@ err])`      (parens)
                        // Either form may appear multiple times in a single
                        // `#[account(...)]`; checks fire in source order.
                        if input.peek(syn::token::Paren) {
                            let content;
                            syn::parenthesized!(content in input);
                            let expr: Expr = content.parse()?;
                            reject_obvious_non_bool_constraint(&expr)?;
                            let err = if content.peek(Token![@]) {
                                content.parse::<Token![@]>()?;
                                Some(content.parse()?)
                            } else {
                                None
                            };
                            if !content.is_empty() {
                                return Err(content.error(
                                    "expected a single `expr [@ err]` inside `constraint(...)`; \
                                     write multiple `constraint(...)` entries to chain checks",
                                ));
                            }
                            result.raw_constraints.push((expr, err));
                        } else {
                            input.parse::<Token![=]>()?;
                            let expr: Expr = input.parse()?;
                            reject_obvious_non_bool_constraint(&expr)?;
                            let err = if input.peek(Token![@]) {
                                input.parse::<Token![@]>()?;
                                Some(input.parse()?)
                            } else {
                                None
                            };
                            result.raw_constraints.push((expr, err));
                        }
                    }
                    _ => {
                        // Check for namespaced constraint: namespace::key = value
                        if input.peek(Token![::]) {
                            input.parse::<Token![::]>()?;
                            let key_ident = Ident::parse_any(input)?;
                            // seeds::program = expr — special case, stored separately
                            if ident == "seeds" && key_ident == "program" {
                                input.parse::<Token![=]>()?;
                                result.seeds_program = Some(input.parse()?);
                                if !input.is_empty() {
                                    input.parse::<Token![,]>()?;
                                }
                                continue;
                            }
                            input.parse::<Token![=]>()?;
                            // Peek to determine if RHS is a simple ident (field ref)
                            // or a literal/expression (value).
                            let is_field_ref = input.peek(syn::Ident);
                            let value: Expr = input.parse()?;
                            let raw_key = key_ident.to_string();
                            let key = constraint_key_ident(&raw_key);
                            result.namespaced.push(NamespacedConstraint {
                                namespace: ident.to_string(),
                                key,
                                raw_key,
                                value,
                                is_field_ref,
                                is_update: false,
                            });
                        } else {
                            // No `::` follows — not a namespaced constraint.
                            // Reject to catch typos like `singler` instead of `signer`.
                            return Err(syn::Error::new(
                                ident.span(),
                                format!("unknown account constraint `{ident}`"),
                            ));
                        }
                    }
                }
                if !input.is_empty() {
                    input.parse::<Token![,]>()?;
                }
            }
            Ok(())
        })?;
    }

    // Reject `init` + `bump = <expr>` (mirroring Anchor v1). Account
    // creation requires an off-curve address, which is only guaranteed by
    // the canonical bump returned by `find_program_address`. A caller-
    // supplied bump could be non-canonical and either create the wrong
    // PDA or fail under the runtime's curve check, so we don't allow the
    // combination at all.
    //
    // `init_if_needed` is different: the create branch still uses the
    // canonical bump, while the existing-account branch can verify an
    // explicit stored bump after loading the account.
    if result.is_init && matches!(result.bump, Some(Some(_))) {
        if let Some(Some(ref bump_expr)) = result.bump {
            return Err(syn::Error::new(
                syn::spanned::Spanned::span(bump_expr),
                "`bump = <expr>` is not allowed with `init`: account creation \
                 must use the canonical bump (write `bump` without a value)",
            ));
        }
    }

    Ok(result)
}

fn reject_obvious_non_bool_constraint(expr: &Expr) -> syn::Result<()> {
    if let Expr::Lit(expr_lit) = expr {
        if !matches!(expr_lit.lit, syn::Lit::Bool(_)) {
            return Err(syn::Error::new_spanned(
                expr,
                "`constraint` expects a boolean expression; non-boolean literals like strings \
                 and numbers are rejected",
            ));
        }
    }
    Ok(())
}

pub fn field_ty_str(ty: &Type) -> String {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return seg.ident.to_string();
        }
    }
    String::new()
}

/// Namespaced constraints whose values are threaded as init-time `Params`
/// fields via `AccountInitialize::Params`. Only built-in namespaces that
/// correspond to SPL account types belong here — every other namespace
/// (including all third-party constraints) is runtime-only and dispatches
/// through the `AccountConstraint` trait.
fn has_init_params(ns: &str) -> bool {
    matches!(ns, "token" | "mint" | "associated_token")
}

/// Returns `true` when the namespace is runtime-only: its values are
/// applied via `AccountConstraint::{init, check, update, exit}` rather
/// than being threaded as init-time `Params` fields.
///
/// Any namespace that is NOT a known init-param provider is runtime-only,
/// so third-party crates can define arbitrary namespaced constraints
/// (e.g. `my_ns::min_balance = 1_000_000`) without changes to this file.
pub fn is_runtime_only_constraint_ns(ns: &str) -> bool {
    !has_init_params(ns)
}

fn expr_as_field_ident(expr: &Expr) -> Option<Ident> {
    let Expr::Path(path) = expr else {
        return None;
    };
    if path.qself.is_some() || path.path.segments.len() != 1 {
        return None;
    }
    Some(path.path.segments[0].ident.clone())
}

fn field_offset_expr(
    field_offsets: &[(String, TokenStream2)],
    ident: &Ident,
) -> syn::Result<TokenStream2> {
    let name = ident.to_string();
    field_offsets
        .iter()
        .find_map(|(field, offset)| (field == &name).then(|| offset.clone()))
        .ok_or_else(|| {
            syn::Error::new(
                ident.span(),
                format!("associated_token constraint references unknown account `{name}`"),
            )
        })
}

fn parse_associated_token_init(
    attrs: &AccountAttrs,
    field_names: &[String],
) -> syn::Result<Option<AssociatedTokenInit>> {
    let mut mint = None;
    let mut authority = None;
    let mut token_program = None;

    for nc in attrs
        .namespaced
        .iter()
        .filter(|nc| nc.namespace == "associated_token")
    {
        let target = match nc.raw_key.as_str() {
            "mint" => &mut mint,
            "authority" => &mut authority,
            "token_program" => &mut token_program,
            _ => {
                return Err(syn::Error::new(
                    nc.value.span(),
                    format!("unknown `associated_token` constraint `{}`", nc.raw_key),
                ));
            }
        };

        let Some(ident) = expr_as_field_ident(&nc.value) else {
            return Err(syn::Error::new(
                nc.value.span(),
                "associated_token constraints currently require sibling account field references",
            ));
        };

        if !field_names.iter().any(|name| name == &ident.to_string()) {
            return Err(syn::Error::new(
                ident.span(),
                format!(
                    "associated_token constraint references unknown account `{}`",
                    ident
                ),
            ));
        }

        *target = Some(ident);
    }

    if mint.is_none() && authority.is_none() && token_program.is_none() {
        return Ok(None);
    }

    if attrs.seeds.is_some() {
        return Err(syn::Error::new(
            attrs.seeds.as_ref().unwrap().span(),
            "`associated_token` constraints cannot be used with `seeds`",
        ));
    }

    let mint = mint.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "`associated_token::mint` is required when using associated_token constraints",
        )
    })?;
    let authority = authority.ok_or_else(|| {
        syn::Error::new(
            proc_macro2::Span::call_site(),
            "`associated_token::authority` is required when using associated_token constraints",
        )
    })?;
    let token_program = token_program
        .unwrap_or_else(|| Ident::new("token_program", proc_macro2::Span::call_site()));

    Ok(Some(AssociatedTokenInit {
        mint,
        authority,
        token_program,
    }))
}

/// Wrap the `Result<Self>`-yielding `init_body` so that each runtime-only
/// namespaced constraint's `AccountConstraint::init` fires against the
/// freshly-typed value, then return the typed value. Producing this
/// wrapper inline (rather than threading the calls through the outer
/// constraints vec) keeps init hooks scoped to the actual create branch —
/// on `init_if_needed`, an already-existing account skips this block and
/// therefore skips every init hook.
fn wrap_init_body_with_constraints(
    field_ty: &Type,
    attrs: &AccountAttrs,
    init_body: &TokenStream2,
) -> TokenStream2 {
    let init_calls: Vec<TokenStream2> = attrs
        .namespaced
        .iter()
        .filter(|nc| !nc.is_update && is_runtime_only_constraint_ns(&nc.namespace))
        .map(|nc| {
            let ns = syn::Ident::new(&nc.namespace, proc_macro2::Span::call_site());
            let key = syn::Ident::new(&nc.key, proc_macro2::Span::call_site());
            let value = &nc.value;
            let expected = if nc.is_field_ref && (nc.namespace == "mint" || nc.namespace == "token")
            {
                quote! { anchor_lang_v2::AccountAddress::account_address(&#value) }
            } else if nc.is_field_ref {
                quote! { AsRef::as_ref(&#value) }
            } else {
                quote! { &#value }
            };
            quote! {
                <#ns::#key as anchor_lang_v2::AccountConstraint<_>>::init(
                    &mut __init, #expected,
                )?;
            }
        })
        .collect();

    if init_calls.is_empty() {
        return quote! { #init_body };
    }

    // `init_body` is a sequence of `let` statements ending in the
    // `create_and_initialize(...)?` expression — wrap it in a block so
    // the sequence resolves to a value that can be bound to `__init`.
    quote! {
        {
            let mut __init: #field_ty = { #init_body };
            #(#init_calls)*
            __init
        }
    }
}

pub fn is_nested_type(ty: &Type) -> bool {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            return seg.ident == "Nested";
        }
    }
    false
}

/// Pull the first generic arg out of a `Nested<T>` type path, e.g.
/// `Nested<InnerAccounts>` → `InnerAccounts`. Returns `None` for anything
/// else. Used by the `HEADER_SIZE` codegen to walk into nested account
/// structs and sum their compile-time header counts.
pub fn extract_nested_inner_type(ty: &Type) -> Option<&Type> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if seg.ident == "Nested" {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

/// Extracts the inner `T` from `Option<T>` for optional-account field detection.
/// Users write `pub foo: Option<Account<Bar>>` in their Accounts struct; the
/// derive constructs `None` when the client passes the program's own address
/// as the sentinel, otherwise `Some(Bar::load(view)?)`.
pub fn extract_option_inner(ty: &Type) -> Option<&Type> {
    if let Type::Path(tp) = ty {
        if let Some(seg) = tp.path.segments.last() {
            if seg.ident == "Option" {
                if let syn::PathArguments::AngleBracketed(args) = &seg.arguments {
                    if let Some(syn::GenericArgument::Type(inner)) = args.args.first() {
                        return Some(inner);
                    }
                }
            }
        }
    }
    None
}

pub struct AccountField {
    pub name: Ident,
    /// The field's original `syn::Type` — used by `impl_accounts` to build
    /// the `HEADER_SIZE` compile-time sum (1 per direct field, +
    /// `<Inner as TryAccounts>::HEADER_SIZE` per `Nested<Inner>`).
    pub ty: Type,
    pub load: TokenStream2,
    pub deferred_load: Option<TokenStream2>,
    pub constraints: Vec<TokenStream2>,
    /// Duplicate-mutable-account check. Collected separately from
    /// `constraints` so all mut-field dup checks can share a single outer
    /// `if let Some(__dups) = __duplicates` gate — non-dup txs pay one
    /// Option-tag branch regardless of field count.
    pub dup_check: Option<TokenStream2>,
    pub exit: Option<TokenStream2>,
    pub has_bump: bool,
    /// True when the field type is `Option<T>` (optional account).
    pub is_optional: bool,
    /// Offset expression for this field within the enclosing struct's
    /// views slice (a compile-time usize). Retained so the trait-impl
    /// emitter can fold direct-mut fields into `MUT_MASK` at the right
    /// bit position and shift each `Nested<U>` child's `MUT_MASK` by
    /// this offset.
    pub offset_expr: TokenStream2,
    /// `true` iff this field contributes a `1` to the enclosing struct's
    /// `MUT_MASK`: a non-`Option<_>` mut field without `unsafe(dup)`.
    /// `Option<T>` mut fields are excluded because a `None` slot (the
    /// client sends `program_id` as the address) should still silence the
    /// dup check; the derive keeps the gated per-field `get()` for those.
    pub contributes_mut_bit: bool,
    /// `true` iff this optional field contributes to the runtime active
    /// mutable mask when it loads as `Some`.
    pub contributes_active_mut_bit: bool,
    /// The local payer field named by this field's `init`/`init_if_needed`
    /// constraint, if present.
    pub init_payer: Option<String>,
    // IDL metadata
    pub idl_writable: bool,
    /// True when this is a fresh-keypair init site (attrs: `init` or
    /// `init_if_needed` without `seeds`). The caller must sign the tx with
    /// the new account's keypair, so it surfaces as `signer: true` in the
    /// IDL. Orthogonal to the `Signer` field type — those contribute via
    /// `<Ty as IdlAccountType>::__IDL_IS_SIGNER` at runtime.
    pub idl_init_signer: bool,
    /// `has_one = target` targets declared on this field's attrs. Relations
    /// emission walks every field and looks for has_one chains targeting
    /// *another* field, so we need to keep them addressable per-source to
    /// build the inverse mapping (matches v1's `get_relations`).
    pub idl_has_one: Vec<String>,
    /// Stringified RHS of `#[account(address = <expr>)]`. Emitted verbatim
    /// as the `address` key of this field in the accounts JSON. `None` when
    /// the attr is absent *or* when the RHS was v1-encodable (see
    /// `idl_address_v1_source`) — in that case the constraint is surfaced
    /// through `relations` instead, matching v1's IDL shape. Wrapper types
    /// that carry a compile-time address via `IdlAccountType::__IDL_ADDRESS`
    /// still emit the trait value when this override is `None`
    /// (fields like `Program<System>`).
    pub idl_address: Option<String>,
    /// Set when `#[account(address = <sibling>.<self_name>)]` was used,
    /// i.e. the same relationship `#[has_one = <self_name>]` on `<sibling>`
    /// would have expressed. The outer derive turns this into an inverse
    /// `relations` entry on self (mirroring what v1 emits), so clients
    /// that already handle `has_one` transparently pick up the v2 spelling.
    /// `None` for the non-v1-encodable case (RHS is a constant path, a
    /// call, or a sibling-field access whose subfield name differs from
    /// self's field name).
    pub idl_address_v1_source: Option<String>,
    /// Extracted `#[doc = "..."]` lines on the field, in source order.
    /// Emitted as `"docs":[...]` in the accounts JSON. Matches
    /// `IdlInstructionAccount.docs` (`idl/spec/src/lib.rs:83`).
    pub idl_docs: Vec<String>,
    /// Classified seed metadata for PDA emission. `None` when the field
    /// has no `seeds = [...]` attr.
    pub idl_pda: Option<IdlPdaMeta>,
    /// The raw field type, post-`Option<T>` unwrap. Used by the generated
    /// IDL dependency registration to dispatch on the wrapper type
    /// (`Program<T>`, `Account<T>`, …) rather than on its `::Data` associated
    /// type. `None` only for non-`Type::Path` fields that can't appear as
    /// accounts (defensive — this path shouldn't trigger in practice).
    pub idl_field_ty: Option<syn::Type>,
}

/// Turn the RHS of `#[account(address = <expr>)]` into the string form the
/// IDL emits. Whitespace from `quote!`'s token reassembly is stripped so
/// `crate :: ID` → `crate::ID`, `data . authority` → `data.authority`, and
/// `crate :: id ()` → `crate::id()` — matching what a user would hand-write
/// and what downstream tooling (the Anchor CLI resolver, TS client path
/// walkers) expect to parse.
fn stringify_address_expr(expr: &Expr) -> String {
    let s = quote!(#expr).to_string();
    s.split_whitespace().collect()
}

/// If `expr` is the v1-encodable shape `<sibling>.<field>` where both:
///   - `<sibling>` is a sibling field name, and
///   - `<field>` matches `self_name` (the field carrying this constraint),
/// returns `Some(<sibling>)`. Otherwise `None`.
///
/// This is exactly the constraint v1 expressed as
/// `#[has_one = <self_name>]` on `<sibling>`. Matching the shape lets the
/// derive emit the same IDL (`relations: [...]` on self) for both
/// spellings, so tooling that already understands v1 has_one output
/// keeps working unchanged.
fn address_v1_relation_source(
    expr: &Expr,
    self_name: &str,
    field_names: &[String],
) -> Option<String> {
    let fa = if let Expr::Field(fa) = expr {
        fa
    } else {
        return None;
    };
    // Subfield must match self's ident, or v1 has_one couldn't express it.
    let subfield = match &fa.member {
        syn::Member::Named(ident) if ident == self_name => ident,
        _ => return None,
    };
    let _ = subfield; // silence unused — we only needed it for the match guard
                      // Base must be a bare sibling ident (not a method call, not a path).
    let base = if let Expr::Path(ep) = &*fa.base {
        ep
    } else {
        return None;
    };
    if base.qself.is_some() || base.path.leading_colon.is_some() || base.path.segments.len() != 1 {
        return None;
    }
    let seg = &base.path.segments[0];
    if !seg.arguments.is_empty() {
        return None;
    }
    let sibling = seg.ident.to_string();
    field_names.contains(&sibling).then_some(sibling)
}

/// Rewrite a single seed expression so that a bare field-name identifier
/// (like `wallet` in `seeds = [b"vault", wallet]`) is replaced with the
/// explicit address accessor `wallet.address()`.
///
/// Strict: only rewrites simple single-segment `Expr::Path` expressions
/// whose identifier matches a known field name. Everything else
/// (literals, method calls, array refs, complex expressions) passes
/// through unchanged so users can still write explicit seed expressions.
fn rewrite_seed_value_expr(expr: &Expr, field_names: &[String]) -> proc_macro2::TokenStream {
    use quote::quote;
    if let Expr::Path(ep) = expr {
        if ep.qself.is_none() && ep.path.segments.len() == 1 && ep.path.leading_colon.is_none() {
            let seg = &ep.path.segments[0];
            if seg.arguments.is_empty() {
                let ident = &seg.ident;
                if field_names.contains(&ident.to_string()) {
                    return quote! { #ident.address() };
                }
            }
        }
    }
    if let Expr::MethodCall(method_call) = expr {
        if method_call.method == "as_ref"
            && method_call.args.is_empty()
            && method_call.turbofish.is_none()
            && !matches!(method_call.receiver.as_ref(), Expr::Path(_))
        {
            let receiver = &method_call.receiver;
            return quote! { #receiver };
        }
    }
    quote! { #expr }
}

fn materialize_seed_refs(
    seeds: &[&Expr],
    field_names: &[String],
) -> (Vec<TokenStream2>, Vec<TokenStream2>) {
    let mut bindings = Vec::with_capacity(seeds.len());
    let mut refs = Vec::with_capacity(seeds.len());
    for (idx, seed) in seeds.iter().enumerate() {
        let value = rewrite_seed_value_expr(seed, field_names);
        let value_ident = Ident::new(
            &format!("__seed_{}_value", idx),
            proc_macro2::Span::call_site(),
        );
        let ref_ident = Ident::new(
            &format!("__seed_{}_ref", idx),
            proc_macro2::Span::call_site(),
        );
        bindings.push(quote! {
            let #value_ident = #value;
            let #ref_ident: &[u8] = #value_ident.as_ref();
        });
        refs.push(quote! { #ref_ident });
    }
    (bindings, refs)
}

/// Build the seed-check codegen for a `#[account(seeds = [..], bump)]`
/// field. Tries to precompute the canonical PDA bump at macro-expansion
/// time when all seeds are byte literals and the crate's program id can
/// be discovered from `src/lib.rs`, emitting `verify_program_address`
/// in place of the runtime `find_program_address` loop.
///
/// Falls back to the dynamic path whenever:
///   - any seed is non-literal (field reference, method call, expr),
///   - `seeds::program = expr` overrides the derivation program id, or
///   - program-id discovery fails for any reason (missing lib.rs,
///     parse error, no `declare_id!` macro, malformed argument).
///
/// `target_addr_ref` must be a TokenStream producing `&Address` for the
/// account whose address we're verifying: `__target.address()` inside
/// the `init` arm, `<field>.account().address()` for non-init
/// constraints.
///
/// `for_init = true` additionally emits the `let __seeds: Option<&[&[u8]]> = Some(...)`
/// binding in the enclosing scope, as required by the init arm's
/// subsequent `create_and_initialize` call.
///
/// `using_our_program_id = false` (i.e. `seeds::program = ...` is set)
/// unconditionally falls back to the dynamic path, since we only know
/// how to discover our own crate's `declare_id!`.
#[allow(clippy::too_many_arguments)]
fn emit_seeds_check(
    seeds: &[&Expr],
    field_names: &[String],
    pda_program: &TokenStream2,
    target_addr_ref: &TokenStream2,
    field_name: &Ident,
    field_ty: Option<&Type>,
    for_init: bool,
    using_our_program_id: bool,
    is_optional: bool,
) -> TokenStream2 {
    let (seed_bindings, seed_refs) = materialize_seed_refs(seeds, field_names);
    // For optional fields the bumps struct field is `Option<u8>`, so the
    // assignment wraps in `Some(...)`. Non-optional fields assign the bump
    // directly.
    let wrap_bump = |b: TokenStream2| -> TokenStream2 {
        if is_optional {
            quote! { Some(#b) }
        } else {
            b
        }
    };
    // Try to precompute the bump and PDA at expansion time.
    if using_our_program_id {
        if let Some(literal_seeds) = crate::pda::seeds_as_byte_literals(seeds) {
            if let Some(program_id) = crate::pda::discover_program_id() {
                let seed_slices: Vec<&[u8]> = literal_seeds.iter().map(|s| s.as_slice()).collect();
                if let Some((bump, pda_bytes)) =
                    crate::pda::precompute_pda(&seed_slices, &program_id)
                {
                    // Field-scoped const names keep multiple fields'
                    // bumps + PDAs from colliding, even when two
                    // constraints share an outer scope.
                    let upper = field_name.to_string().to_uppercase();
                    let bump_const = Ident::new(&format!("__{}_BUMP", upper), field_name.span());
                    let pda_const = Ident::new(&format!("__{}_PDA", upper), field_name.span());
                    // Emit the 32-byte PDA as an `Address` const.
                    let pda_bytes_tokens = pda_bytes.iter().map(|b| quote! { #b });
                    let bump_assign = wrap_bump(quote! { #bump_const });
                    let check = quote! {
                        const #bump_const: u8 = #bump;
                        const #pda_const: anchor_lang_v2::Address =
                            anchor_lang_v2::Address::new_from_array([#(#pda_bytes_tokens),*]);
                        if !anchor_lang_v2::address_eq(#target_addr_ref, &#pda_const) {
                            return Err(anchor_lang_v2::ErrorCode::ConstraintSeeds.into());
                        }
                        __bumps.#field_name = #bump_assign;
                    };
                    return if for_init {
                        quote! {
                            #check
                            #(#seed_bindings)*
                            let __bump_seed = [#bump_const];
                            let __seeds: Option<&[&[u8]]> =
                                Some(&[#(#seed_refs),* , __bump_seed.as_ref()]);
                        }
                    } else {
                        // Wrap non-init in a block so the consts are
                        // scoped and can't collide with other fields.
                        quote! { { #check } }
                    };
                }
            }
        }
    }

    // Fallback: runtime find loop fused with the equality check.
    //
    // Skip `sol_curve_validate_point` when the account is provably
    // signed-for (`MIN_DATA_LEN > 0`), since account creation already
    // validates the PDA via `create_program_address`.
    //
    // Otherwise (`UncheckedAccount` with zero data, non-init): the curve
    // check is the only proof the address is a real PDA.
    //
    // `MIN_DATA_LEN` is a trait const, so the branch is resolved at
    // compile time — LLVM eliminates the dead path entirely.
    // TODO: decide whether init paths should assume the subsequent
    // CreateAccount CPI guarantees the address is off-curve, letting
    // us skip `sol_curve_validate_point`. Currently we always run the
    // curve check on init to avoid relying on the trait impl's CPI.
    let skip_curve = if let Some(ty) = field_ty {
        quote! { <#ty as anchor_lang_v2::AnchorAccount>::MIN_DATA_LEN > 0 }
    } else {
        quote! { false }
    };
    let bump_assign = wrap_bump(quote! { __bump });
    let find = quote! {
        #(#seed_bindings)*
        let __bump = if #skip_curve {
            anchor_lang_v2::find_and_verify_program_address_skip_curve(
                &[#(#seed_refs),*], #pda_program, #target_addr_ref,
            ).map_err(|_| anchor_lang_v2::ErrorCode::ConstraintSeeds)?
        } else {
            anchor_lang_v2::find_and_verify_program_address(
                &[#(#seed_refs),*], #pda_program, #target_addr_ref,
            ).map_err(|_| anchor_lang_v2::ErrorCode::ConstraintSeeds)?
        };
        __bumps.#field_name = #bump_assign;
    };
    if for_init {
        quote! {
            #find
            let __bump_seed = [__bump];
            let __seeds: Option<&[&[u8]]> =
                Some(&[#(#seed_refs),* , __bump_seed.as_ref()]);
        }
    } else {
        find
    }
}

/// Emit the shared init body used by both `#[account(init)]` and
/// `#[account(init_if_needed)]`: seeds check, param assignments,
/// `create_and_initialize`, and `load_mut_after_init`.
fn emit_init_body(
    field_name: &Ident,
    field_ty: &Type,
    attrs: &AccountAttrs,
    field_names: &[String],
    is_optional: bool,
) -> TokenStream2 {
    let payer = attrs.payer.as_ref().expect("init requires payer");
    // Fall back to `<T as Space>::INIT_SPACE` when `space` is omitted.
    // SPL types (Mint, TokenAccount) impl Space = size_of<Self>() so
    // `#[account(init, token::mint = ..., token::authority = ...)]` works
    // without hardcoding magic numbers like `space = 165`.
    let space = match attrs.space.as_ref() {
        Some(expr) => quote! { #expr },
        None => quote! { <#field_ty as anchor_lang_v2::Space>::INIT_SPACE },
    };
    let owner = match attrs.owner.as_ref() {
        Some(expr) => quote! { #expr },
        None => quote! { *__program_id },
    };
    let owner_check = attrs.owner.as_ref().map(|_| {
        quote! {
            fn __anchor_assert_foreign_owner_init<
                T: anchor_lang_v2::ForeignOwnerInit,
            >() {}
            __anchor_assert_foreign_owner_init::<#field_ty>();
        }
    });

    // Init params come from namespaced constraints that name init-time
    // inputs (e.g. `mint::authority = x`). Runtime-only constraints —
    // currently any constraint whose Params type has no matching field —
    // would fail to typecheck if threaded here. We filter out the ones
    // we know are runtime-only before collecting param assignments.
    let param_assignments: Vec<_> = attrs
        .namespaced
        .iter()
        .filter(|nc| !is_runtime_only_constraint_ns(&nc.namespace))
        .map(|nc| {
            let key = Ident::new(&nc.raw_key, proc_macro2::Span::call_site());
            let value = &nc.value;
            if nc.is_field_ref {
                quote! { __p.#key = Some(#value.account()); }
            } else {
                quote! { __p.#key = Some(#value); }
            }
        })
        .collect();

    let seeds_arg = if let Some(ref seeds_expr) = attrs.seeds {
        let using_our_program_id = attrs.seeds_program.is_none();
        let pda_program = match &attrs.seeds_program {
            Some(prog) => quote! { &#prog },
            None => quote! { __program_id },
        };
        if let Expr::Array(arr) = seeds_expr {
            let seed_elems: Vec<&Expr> = arr.elems.iter().collect();
            emit_seeds_check(
                &seed_elems,
                field_names,
                &pda_program,
                &quote! { __target.address() },
                field_name,
                None,
                true,
                using_our_program_id,
                is_optional,
            )
        } else {
            // Opaque expression seeds — runtime find + verify.
            let bump_assign = if is_optional {
                quote! { Some(__bump) }
            } else {
                quote! { __bump }
            };
            quote! {
                let __seed_expr_val = #seeds_expr;
                let __seed_ref: &[&[u8]] = __seed_expr_val.as_ref();
                let __bump =
                    anchor_lang_v2::find_and_verify_program_address(
                        __seed_ref, #pda_program, &__target.address(),
                    ).map_err(|_| anchor_lang_v2::ErrorCode::ConstraintSeeds)?;
                __bumps.#field_name = #bump_assign;
                let mut __seed_buf: [&[u8]; 17] = [&[]; 17];
                let __n = __seed_ref.len();
                __seed_buf[..__n].copy_from_slice(__seed_ref);
                __seed_buf[__n] = &[__bump];
                let __seeds: Option<&[&[u8]]> = Some(&__seed_buf[..__n + 1]);
            }
        }
    } else {
        quote! { let __seeds: Option<&[&[u8]]> = None; }
    };

    quote! {
        let __payer = #payer.account();
        #seeds_arg
        #owner_check
        let __owner = #owner;
        let __init_params = {
            type __P<'__a> = <#field_ty as anchor_lang_v2::AccountInitialize>::Params<'__a>;
            let mut __p = <__P as Default>::default();
            #(#param_assignments)*
            __p
        };
        <#field_ty as anchor_lang_v2::AccountInitialize>::create_and_initialize(
            __payer, &__target, #space, &__owner, &__init_params, __seeds,
        )?
    }
}

fn emit_associated_token_init_body(
    field_ty: &Type,
    attrs: &AccountAttrs,
    associated_token: &AssociatedTokenInit,
    field_offsets: &[(String, TokenStream2)],
    _is_optional: bool,
) -> syn::Result<TokenStream2> {
    let payer = attrs.payer.as_ref().expect("init requires payer");
    let payer_offset = field_offset_expr(field_offsets, payer)?;
    let mint_offset = field_offset_expr(field_offsets, &associated_token.mint)?;
    let authority_offset = field_offset_expr(field_offsets, &associated_token.authority)?;
    let token_program_offset = field_offset_expr(field_offsets, &associated_token.token_program)?;
    let system_program = Ident::new("system_program", proc_macro2::Span::call_site());
    let associated_token_program =
        Ident::new("associated_token_program", proc_macro2::Span::call_site());
    let system_program_offset = field_offset_expr(field_offsets, &system_program)?;
    let associated_token_program_offset =
        field_offset_expr(field_offsets, &associated_token_program)?;

    Ok(quote! {
        {
            let mut __payer =
                <anchor_lang_v2::accounts::UncheckedAccount as anchor_lang_v2::AnchorAccount>
                    ::load(__views[#payer_offset])?;
            let mut __associated_token =
                <anchor_lang_v2::accounts::UncheckedAccount as anchor_lang_v2::AnchorAccount>
                    ::load(__target)?;
            let __authority =
                <anchor_lang_v2::accounts::UncheckedAccount as anchor_lang_v2::AnchorAccount>
                    ::load(__views[#authority_offset])?;
            let __mint =
                <anchor_lang_v2::accounts::UncheckedAccount as anchor_lang_v2::AnchorAccount>
                    ::load(__views[#mint_offset])?;
            let __system_program =
                <anchor_lang_v2::accounts::UncheckedAccount as anchor_lang_v2::AnchorAccount>
                    ::load(__views[#system_program_offset])?;
            let __token_program =
                <anchor_lang_v2::accounts::UncheckedAccount as anchor_lang_v2::AnchorAccount>
                    ::load(__views[#token_program_offset])?;
            let __associated_token_program =
                <anchor_lang_v2::accounts::UncheckedAccount as anchor_lang_v2::AnchorAccount>
                    ::load(__views[#associated_token_program_offset])?;

            if !anchor_lang_v2::address_eq(
                __system_program.account().address(),
                &<anchor_lang_v2::programs::System as anchor_lang_v2::Id>::id(),
            ) {
                return Err(anchor_lang_v2::ErrorCode::ConstraintAddress.into());
            }
            if !anchor_lang_v2::address_eq(
                __associated_token_program.account().address(),
                &<anchor_lang_v2::programs::AssociatedToken as anchor_lang_v2::Id>::id(),
            ) {
                return Err(anchor_lang_v2::ErrorCode::ConstraintAddress.into());
            }
            anchor_spl_v2::associated_token::create(anchor_lang_v2::CpiContext::new(
                __associated_token_program.account().address(),
                anchor_spl_v2::associated_token::Create {
                    payer: __payer.cpi_handle_mut(),
                    associated_token: __associated_token.cpi_handle_mut(),
                    authority: __authority.cpi_handle(),
                    mint: __mint.cpi_handle(),
                    system_program: __system_program.cpi_handle(),
                    token_program: __token_program.cpi_handle(),
                },
            ))?;

            // SAFETY: this field has just been initialized by the associated
            // token program, and duplicate mutable accounts are rejected by
            // the generated account bitvec check. ATA init is performed by
            // external programs selected at runtime, so run the field type's
            // full validation after the CPI.
            unsafe { <#field_ty as anchor_lang_v2::AnchorAccount>::load_mut(__target)? }
        }
    })
}

pub fn parse_field(
    field: &syn::Field,
    field_names: &[String],
    field_offsets: &[(String, TokenStream2)],
    offset_expr: proc_macro2::TokenStream,
    ix_arg_names: &[String],
) -> syn::Result<AccountField> {
    let field_name = field.ident.as_ref().expect("named field");
    let field_ty = &field.ty;
    let attrs = parse_account_attrs(&field.attrs)?;
    if attrs.close.is_some() && !attrs.is_mut {
        return Err(syn::Error::new(
            field_name.span(),
            "mut must be provided when using close",
        ));
    }
    let associated_token = parse_associated_token_init(&attrs, field_names)?;

    let option_inner = extract_option_inner(field_ty);
    let is_optional = option_inner.is_some();
    // Explicit signer constraint or fresh-keypair init (no seeds) — caller
    // signs the tx. Distinct from `Signer`-type fields, which the IDL picks
    // up through `IdlAccountType::__IDL_IS_SIGNER` at runtime.
    let idl_init_signer = attrs.is_signer
        || ((attrs.is_init || attrs.is_init_if_needed)
            && attrs.seeds.is_none()
            && associated_token.is_none());
    let idl_writable = attrs.is_mut;
    let idl_has_one: Vec<String> = attrs
        .has_one
        .iter()
        .map(|(_, i, _)| i.to_string())
        .collect();
    // Classify the `#[account(address = <expr>)]` RHS for IDL emission:
    //
    //   * `<sibling>.<self_name>` — v1-encodable as `has_one = <self_name>`
    //     on `<sibling>`. Surface as a `relations` entry so tooling that
    //     already speaks v1 output sees the same shape for both spellings.
    //     `idl_address` stays `None` to avoid double-encoding the same
    //     check.
    //   * Anything else — constant path, const-fn call, or a field access
    //     whose subfield doesn't match self's ident. Emit verbatim under
    //     the `address` key; the Anchor CLI resolves constants to base58
    //     pubkeys at IDL-build time, and dotted paths flow through as
    //     client-side resolution hints.
    let (idl_address, idl_address_v1_source) = match attrs.address.as_ref() {
        Some(addr) => {
            match address_v1_relation_source(addr, &field_name.to_string(), field_names) {
                Some(sibling) => (None, Some(sibling)),
                None => (Some(stringify_address_expr(addr)), None),
            }
        }
        None => (None, None),
    };
    let idl_docs = crate::idl::extract_doc_lines(&field.attrs);
    let idl_pda = attrs.seeds.as_ref().map(|seeds_expr| {
        let seed_entries: Vec<crate::idl::SeedJson> = if let Expr::Array(arr) = seeds_expr {
            arr.elems
                .iter()
                .map(|s| crate::idl::classify_seed(s, field_names, ix_arg_names))
                .collect()
        } else {
            // Non-array seed expr — surface as the placeholder `{"kind":"expr"}`
            // shape. Static because it doesn't depend on the user's expr value.
            vec![crate::idl::SeedJson::Static(
                r#"{"kind":"expr"}"#.to_string(),
            )]
        };
        IdlPdaMeta {
            seeds: seed_entries,
            program: attrs
                .seeds_program
                .as_ref()
                .map(|p| crate::idl::classify_program_seed(p, field_names, ix_arg_names)),
        }
    });
    let idl_field_ty: Option<syn::Type> = {
        let base_ty = option_inner.unwrap_or(field_ty);
        if let Type::Path(_) = base_ty {
            Some(base_ty.clone())
        } else {
            None
        }
    };

    let has_bump = attrs.seeds.is_some();
    let init_if_needed_existed = attrs.is_init_if_needed.then(|| {
        Ident::new(
            &format!("__anchor_{}_existed", field_name),
            proc_macro2::Span::call_site(),
        )
    });

    // --- Load ---
    if is_nested_type(field_ty) {
        if field
            .attrs
            .iter()
            .any(|attr| attr.path().is_ident("account"))
        {
            return Err(syn::Error::new(
                field_name.span(),
                "`#[account(...)]` attributes are not supported on `Nested<T>` fields; \
                 put constraints on the fields inside the nested `Accounts` struct",
            ));
        }

        let inner_ty = extract_nested_inner_type(field_ty)
            .expect("is_nested_type was true but extract_nested_inner_type returned None");
        // Nested<Inner> — delegate to Inner::try_accounts, which advances the
        // shared cursor by Inner::HEADER_SIZE. The outer walk_n covers only
        // direct (non-nested) fields; the nested try_accounts picks up where
        // the outer left off.
        //
        // Constraint processing and exit are handled by the inner struct's own
        // try_accounts / exit_accounts — the outer derives don't need to
        // re-check them.
        // TODO: passing `__base_offset + #offset_expr` means the nested
        // struct's bitvec lookups hit the correct global indices. This is
        // correct but adds a runtime addition per dup-check inside the
        // nested struct. A future optimization could pre-shift the bitvec
        // or use a wrapper that offsets transparently.
        let load = quote! {
            let (__nested_inner, _, _) =
                <#inner_ty as anchor_lang_v2::TryAccounts>::try_accounts(
                    __program_id,
                    &__views[#offset_expr .. #offset_expr + <#inner_ty as anchor_lang_v2::TryAccounts>::HEADER_SIZE],
                    __duplicates,
                    __base_offset + #offset_expr,
                    __ix_data,
                )?;
            let #field_name = anchor_lang_v2::Nested(__nested_inner);
        };
        let exit = Some(quote! {
            self.#field_name.0.exit_accounts()?;
        });
        return Ok(AccountField {
            name: field_name.clone(),
            ty: field.ty.clone(),
            load,
            deferred_load: None,
            constraints: vec![],
            dup_check: None,
            exit,
            has_bump: false,
            is_optional: false,
            offset_expr,
            // Nested children contribute via their own `MUT_MASK` shifted
            // into the parent's; they don't set a bit at the nested field's
            // own offset.
            contributes_mut_bit: false,
            contributes_active_mut_bit: false,
            init_payer: None,
            idl_writable: false,
            idl_init_signer: false,
            idl_has_one: vec![],
            idl_address: None,
            idl_address_v1_source: None,
            idl_docs: vec![],
            idl_pda: None,
            idl_field_ty: None,
        });
    }

    let mut deferred_load = None;
    let load = if let Some(inner_ty) = option_inner {
        // `Option<T>` field: client-side sentinel of "account address ==
        // program_id" is interpreted as `None`. Otherwise we run the same
        // load / init / init_if_needed / zeroed logic we would for a
        // non-optional `T`, but against `inner_ty` (so the v2 trait-based
        // `AccountInitialize` / `AnchorAccount` impls dispatch on `T`, not
        // `Option<T>`), and wrap the result in `Some`.
        let inner_action = if attrs.is_init {
            // Init body emitted against inner_ty so the trait call lands on T.
            let init_body = if let Some(ref at) = associated_token {
                emit_associated_token_init_body(inner_ty, &attrs, at, field_offsets, true)?
            } else {
                emit_init_body(field_name, inner_ty, &attrs, field_names, true)
            };
            let init_body_with_constraints =
                wrap_init_body_with_constraints(inner_ty, &attrs, &init_body);
            quote! { Some({ #init_body_with_constraints }) }
        } else if attrs.is_init_if_needed {
            let init_body = if let Some(ref at) = associated_token {
                emit_associated_token_init_body(inner_ty, &attrs, at, field_offsets, true)?
            } else {
                emit_init_body(field_name, inner_ty, &attrs, field_names, true)
            };
            let init_body_with_constraints =
                wrap_init_body_with_constraints(inner_ty, &attrs, &init_body);
            quote! {
                if __target.data_len() > 0
                    && !__target.owned_by(&anchor_lang_v2::programs::System::id())
                {
                    // SAFETY: the bitvec duplicate-account check below ensures
                    // no other mutable reference to this account's data exists.
                    Some(unsafe {
                        <#inner_ty as anchor_lang_v2::AnchorAccount>::load_mut(__target)?
                    })
                } else {
                    Some({ #init_body_with_constraints })
                }
            }
        } else if attrs.is_zeroed {
            quote! {
                {
                    let __disc = <#inner_ty as anchor_lang_v2::Discriminator>::DISCRIMINATOR;
                    {
                        let __data = __target.try_borrow()?;
                        if __data.len() < __disc.len()
                            || __data[..__disc.len()].iter().any(|b| *b != 0)
                        {
                            return Err(anchor_lang_v2::ErrorCode::ConstraintZero.into());
                        }
                    }
                    unsafe {
                        let mut __view = __target;
                        let __data = __view.borrow_unchecked_mut();
                        __data[..__disc.len()].copy_from_slice(__disc);
                    }
                    // SAFETY: the bitvec duplicate-account check below ensures
                    // no other mutable reference to this account's data exists.
                    Some(unsafe {
                        <#inner_ty as anchor_lang_v2::AnchorAccount>::load_mut(__target)?
                    })
                }
            }
        } else if attrs.is_mut {
            quote! {
                // SAFETY: the bitvec duplicate-account check below ensures
                // no other mutable reference to this account's data exists.
                Some(unsafe {
                    <#inner_ty as anchor_lang_v2::AnchorAccount>::load_mut(__target)?
                })
            }
        } else {
            quote! {
                Some(<#inner_ty as anchor_lang_v2::AnchorAccount>::load(__target)?)
            }
        };
        let init_if_needed_existed_binding = init_if_needed_existed.as_ref().map(|existed| {
            quote! {
                let #existed = {
                    let __target = __views[#offset_expr];
                    !anchor_lang_v2::address_eq(__target.address(), __program_id)
                        && __target.data_len() > 0
                        && !__target.owned_by(&anchor_lang_v2::programs::System::id())
                };
            }
        });
        let load = quote! {
            #init_if_needed_existed_binding
            let mut #field_name: #field_ty = {
                let __target = __views[#offset_expr];
                if anchor_lang_v2::address_eq(__target.address(), __program_id) {
                    None
                } else {
                    #inner_action
                }
            };
        };
        if attrs.is_init || attrs.is_init_if_needed {
            deferred_load = Some(load);
            quote! {}
        } else {
            load
        }
    } else if attrs.is_init {
        let init_body = if let Some(ref at) = associated_token {
            emit_associated_token_init_body(field_ty, &attrs, at, field_offsets, false)?
        } else {
            emit_init_body(field_name, field_ty, &attrs, field_names, false)
        };
        let init_body_with_constraints =
            wrap_init_body_with_constraints(field_ty, &attrs, &init_body);
        deferred_load = Some(quote! {
            let mut #field_name: #field_ty = {
                let __target = __views[#offset_expr];
                #init_body_with_constraints
            };
        });
        quote! {}
    } else if attrs.is_init_if_needed {
        let init_body = if let Some(ref at) = associated_token {
            emit_associated_token_init_body(field_ty, &attrs, at, field_offsets, false)?
        } else {
            emit_init_body(field_name, field_ty, &attrs, field_names, false)
        };
        let init_body_with_constraints =
            wrap_init_body_with_constraints(field_ty, &attrs, &init_body);
        let existed = init_if_needed_existed.as_ref().unwrap();
        deferred_load = Some(quote! {
            let #existed = {
                let __target = __views[#offset_expr];
                __target.data_len() > 0
                    && !__target.owned_by(&anchor_lang_v2::programs::System::id())
            };
            let mut #field_name: #field_ty = {
                let __target = __views[#offset_expr];
                if #existed {
                    // SAFETY: the bitvec duplicate-account check below ensures
                    // no other mutable reference to this account's data exists.
                    unsafe { <#field_ty as anchor_lang_v2::AnchorAccount>::load_mut(__target)? }
                } else {
                    // Create branch: run `AccountConstraint::init` for every
                    // runtime-only constraint AFTER the account's typed
                    // creation. Gated to this branch so the init hook only
                    // fires on actual creation, never on the exist branch.
                    #init_body_with_constraints
                }
            };
        });
        quote! {}
    } else if attrs.is_zeroed {
        // zeroed: account exists but discriminator must be all zeros. Verify,
        // stamp the real discriminator, then load mutably.
        quote! {
            let mut #field_name: #field_ty = {
                let __target = __views[#offset_expr];
                let __disc = <#field_ty as anchor_lang_v2::Discriminator>::DISCRIMINATOR;
                {
                    let __data = __target.try_borrow()?;
                    if __data.len() < __disc.len() || __data[..__disc.len()].iter().any(|b| *b != 0) {
                        return Err(anchor_lang_v2::ErrorCode::ConstraintZero.into());
                    }
                }
                unsafe {
                    let mut __view = __target;
                    let __data = __view.borrow_unchecked_mut();
                    __data[..__disc.len()].copy_from_slice(__disc);
                }
                // SAFETY: the bitvec duplicate-account check below ensures
                // no other mutable reference to this account's data exists.
                unsafe { <#field_ty as anchor_lang_v2::AnchorAccount>::load_mut(__target)? }
            };
        }
    } else if attrs.is_mut {
        quote! {
            // SAFETY: the bitvec duplicate-account check below ensures no
            // other mutable reference to this account's data exists.
            let mut #field_name = unsafe { <#field_ty as anchor_lang_v2::AnchorAccount>::load_mut(__views[#offset_expr])? };
        }
    } else {
        quote! {
            let #field_name: #field_ty = anchor_lang_v2::AnchorAccount::load(__views[#offset_expr])?;
        }
    };

    // --- Constraints ---
    let mut constraints = Vec::new();

    // Writable check is now owned by `AnchorAccount::load_mut` (default
    // impl in `lang-v2/src/traits.rs`), so the derive no longer emits a
    // separate constraint block for `#[account(mut)]`. Types that
    // override `load_mut` (Slab/Account, BorshAccount, Signer, Boxed,
    // Option) each validate is_writable themselves; types that inherit
    // the default (UncheckedAccount, SystemAccount, Program, Sysvar) get
    // it via the trait default.

    // signer check
    if attrs.is_signer {
        constraints.push(quote! {
            if !#field_name.account().is_signer() {
                return Err(anchor_lang_v2::ErrorCode::ConstraintSigner.into());
            }
        });
    }

    // executable check
    if attrs.is_executable {
        constraints.push(quote! {
            if !#field_name.account().executable() {
                return Err(anchor_lang_v2::ErrorCode::ConstraintExecutable.into());
            }
        });
    }

    // Seeds constraint. Runs for all non-init fields, INCLUDING
    // init_if_needed: when the account already exists the init body
    // (which contains its own seeds check) is skipped, so this is the
    // only PDA verification on that path. For plain `init`, the seeds
    // check inside emit_init_body is authoritative and this block is
    // skipped to avoid a redundant find loop.
    if !attrs.is_init {
        if let Some(ref seeds_expr) = attrs.seeds {
            let using_our_program_id = attrs.seeds_program.is_none();
            let pda_program = match &attrs.seeds_program {
                Some(prog) => quote! { &#prog },
                None => quote! { __program_id },
            };
            if let Expr::Array(arr) = seeds_expr {
                // Array-literal seeds: `seeds = [b"vault", user.address().as_ref()]`
                let seed_elems: Vec<&Expr> = arr.elems.iter().collect();
                let seed_constraint = if let Some(Some(ref bump_expr)) = attrs.bump {
                    let bump_assign = if is_optional {
                        quote! { Some(__bump_val) }
                    } else {
                        quote! { __bump_val }
                    };
                    let (seed_bindings, seed_refs) =
                        materialize_seed_refs(&seed_elems, field_names);
                    quote! {
                        {
                            #(#seed_bindings)*
                            let __bump_val: u8 = #bump_expr;
                            anchor_lang_v2::verify_program_address(
                                &[#(#seed_refs),* , &[__bump_val]],
                                #pda_program,
                                #field_name.account().address(),
                            )?;
                            __bumps.#field_name = #bump_assign;
                        }
                    }
                } else {
                    let target_addr_ref = quote! { #field_name.account().address() };
                    emit_seeds_check(
                        &seed_elems,
                        field_names,
                        &pda_program,
                        &target_addr_ref,
                        field_name,
                        Some(field_ty),
                        false,
                        using_our_program_id,
                        is_optional,
                    )
                };
                constraints.push(if let Some(existed) = init_if_needed_existed.as_ref() {
                    quote! {
                        if #existed {
                            #seed_constraint
                        }
                    }
                } else {
                    seed_constraint
                });
            } else {
                // Opaque expression: `seeds = Counter::seeds()` etc.
                let bump_assign = if is_optional {
                    quote! { Some(__bump) }
                } else {
                    quote! { __bump }
                };
                let seed_constraint = if let Some(Some(ref bump_expr)) = attrs.bump {
                    // Explicit bump + expression seeds: verify with appended bump
                    quote! {
                        {
                            let __seed_val = #seeds_expr;
                            let __seed_ref: &[&[u8]] = __seed_val.as_ref();
                            if __seed_ref.len() > 16 {
                                return Err(anchor_lang_v2::ErrorCode::ConstraintSeeds.into());
                            }
                            let __bump: u8 = #bump_expr;
                            let __bump_bytes = [__bump];
                            let mut __seed_buf: [&[u8]; 17] = [&[]; 17];
                            let __n = __seed_ref.len();
                            __seed_buf[..__n].copy_from_slice(__seed_ref);
                            __seed_buf[__n] = &__bump_bytes;
                            anchor_lang_v2::verify_program_address(
                                &__seed_buf[..__n + 1],
                                #pda_program,
                                #field_name.account().address(),
                            )?;
                            __bumps.#field_name = #bump_assign;
                        }
                    }
                } else {
                    // Bare bump: use find_and_verify with skip_curve
                    // when the account type guarantees non-zero data.
                    let skip_curve = quote! {
                        <#field_ty as anchor_lang_v2::AnchorAccount>::MIN_DATA_LEN > 0
                    };
                    let target_addr = quote! { #field_name.account().address() };
                    quote! {
                        {
                            let __seed_val = #seeds_expr;
                            let __seed_ref: &[&[u8]] = __seed_val.as_ref();
                            let __bump = if #skip_curve {
                                anchor_lang_v2::find_and_verify_program_address_skip_curve(
                                    __seed_ref, #pda_program, #target_addr,
                                ).map_err(|_| anchor_lang_v2::ErrorCode::ConstraintSeeds)?
                            } else {
                                anchor_lang_v2::find_and_verify_program_address(
                                    __seed_ref, #pda_program, #target_addr,
                                ).map_err(|_| anchor_lang_v2::ErrorCode::ConstraintSeeds)?
                            };
                            __bumps.#field_name = #bump_assign;
                        }
                    }
                };
                constraints.push(if let Some(existed) = init_if_needed_existed.as_ref() {
                    quote! {
                        if #existed {
                            #seed_constraint
                        }
                    }
                } else {
                    seed_constraint
                });
            }
        }
    }

    // has_one
    //
    // This syntax is supported, but deprecated in favor of `address`.
    for (ho_span, ho, ho_err) in &attrs.has_one {
        let err = if let Some(ref e) = ho_err {
            quote! { core::convert::Into::into(#e) }
        } else {
            quote! { anchor_lang_v2::ErrorCode::ConstraintHasOne.into() }
        };
        let deprecation = quote_spanned! { *ho_span =>
            {
                #[deprecated(
                    note = "`has_one` is deprecated; on the sibling field, use \
                            `#[account(address = owner.field)]` instead."
                )]
                fn __deprecated_has_one() {}
                __deprecated_has_one();
            }
        };
        constraints.push(quote! {
            #deprecation
            if AsRef::<[u8]>::as_ref(&#field_name.#ho) != AsRef::<[u8]>::as_ref(#ho.account().address()) {
                return Err(#err);
            }
        });
    }

    // address
    if let Some(ref addr) = attrs.address {
        let err = if let Some(ref e) = attrs.address_error {
            quote! { core::convert::Into::into(#e) }
        } else {
            quote! { anchor_lang_v2::ErrorCode::ConstraintAddress.into() }
        };
        constraints.push(quote! {
            {
                // Accept any `T: Into<Address>` on the RHS — `Address`
                // itself goes through the blanket `From<T> for T`,
                // `&Address`, `[u8; 32]`, and any user-defined wrapper
                // with an `Into<Address>` impl all flow through the
                // same conversion. Still binds to a local first so
                // `address_eq` sees a stable reference.
                let __expected: anchor_lang_v2::Address =
                    core::convert::Into::into(#addr);
                if !anchor_lang_v2::address_eq(#field_name.account().address(), &__expected) {
                    return Err(#err);
                }
            }
        });
    }

    // owner
    if let Some(ref owner_expr) = attrs.owner {
        let err = if let Some(ref e) = attrs.owner_error {
            quote! { core::convert::Into::into(#e) }
        } else {
            quote! { anchor_lang_v2::ErrorCode::ConstraintOwner.into() }
        };
        constraints.push(quote! {
            if !#field_name.account().owned_by(&#owner_expr) {
                return Err(#err);
            }
        });
    }

    // constraint(s) — emitted in the order they appeared in the attribute.
    for (expr, custom_err) in &attrs.raw_constraints {
        let err = if let Some(custom_err) = custom_err {
            quote! { core::convert::Into::into(#custom_err) }
        } else {
            quote! { anchor_lang_v2::ErrorCode::ConstraintRaw.into() }
        };
        constraints.push(quote! {
            if !(#expr) {
                return Err(#err);
            }
        });
    }

    if !attrs.is_init {
        if let Some(ref at) = associated_token {
            let mint = &at.mint;
            let authority = &at.authority;
            let token_program = &at.token_program;
            constraints.push(quote! {
                {
                    let __associated_token_mint =
                        anchor_lang_v2::AccountAddress::account_address(&#mint);
                    let __associated_token_authority =
                        anchor_lang_v2::AccountAddress::account_address(&#authority);
                    let __associated_token_token_program =
                        anchor_lang_v2::AccountAddress::account_address(&#token_program);

                    if !anchor_lang_v2::address_eq(
                        #field_name.mint(),
                        __associated_token_mint,
                    ) {
                        return Err(anchor_lang_v2::ErrorCode::ConstraintAddress.into());
                    }
                    if !anchor_lang_v2::address_eq(
                        #field_name.owner(),
                        __associated_token_authority,
                    ) {
                        return Err(anchor_lang_v2::ErrorCode::ConstraintAddress.into());
                    }
                    if !#field_name.account().owned_by(__associated_token_token_program) {
                        return Err(anchor_lang_v2::ErrorCode::ConstraintOwner.into());
                    }

                    let __expected_associated_token =
                        anchor_spl_v2::associated_token::get_associated_token_address_with_program_id(
                            __associated_token_authority,
                            __associated_token_mint,
                            __associated_token_token_program,
                        );
                    if !anchor_lang_v2::address_eq(
                        #field_name.account().address(),
                        &__expected_associated_token,
                    ) {
                        return Err(anchor_lang_v2::ErrorCode::ConstraintAddress.into());
                    }
                }
            });
        }
    }

    // Namespaced constraints → `AccountConstraint` method dispatch.
    //
    //   | context                                       | method(s)              |
    //   |-----------------------------------------------|------------------------|
    //   | `update(ns::k = v)`                           | `update`               |
    //   | `init, ns::k = v` (runtime-only ns)           | `init` (inside create) |
    //   | `init_if_needed, ns::k = v` (runtime-only ns) | `init` + `check`       |
    //   |     (init runs only on the create branch)     |                        |
    //   | `init_if_needed, ns::k = v` (built-in ns)     | `check` (exist branch) |
    //   | `ns::k = v` (non-init)                        | `check`                |
    //   | `init, ns::k = v` (built-in ns)               | skipped — Params path  |
    //
    // The `init` dispatch is embedded inline into the init body by
    // `wrap_init_body_with_constraints` above so the hook only fires on
    // actual creation. Only `check` and `update` emit out here in the
    // constraint phase.
    //
    // Field refs thread through `AsRef::as_ref` so the call-site's
    // `V` is inferred from the `AccountConstraint::Value` associated
    // type. Literals / expressions pass through verbatim.
    for nc in &attrs.namespaced {
        if nc.namespace == "associated_token" {
            continue;
        }
        // TODO: Improve diagnostics for missing SPL namespace imports.
        // Today `token::...` / `mint::...` resolution failures point at the
        // derive output. We want to keep the normal Rust E0433, but add a
        // useful hint for importing `anchor_spl_v2::prelude::*` or the
        // specific marker module.
        let ns = syn::Ident::new(&nc.namespace, proc_macro2::Span::call_site());
        let key = syn::Ident::new(&nc.key, proc_macro2::Span::call_site());
        let value = &nc.value;
        let expected = if nc.is_field_ref && (nc.namespace == "mint" || nc.namespace == "token") {
            quote! { anchor_lang_v2::AccountAddress::account_address(&#value) }
        } else if nc.is_field_ref {
            quote! { AsRef::as_ref(&#value) }
        } else {
            quote! { &#value }
        };

        if nc.is_update {
            let update_target = if is_optional {
                quote! { #field_name }
            } else {
                quote! { &mut #field_name }
            };
            // `update(...)` — fires regardless of init state.
            constraints.push(quote! {
                <#ns::#key as anchor_lang_v2::AccountConstraint<_>>::update(
                    #update_target, #expected,
                )?;
            });
            continue;
        }

        // `check` fires for:
        //   - non-init fields,
        //   - init_if_needed fields (both runtime-only and built-in,
        //     covering the already-exists branch where the Params path
        //     never ran, and redundantly on the create branch after
        //     init already stamped the state).
        //
        // Pure `init` fields do not emit `check`: runtime-only got
        // `init` via `wrap_init_body_with_constraints`, built-in was
        // handled by `AccountInitialize::Params`, and the values are
        // authoritative by construction.
        if !attrs.is_init {
            let check_target = if is_optional {
                quote! { &*#field_name }
            } else {
                quote! { &#field_name }
            };
            constraints.push(quote! {
                <#ns::#key as anchor_lang_v2::AccountConstraint<_>>::check(
                    #check_target, #expected,
                )?;
            });
        }
    }

    // realloc
    if let Some(ref new_space) = attrs.realloc {
        let realloc_payer = attrs
            .realloc_payer
            .as_ref()
            .expect("realloc requires realloc_payer");
        let zero_fill = attrs.realloc_zero;
        let realloc_target = if is_optional {
            quote! { #field_name }
        } else {
            quote! { &mut #field_name }
        };
        constraints.push(quote! {
            {
                let __new_space = #new_space;
                let __payer_view = *#realloc_payer.account();
                anchor_lang_v2::AccountRealloc::realloc_account(
                    #realloc_target,
                    __new_space,
                    __payer_view,
                    #zero_fill,
                )?;
            }
        });
    }

    // Namespaced constraint exits: emit `AccountConstraint::exit` calls
    // for every namespaced constraint in source order, routed through
    // `self.<field>` so they run in `exit_accounts()` context. Field-ref
    // RHS values are rewritten from bare `sibling` → `self.sibling`;
    // literal / expression values pass through unchanged (callers that
    // need self-qualified expression exits should spell the path in
    // full).
    let constraint_exits: Vec<TokenStream2> = attrs
        .namespaced
        .iter()
        .filter(|nc| nc.namespace != "associated_token")
        .map(|nc| {
            let ns = syn::Ident::new(&nc.namespace, proc_macro2::Span::call_site());
            let key = syn::Ident::new(&nc.key, proc_macro2::Span::call_site());
            let value = &nc.value;
            let expected = if nc.is_field_ref && (nc.namespace == "mint" || nc.namespace == "token")
            {
                quote! { anchor_lang_v2::AccountAddress::account_address(&self.#value) }
            } else if nc.is_field_ref {
                quote! { AsRef::as_ref(&self.#value) }
            } else {
                quote! { &#value }
            };
            quote! {
                <#ns::#key as anchor_lang_v2::AccountConstraint<_>>::exit(
                    &mut self.#field_name, #expected,
                )?;
            }
        })
        .collect();
    let has_constraint_exits = !constraint_exits.is_empty();

    // close (self-close prevention constraint + exit)
    let exit = if let Some(ref close_target) = attrs.close {
        constraints.push(quote! {
            if anchor_lang_v2::address_eq(
                #field_name.account().address(),
                #close_target.account().address(),
            ) {
                return Err(anchor_lang_v2::ErrorCode::ConstraintClose.into());
            }
        });
        Some(quote! {
            #(#constraint_exits)*
            anchor_lang_v2::AnchorAccount::close(
                &mut self.#field_name,
                *self.#close_target.account(),
            )?;
        })
    } else if attrs.is_mut {
        Some(quote! {
            #(#constraint_exits)*
            anchor_lang_v2::AnchorAccount::exit(&mut self.#field_name)?;
        })
    } else if has_constraint_exits {
        // Constraint exits even on read-only fields: callers can attach
        // an exit hook to a non-mut field (e.g. a bookkeeping constraint
        // that only needs to run post-instruction).
        Some(quote! {
            #(#constraint_exits)*
        })
    } else {
        None
    };

    // Dup-check emission: only `Option<_>` mut fields keep a gated
    // per-field `get()` check — a `None` slot (the client encodes
    // `program_id` as the address) must stay silent even when that slot
    // is also the dup target of another account, and the
    // `if let Some(...)` wrapper built below preserves that. Non-`Option`
    // mut fields are folded into the enclosing struct's `MUT_MASK` const
    // and checked once per dispatch by `run_handler`. Stored separately
    // from `constraints` so the struct-level codegen can aggregate all
    // mut fields' dup checks under a single outer
    // `if let Some(__dups) = __duplicates { ... }` gate.
    let dup_check = if attrs.is_mut && !attrs.is_dup && is_optional {
        Some(quote! {
            if __dups.get((__base_offset + #offset_expr) as u8) {
                return Err(anchor_lang_v2::ErrorCode::ConstraintDuplicateMutableAccount.into());
            }
        })
    } else {
        None
    };

    // For `Option<T>` fields, each constraint body was generated against the
    // unwrapped inner — we wrap it in `if let Some(#field_name) = #field_name`
    // so `#field_name.account()`, `#field_name.authority`, etc. resolve on the
    // inner `T` (via autoderef). The exit/close path regenerates against the
    // unwrapped `&mut T` so `AnchorAccount::exit/close` get the right type.
    //
    // Mutable fields use `ref mut` so constraint bodies that need `&mut self`
    // (e.g. BorshAccount::release_borrow in the realloc path) can work.
    // Read-only methods still resolve via auto-deref from `&mut T` to `&T`.
    let (constraints, exit) = if is_optional {
        let constraints = constraints
            .into_iter()
            .map(|c| {
                if attrs.is_mut {
                    quote! {
                        if let Some(ref mut #field_name) = #field_name {
                            let _ = &#field_name;
                            #c
                        }
                    }
                } else {
                    quote! {
                        if let Some(ref #field_name) = #field_name {
                            // `#c` may not textually name `#field_name` (e.g. a
                            // literal `constraint = false`, or the derive-
                            // generated duplicate-mut guard that only touches
                            // `__duplicates[..]`). Without this no-op reference
                            // rustc flags the original field as unused. Narrow
                            // silencer rather than a blanket
                            // `#[allow(unused_variables)]` so real typos in
                            // `#c` still surface.
                            let _ = &#field_name;
                            #c
                        }
                    }
                }
            })
            .collect();
        let exit = exit.map(|e| {
            // `e` was built against `self.#field_name` (e.g.
            // `AnchorAccount::exit(&mut self.#field_name)`). For optional
            // fields we rebuild with the unwrapped inner so the trait call
            // dispatches on `T`, not `Option<T>`.
            let _ = e; // silence unused (shape decided below)

            // Rebuild namespaced-constraint exits against the unwrapped
            // inner `&mut T` bound as `__inner`.
            let inner_constraint_exits: Vec<TokenStream2> = attrs
                .namespaced
                .iter()
                .filter(|nc| nc.namespace != "associated_token")
                .map(|nc| {
                    let ns = syn::Ident::new(&nc.namespace, proc_macro2::Span::call_site());
                    let key = syn::Ident::new(&nc.key, proc_macro2::Span::call_site());
                    let value = &nc.value;
                    let expected =
                        if nc.is_field_ref && (nc.namespace == "mint" || nc.namespace == "token") {
                            quote! { anchor_lang_v2::AccountAddress::account_address(&self.#value) }
                        } else if nc.is_field_ref {
                            quote! { AsRef::as_ref(&self.#value) }
                        } else {
                            quote! { &#value }
                        };
                    quote! {
                        <#ns::#key as anchor_lang_v2::AccountConstraint<_>>::exit(
                            __inner, #expected,
                        )?;
                    }
                })
                .collect();

            if let Some(ref close_target) = attrs.close {
                quote! {
                    if let Some(__inner) = self.#field_name.as_mut() {
                        #(#inner_constraint_exits)*
                        anchor_lang_v2::AnchorAccount::close(
                            __inner,
                            *self.#close_target.account(),
                        )?;
                    }
                }
            } else if attrs.is_mut {
                quote! {
                    if let Some(__inner) = self.#field_name.as_mut() {
                        #(#inner_constraint_exits)*
                        anchor_lang_v2::AnchorAccount::exit(__inner)?;
                    }
                }
            } else {
                quote! {
                    if let Some(__inner) = self.#field_name.as_mut() {
                        #(#inner_constraint_exits)*
                    }
                }
            }
        });
        (constraints, exit)
    } else {
        (constraints, exit)
    };

    let contributes_mut_bit = attrs.is_mut && !attrs.is_dup && !is_optional;
    let contributes_active_mut_bit = attrs.is_mut && !attrs.is_dup && is_optional;
    let init_payer = (attrs.is_init || attrs.is_init_if_needed)
        .then(|| attrs.payer.as_ref().map(ToString::to_string))
        .flatten();

    Ok(AccountField {
        name: field_name.clone(),
        ty: field.ty.clone(),
        load,
        deferred_load,
        constraints,
        dup_check,
        exit,
        has_bump,
        is_optional,
        offset_expr,
        contributes_mut_bit,
        contributes_active_mut_bit,
        init_payer,
        idl_writable,
        idl_init_signer,
        idl_has_one,
        idl_address,
        idl_address_v1_source,
        idl_docs,
        idl_pda,
        idl_field_ty,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_account_attrs() {
        let attrs: Vec<Attribute> = vec![syn::parse_quote!(
            #[account(mut, seeds = [b"vault"], bump, signer)]
        )];
        let parsed_attrs = parse_account_attrs(&attrs).unwrap();
        assert!(parsed_attrs.is_mut);
        assert!(parsed_attrs.seeds.is_some());
        assert!(parsed_attrs.bump.is_some());
        assert!(parsed_attrs.is_signer);
    }

    #[test]
    fn close_does_not_imply_mutability() {
        let attrs: Vec<Attribute> = vec![syn::parse_quote!(
            #[account(close = receiver)]
        )];
        let parsed_attrs = parse_account_attrs(&attrs).unwrap();
        assert!(!parsed_attrs.is_mut);
        assert_eq!(parsed_attrs.close.unwrap().to_string(), "receiver");
    }

    #[test]
    fn account_attrs_on_nested_field_are_rejected() {
        use syn::parse::Parser;

        let field: syn::Field = syn::Field::parse_named
            .parse2(quote::quote! {
                #[account(constraint = missing_symbol_that_should_not_compile())]
                pub inner: Nested<Inner>
            })
            .unwrap();
        let err = match parse_field(&field, &[], &[], quote::quote!(0usize), &[]) {
            Ok(_) => panic!("account attrs on Nested<T> must be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string()
                .contains("`#[account(...)]` attributes are not supported on `Nested<T>` fields"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn opaque_seeds_with_explicit_bump_emits_seed_len_guard() {
        // The opaque-seeds + explicit-bump branch builds a fixed
        // `[&[u8]; 17]` buffer at runtime. Without a length guard, a seed
        // expression returning more than 16 elements panics in
        // `copy_from_slice` or when writing the bump byte. Assert that the
        // generated code rejects oversized seeds with `ConstraintSeeds`
        // before touching the buffer.
        use syn::parse::Parser;
        let field: syn::Field = syn::Field::parse_named
            .parse2(quote::quote! {
                #[account(seeds = MyAcc::seeds(), bump = 0)]
                pub my_acc: Account<MyAcc>
            })
            .unwrap();
        let parsed = parse_field(&field, &[], &[], quote::quote!(0usize), &[]).unwrap();
        let joined = parsed
            .constraints
            .iter()
            .map(|t| t.to_string())
            .collect::<String>();
        assert!(
            joined.contains("__seed_ref . len () > 16"),
            "expected seed-length guard in generated constraints, got: {joined}"
        );
        assert!(
            joined.contains("ConstraintSeeds"),
            "expected ConstraintSeeds error path in generated constraints, got: {joined}"
        );
    }

    #[test]
    fn init_with_explicit_bump_is_rejected() {
        // Mirrors Anchor v1: `init` requires the canonical bump (off-curve
        // guarantee), so caller-supplied bumps must be rejected at parse
        // time rather than silently discarded by the codegen.
        let attrs: Vec<Attribute> = vec![syn::parse_quote!(
            #[account(init, payer = payer, space = 8, seeds = [b"x"], bump = 0)]
        )];
        let err = match parse_account_attrs(&attrs) {
            Ok(_) => panic!("init + bump=<expr> must be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string()
                .contains("`bump = <expr>` is not allowed with `init`"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn init_if_needed_with_explicit_bump_is_accepted() {
        let attrs: Vec<Attribute> = vec![syn::parse_quote!(
            #[account(init_if_needed, payer = payer, space = 8, seeds = [b"x"], bump = 0)]
        )];
        let parsed = parse_account_attrs(&attrs).expect("init_if_needed + bump=<expr>");
        assert!(parsed.is_init_if_needed);
        assert!(matches!(parsed.bump, Some(Some(_))));
    }

    #[test]
    fn multiple_constraints_collected_in_source_order() {
        // Mixed `=` and parenthesized spellings, repeated. Each entry
        // must land in `raw_constraints` at the index it appears, so
        // codegen emits the checks in the same order the user wrote.
        let attrs: Vec<Attribute> = vec![syn::parse_quote!(
            #[account(
                mut,
                constraint = a == b,
                constraint(c == d @ MyErr::X),
                constraint(e.f()),
                constraint = g @ MyErr::Y,
            )]
        )];
        let parsed = parse_account_attrs(&attrs).unwrap();
        assert_eq!(parsed.raw_constraints.len(), 4);
        let strs: Vec<(String, Option<String>)> = parsed
            .raw_constraints
            .iter()
            .map(|(e, err)| {
                (
                    quote!(#e).to_string(),
                    err.as_ref().map(|x| quote!(#x).to_string()),
                )
            })
            .collect();
        assert_eq!(strs[0].0, "a == b");
        assert_eq!(strs[0].1, None);
        assert_eq!(strs[1].0, "c == d");
        assert_eq!(strs[1].1.as_deref(), Some("MyErr :: X"));
        assert_eq!(strs[2].0, "e . f ()");
        assert_eq!(strs[2].1, None);
        assert_eq!(strs[3].0, "g");
        assert_eq!(strs[3].1.as_deref(), Some("MyErr :: Y"));
    }

    #[test]
    fn paren_constraint_rejects_extra_tokens() {
        // `constraint(a, b)` is not a chain — chained checks must be
        // written as separate `constraint(...)` entries. The parser
        // surfaces the misuse at parse time rather than silently
        // dropping the trailing tokens.
        let attrs: Vec<Attribute> = vec![syn::parse_quote!(
            #[account(constraint(a == b, c == d))]
        )];
        let err = match parse_account_attrs(&attrs) {
            Ok(_) => panic!("expected `constraint(a, b)` to be rejected"),
            Err(err) => err,
        };
        assert!(
            err.to_string().contains("single `expr"),
            "unexpected error: {err}"
        );
    }

    #[test]
    fn test_parse_invalid_account_attrs() {
        let attrs: Vec<Attribute> = vec![syn::parse_quote!(
            #[account(mut, seeds = [b"vault"], bumpp, signer)]
        )];

        let err = match parse_account_attrs(&attrs) {
            Ok(_) => panic!("expected malformed account attrs to be rejected"),
            Err(err) => err,
        };

        assert!(
            err.to_string()
                .contains("unknown account constraint `bumpp`"),
            "unexpected error: {err}"
        );
    }
}
