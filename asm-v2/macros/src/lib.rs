//! Proc macros for `anchor-asm-v2`.
//!
//! The main entry point is `asm_program!` which takes type definitions
//! and assembly file paths in a single invocation, generating both the
//! Rust items and a `global_asm!` block with all const operands.
//!
//! ```ignore
//! anchor_asm_v2_macros::asm_program! {
//!     #[error_enum(prefix = "E")]
//!     pub enum ErrorCode {
//!         InvalidDiscriminant,
//!         InvalidInstructionLength,
//!     }
//!
//!     #[offsets(prefix = "CTR")]
//!     #[repr(C)]
//!     pub struct Counter {
//!         pub value: u64,
//!         pub bump: u8,
//!         pub _pad: [u8; 7],
//!     }
//!
//!     asm {
//!         include_str!("asm/errors.s"),
//!         include_str!("asm/entrypoint.s"),
//!     }
//! }
//! ```

use proc_macro::TokenStream;
use proc_macro2::TokenStream as TokenStream2;
use quote::{format_ident, quote};
use syn::{
    braced,
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Expr, Fields, Ident, Item, Lit, Meta,
    Token,
};

/// Default first custom error code. Mirrors `anchor_lang::error_code`.
const DEFAULT_ERROR_CODE_OFFSET: u32 = 6000;

// ---------------------------------------------------------------------------
// asm_program! — the single entry point
// ---------------------------------------------------------------------------

struct AsmProgram {
    items: Vec<AnnotatedItem>,
    /// Raw token trees from the `asm { ... }` block. Passed through
    /// verbatim to `global_asm!` so expressions like
    /// `concat!(env!("OUT_DIR"), "/combined.s")` work.
    asm_tokens: Vec<proc_macro2::TokenTree>,
}

enum AnnotatedItem {
    ErrorEnum {
        prefix: String,
        item: syn::ItemEnum,
    },
    Discriminant {
        prefix: String,
        item: syn::ItemEnum,
    },
    Offsets {
        prefix: String,
        item: syn::ItemStruct,
    },
    /// Stack frame: fields at negative offsets from r10 (frame pointer).
    /// Offset = -(size_of::<Struct>() - offset_of!(Struct, field)).
    Frame {
        prefix: String,
        item: syn::ItemStruct,
    },
    /// Pass-through: items without a recognized #[...] annotation
    /// are emitted as-is (useful for helper structs, impls, etc.)
    Passthrough(Item),
}

impl Parse for AsmProgram {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut items = Vec::new();
        let mut asm_tokens = Vec::new();
        let mut saw_asm = false;

        while !input.is_empty() {
            // Check for `asm { ... }` block
            if input.peek(Ident) {
                let lookahead = input.fork();
                let ident: Ident = lookahead.parse()?;
                if ident == "asm" {
                    if saw_asm {
                        return Err(syn::Error::new(
                            ident.span(),
                            "multiple `asm { ... }` blocks are not allowed",
                        ));
                    }
                    let _: Ident = input.parse()?;
                    let content;
                    braced!(content in input);
                    // Collect all tokens verbatim
                    asm_tokens = content.parse::<proc_macro2::TokenStream>()?.into_iter().collect();
                    saw_asm = true;
                    continue;
                }
            }

            let item: Item = input.parse()?;
            match classify_item(item)? {
                classified => items.push(classified),
            }
        }

        Ok(AsmProgram { items, asm_tokens })
    }
}

/// Look at the attributes on an item and classify it.
fn classify_item(item: Item) -> syn::Result<AnnotatedItem> {
    match item {
        Item::Enum(mut e) => {
            if let Some((kind, prefix)) = extract_asm_attr(&mut e.attrs) {
                match kind.as_str() {
                    "error_enum" => Ok(AnnotatedItem::ErrorEnum { prefix, item: e }),
                    "discriminant" => Ok(AnnotatedItem::Discriminant { prefix, item: e }),
                    other => Err(syn::Error::new_spanned(
                        &e.ident,
                        format!("unknown asm attribute: {other}"),
                    )),
                }
            } else {
                Ok(AnnotatedItem::Passthrough(Item::Enum(e)))
            }
        }
        Item::Struct(mut s) => {
            if let Some((kind, prefix)) = extract_asm_attr(&mut s.attrs) {
                match kind.as_str() {
                    "offsets" => Ok(AnnotatedItem::Offsets { prefix, item: s }),
                    "frame" => Ok(AnnotatedItem::Frame { prefix, item: s }),
                    other => Err(syn::Error::new_spanned(
                        &s.ident,
                        format!("unknown asm attribute: {other}"),
                    )),
                }
            } else {
                Ok(AnnotatedItem::Passthrough(Item::Struct(s)))
            }
        }
        other => Ok(AnnotatedItem::Passthrough(other)),
    }
}

/// Extract and remove `#[error_enum(...)]`, `#[discriminant(...)]`, or
/// `#[offsets(...)]` from an attribute list. Returns (kind, prefix).
fn extract_asm_attr(attrs: &mut Vec<syn::Attribute>) -> Option<(String, String)> {
    let known = ["error_enum", "discriminant", "offsets", "frame"];
    let pos = attrs.iter().position(|a| {
        a.path()
            .get_ident()
            .map(|id| known.contains(&id.to_string().as_str()))
            .unwrap_or(false)
    })?;
    let attr = attrs.remove(pos);
    let kind = attr.path().get_ident()?.to_string();
    let prefix = parse_prefix_from_meta(&attr).unwrap_or_else(|| default_prefix(&kind));
    Some((kind, prefix))
}

fn parse_prefix_from_meta(attr: &syn::Attribute) -> Option<String> {
    let meta = attr.meta.clone();
    if let Meta::List(list) = meta {
        let inner: Meta = syn::parse2(list.tokens).ok()?;
        if let Meta::NameValue(nv) = inner {
            if nv.path.is_ident("prefix") {
                if let Expr::Lit(lit) = &nv.value {
                    if let Lit::Str(s) = &lit.lit {
                        return Some(s.value());
                    }
                }
            }
        }
    }
    None
}

fn default_prefix(kind: &str) -> String {
    match kind {
        "error_enum" => "E".to_string(),
        "discriminant" => "DISC".to_string(),
        _ => String::new(),
    }
}

/// The main proc macro.
#[proc_macro]
pub fn asm_program(input: TokenStream) -> TokenStream {
    let program = syn::parse_macro_input!(input as AsmProgram);
    match expand_asm_program(program) {
        Ok(expanded) => expanded.into(),
        Err(err) => err.to_compile_error().into(),
    }
}

fn expand_asm_program(program: AsmProgram) -> syn::Result<TokenStream2> {
    let mut rust_items = Vec::new();
    let mut const_operands: Vec<TokenStream2> = Vec::new();

    for item in &program.items {
        match item {
            AnnotatedItem::ErrorEnum { prefix, item } => {
                rust_items.push(quote! { #item });
                let enum_name = &item.ident;
                for v in &item.variants {
                    let name = format_ident!(
                        "{}_{}",
                        prefix,
                        to_screaming_snake(&v.ident.to_string())
                    );
                    let variant = &v.ident;
                    const_operands.push(quote! {
                        #name = const (#enum_name::#variant as u32 + #DEFAULT_ERROR_CODE_OFFSET),
                    });
                }
            }
            AnnotatedItem::Discriminant { prefix, item } => {
                rust_items.push(quote! { #item });
                let enum_name = &item.ident;
                for v in &item.variants {
                    let name = format_ident!(
                        "{}_{}",
                        prefix,
                        to_screaming_snake(&v.ident.to_string())
                    );
                    let variant = &v.ident;
                    const_operands.push(quote! {
                        #name = const #enum_name::#variant as u32,
                    });
                }
            }
            AnnotatedItem::Offsets { prefix, item } => {
                ensure_repr_c(item, "offsets")?;
                rust_items.push(quote! { #item });
                let struct_name = &item.ident;
                if let Fields::Named(fields) = &item.fields {
                    for field in &fields.named {
                        let field_name = field.ident.as_ref().unwrap();
                        if field_name.to_string().starts_with('_') {
                            continue;
                        }
                        let const_name = format_ident!(
                            "{}_{}",
                            prefix,
                            to_screaming_snake(&field_name.to_string())
                        );
                        const_operands.push(quote! {
                            #const_name = const core::mem::offset_of!(#struct_name, #field_name) as i32,
                        });
                    }
                    let size_name = format_ident!("{}_SIZE", prefix);
                    const_operands.push(quote! {
                        #size_name = const core::mem::size_of::<#struct_name>() as i32,
                    });
                }
            }
            AnnotatedItem::Frame { prefix, item } => {
                ensure_repr_c(item, "frame")?;
                rust_items.push(quote! { #item });
                let struct_name = &item.ident;
                if let Fields::Named(fields) = &item.fields {
                    // Frame offsets are negative: -(size - offset_of(field))
                    // so the first field is at the most negative offset and
                    // the last field is closest to r10.
                    for field in &fields.named {
                        let field_name = field.ident.as_ref().unwrap();
                        if field_name.to_string().starts_with('_') {
                            continue;
                        }
                        let const_name = format_ident!(
                            "{}_{}",
                            prefix,
                            to_screaming_snake(&field_name.to_string())
                        );
                        const_operands.push(quote! {
                            #const_name = const -(core::mem::size_of::<#struct_name>() as i32
                                - core::mem::offset_of!(#struct_name, #field_name) as i32),
                        });
                    }
                    let size_name = format_ident!("{}_SIZE", prefix);
                    const_operands.push(quote! {
                        #size_name = const core::mem::size_of::<#struct_name>() as i32,
                    });
                }
            }
            AnnotatedItem::Passthrough(item) => {
                rust_items.push(quote! { #item });
            }
        }
    }

    let asm_tokens: proc_macro2::TokenStream = program.asm_tokens.into_iter().collect();

    // Build an asm comment that references every const operand name
    // so LLVM doesn't error on "unused named argument". The comment
    // is zero-cost — it's stripped during assembly.
    let const_names: Vec<String> = const_operands
        .iter()
        .filter_map(|ts| {
            let s = ts.to_string();
            s.split('=').next().map(|n| format!("{{{}}}", n.trim()))
        })
        .collect();
    let sink_comment = if const_names.is_empty() {
        String::new()
    } else {
        format!("/* {} */", const_names.join(" "))
    };

    let expanded = quote! {
        #(#rust_items)*

        core::arch::global_asm!(
            #sink_comment,
            #asm_tokens
            #(#const_operands)*
        );
    };

    Ok(expanded)
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn to_screaming_snake(s: &str) -> String {
    let mut result = String::with_capacity(s.len() + 4);
    for (i, c) in s.chars().enumerate() {
        if c.is_uppercase() && i > 0 {
            let prev = s.chars().nth(i - 1).unwrap_or('_');
            let next = s.chars().nth(i + 1);
            if prev.is_lowercase()
                || prev.is_ascii_digit()
                || (prev.is_uppercase() && next.map_or(false, |n| n.is_lowercase()))
            {
                result.push('_');
            }
        }
        result.push(c.to_ascii_uppercase());
    }
    result
}

fn ensure_repr_c(item: &syn::ItemStruct, attr_name: &str) -> syn::Result<()> {
    let has_repr_c = item.attrs.iter().any(|attr| {
        attr.path().is_ident("repr")
            && attr
                .parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)
                .map(|metas| {
                    metas.into_iter().any(
                        |meta| matches!(meta, Meta::Path(path) if path.is_ident("C")),
                    )
                })
                .unwrap_or(false)
    });
    if has_repr_c {
        Ok(())
    } else {
        Err(syn::Error::new_spanned(
            &item.ident,
            format!("#[{attr_name}] requires #[repr(C)]"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_screaming_snake() {
        assert_eq!(to_screaming_snake("InvalidDiscriminant"), "INVALID_DISCRIMINANT");
        assert_eq!(to_screaming_snake("RegisterMarket"), "REGISTER_MARKET");
        assert_eq!(to_screaming_snake("BaseVaultHasData"), "BASE_VAULT_HAS_DATA");
        assert_eq!(to_screaming_snake("UserHasData"), "USER_HAS_DATA");
    }

    #[test]
    fn test_screaming_snake_splits_digit_to_uppercase_boundaries() {
        assert_eq!(to_screaming_snake("Ipv4Addr"), "IPV4_ADDR");
    }

    fn test_offsets_require_repr_c() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[offsets(prefix = "CTR")]
            pub struct Counter {
                pub value: u64,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let err = expand_asm_program(program).unwrap_err();
        assert!(err.to_string().contains("#[offsets] requires #[repr(C)]"));
    }

    #[test]
    fn test_offsets_accept_repr_c() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[offsets(prefix = "CTR")]
            #[repr(C)]
            pub struct Counter {
                pub value: u64,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let expanded = expand_asm_program(program).unwrap().to_string();
        assert!(expanded.contains("CTR_VALUE"));
        assert!(expanded.contains("CTR_SIZE"));
    }

    #[test]
    fn test_offsets_accept_combined_repr_c_and_align() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[offsets(prefix = "CTR")]
            #[repr(C, align(8))]
            pub struct Counter {
                pub value: u64,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let expanded = expand_asm_program(program).unwrap().to_string();
        assert!(expanded.contains("CTR_VALUE"));
        assert!(expanded.contains("CTR_SIZE"));
    }

    #[test]
    fn test_offsets_accept_split_repr_c_and_align() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[offsets(prefix = "CTR")]
            #[repr(C)]
            #[repr(align(8))]
            pub struct Counter {
                pub value: u64,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let expanded = expand_asm_program(program).unwrap().to_string();
        assert!(expanded.contains("CTR_VALUE"));
        assert!(expanded.contains("CTR_SIZE"));
    }

    #[test]
    fn test_offsets_reject_repr_transparent() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[offsets(prefix = "CTR")]
            #[repr(transparent)]
            pub struct Counter {
                pub value: u64,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let err = expand_asm_program(program).unwrap_err();
        assert!(err.to_string().contains("#[offsets] requires #[repr(C)]"));
    }

    #[test]
    fn test_frame_requires_repr_c() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[frame(prefix = "FM")]
            pub struct Frame {
                pub value: u64,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let err = expand_asm_program(program).unwrap_err();
        assert!(err.to_string().contains("#[frame] requires #[repr(C)]"));
    }

    #[test]
    fn test_frame_accepts_repr_c() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[frame(prefix = "FM")]
            #[repr(C)]
            pub struct Frame {
                pub value: u64,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let expanded = expand_asm_program(program).unwrap().to_string();
        assert!(expanded.contains("FM_VALUE"));
        assert!(expanded.contains("FM_SIZE"));
    }

    #[test]
    fn test_frame_accepts_combined_repr_c_and_packed() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[frame(prefix = "FM")]
            #[repr(C, packed)]
            pub struct Frame {
                pub value: u64,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let expanded = expand_asm_program(program).unwrap().to_string();
        assert!(expanded.contains("FM_VALUE"));
        assert!(expanded.contains("FM_SIZE"));
    }

    #[test]
    fn test_frame_accepts_split_repr_c_and_packed() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[frame(prefix = "FM")]
            #[repr(C)]
            #[repr(packed)]
            pub struct Frame {
                pub value: u64,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let expanded = expand_asm_program(program).unwrap().to_string();
        assert!(expanded.contains("FM_VALUE"));
        assert!(expanded.contains("FM_SIZE"));
    }

    #[test]
    fn test_frame_rejects_repr_packed_without_c() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[frame(prefix = "FM")]
            #[repr(packed)]
            pub struct Frame {
                pub value: u64,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let err = expand_asm_program(program).unwrap_err();
        assert!(err.to_string().contains("#[frame] requires #[repr(C)]"));
    }


    #[test]
    fn test_enum_constants_follow_rust_discriminants() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[error_enum(prefix = "E")]
            pub enum Errors {
                Alpha = 7,
                Beta,
                Gamma = 11,
            }

            #[discriminant(prefix = "DISC")]
            pub enum Disc {
                Start = 3,
                Next,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let expanded = expand_asm_program(program).unwrap().to_string();
        assert!(expanded.contains("E_ALPHA = const (Errors :: Alpha as u32 + 6000u32)"));
        assert!(expanded.contains("E_BETA = const (Errors :: Beta as u32 + 6000u32)"));
        assert!(expanded.contains("E_GAMMA = const (Errors :: Gamma as u32 + 6000u32)"));
        assert!(expanded.contains("DISC_START = const Disc :: Start as u32"));
        assert!(expanded.contains("DISC_NEXT = const Disc :: Next as u32"));
    }

    #[test]
    fn test_error_enum_auto_increment_tracks_explicit_gaps() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[error_enum(prefix = "ERR")]
            pub enum Errors {
                Alpha = 41,
                Beta,
                Gamma = 90,
                Delta,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let expanded = expand_asm_program(program).unwrap().to_string();
        assert!(expanded.contains("ERR_ALPHA = const (Errors :: Alpha as u32 + 6000u32)"));
        assert!(expanded.contains("ERR_BETA = const (Errors :: Beta as u32 + 6000u32)"));
        assert!(expanded.contains("ERR_GAMMA = const (Errors :: Gamma as u32 + 6000u32)"));
        assert!(expanded.contains("ERR_DELTA = const (Errors :: Delta as u32 + 6000u32)"));
    }

    #[test]
    fn test_error_enum_preserves_u32_discriminants() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[error_enum(prefix = "ERR")]
            #[repr(u32)]
            pub enum Errors {
                Small = 7,
                Large = 0x8000_0000,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let expanded = expand_asm_program(program).unwrap().to_string();
        assert!(expanded.contains("ERR_SMALL = const (Errors :: Small as u32 + 6000u32)"));
        assert!(expanded.contains("ERR_LARGE = const (Errors :: Large as u32 + 6000u32)"));
        assert!(!expanded.contains("Errors :: Large as i32"));
    }

    #[test]
    fn test_error_enum_default_first_variant_is_not_success() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[error_enum(prefix = "ERR")]
            pub enum Errors {
                InvalidSignature,
                Unauthorized,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let expanded = expand_asm_program(program).unwrap().to_string();
        assert!(expanded.contains("ERR_INVALID_SIGNATURE = const (Errors :: InvalidSignature as u32 + 6000u32)"));
        assert!(expanded.contains("ERR_UNAUTHORIZED = const (Errors :: Unauthorized as u32 + 6000u32)"));
        assert!(!expanded.contains("ERR_INVALID_SIGNATURE = const Errors :: InvalidSignature as u32"));
    }

    #[test]
    fn test_discriminant_auto_increment_tracks_explicit_gaps() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            #[discriminant(prefix = "DISC")]
            pub enum Instruction {
                Init = 4,
                Update,
                Close = 12,
                Sweep,
            }

            asm { "" }
            "#,
        )
        .unwrap();

        let expanded = expand_asm_program(program).unwrap().to_string();
        assert!(expanded.contains("DISC_INIT = const Instruction :: Init as u32"));
        assert!(expanded.contains("DISC_UPDATE = const Instruction :: Update as u32"));
        assert!(expanded.contains("DISC_CLOSE = const Instruction :: Close as u32"));
        assert!(expanded.contains("DISC_SWEEP = const Instruction :: Sweep as u32"));
    }

    #[test]
    fn test_multiple_asm_blocks_are_rejected() {
        let err = syn::parse_str::<AsmProgram>(
            r#"
            asm { "" }
            asm { "" }
            "#,
        )
        .err()
        .expect("multiple asm blocks should be rejected");

        assert!(err
            .to_string()
            .contains("multiple `asm { ... }` blocks are not allowed"));
    }

    #[test]
    fn test_single_asm_block_is_accepted() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            pub struct Helper;

            asm { include_str!("asm/errors.s"), }
            "#,
        )
        .unwrap();

        let asm_tokens = proc_macro2::TokenStream::from_iter(program.asm_tokens);
        assert_eq!(program.items.len(), 1);
        assert_eq!(asm_tokens.to_string(), "include_str ! (\"asm/errors.s\") ,");
    }

    #[test]
    fn test_asm_block_accepts_include_str_tokens() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            asm { include_str!("asm/errors.s"), }
            "#,
        )
        .unwrap();

        let asm_tokens = proc_macro2::TokenStream::from_iter(program.asm_tokens);
        assert_eq!(asm_tokens.to_string(), "include_str ! (\"asm/errors.s\") ,");
    }

    #[test]
    fn test_asm_block_accepts_include_str_concat_tokens() {
        let program = syn::parse_str::<AsmProgram>(
            r#"
            asm { include_str!(concat!(env!("OUT_DIR"), "/combined.s")), }
            "#,
        )
        .unwrap();

        let asm_tokens = proc_macro2::TokenStream::from_iter(program.asm_tokens);
        assert_eq!(
            asm_tokens.to_string(),
            "include_str ! (concat ! (env ! (\"OUT_DIR\") , \"/combined.s\")) ,"
        );
    }
}
