use {
    crate::{parser::docs, Program},
    syn::{
        parse::{Error as ParseError, Result as ParseResult},
        spanned::Spanned,
    },
};

mod instructions;

pub fn parse(program_mod: syn::ItemMod) -> ParseResult<Program> {
    let docs = docs::parse(&program_mod.attrs);
    let (ixs, fallback_fn) = instructions::parse(&program_mod)?;
    Ok(Program {
        ixs,
        name: program_mod.ident.clone(),
        docs,
        program_mod,
        fallback_fn,
        program_args: None,
    })
}

/// Whether a function in a program is an ix handler, a fallback fn or unrecognized
enum FunctionType {
    /// Regular instruction handler - takes a `Context<Account>` and other arguments
    IxHandler,
    /// Fallback method - takes `(&Pubkey, &[AccountInfo], &[u8])`
    Fallback,
    /// Invalid method type, raises an error
    Error(ParseError),
}

/// Identify a function type via the parameters
fn function_type(method: &syn::ItemFn) -> FunctionType {
    let inputs = method
        .sig
        .inputs
        .iter()
        .map(|arg| {
            let syn::FnArg::Typed(arg) = arg else {
                return Err(ParseError::new(
                    arg.span(),
                    "handlers may not take receivers",
                ));
            };
            Ok(arg)
        })
        .collect::<ParseResult<Vec<_>>>();

    let inputs = match inputs {
        Ok(i) => i,
        Err(e) => {
            return FunctionType::Error(e);
        }
    };

    fn named_args(args: &[&syn::PatType]) -> bool {
        args.iter()
            .all(|arg| matches!(&*arg.pat, syn::Pat::Ident(_)))
    }

    fn valid_handler(context: &syn::Type) -> bool {
        let syn::Type::Path(context) = context else {
            return false;
        };
        let Some(segment) = context.path.segments.last() else {
            return false;
        };
        matches!(segment,
            syn::PathSegment {
                ident,
                arguments: syn::PathArguments::AngleBracketed(_),
            } if ident == "Context"
        )
    }

    match inputs.as_slice() {
        [context, ..] if valid_handler(&context.ty) => FunctionType::IxHandler,
        [_, _, _] if named_args(&inputs) => FunctionType::Fallback,
        _ => FunctionType::Error(ParseError::new(
            method.span(),
            "handlers must take a `Context<...>` argument",
        )),
    }
}

impl crate::Ix {
    /// Path to the struct deriving `Accounts`, as written in `Context<...>`,
    /// normalized to be relative to the crate root.
    ///
    /// Recomputed from `raw_method` instead of being stored as a field so that
    /// `Ix` remains constructible with the same fields as before.
    pub(crate) fn anchor_path(&self) -> syn::Path {
        #[allow(
            clippy::expect_used,
            reason = "the instruction was validated when the program was parsed"
        )]
        let (ctx, _) = instructions::parse_args(&self.raw_method)
            .expect("`Ix::raw_method` has a `Context<...>` argument");
        #[allow(
            clippy::expect_used,
            reason = "the instruction was validated when the program was parsed"
        )]
        ctx_accounts_path(&ctx.raw_arg)
            .expect("`Ix::raw_method` has a valid `Context<...>` accounts path")
    }
}

fn ctx_accounts_path(path_ty: &syn::PatType) -> ParseResult<syn::Path> {
    let p = match &*path_ty.ty {
        syn::Type::Path(p) => &p.path,
        _ => return Err(ParseError::new(path_ty.ty.span(), "invalid type")),
    };
    let segment = p
        .segments
        .last()
        .ok_or_else(|| ParseError::new(p.segments.span(), "expected generic arguments here"))?;

    let generic_args = match &segment.arguments {
        syn::PathArguments::AngleBracketed(args) => args,
        _ => return Err(ParseError::new(path_ty.span(), "missing accounts context")),
    };
    let generic_ty = generic_args
        .args
        .iter()
        .filter_map(|arg| match arg {
            syn::GenericArgument::Type(ty) => Some(ty),
            _ => None,
        })
        .next()
        .ok_or_else(|| ParseError::new(generic_args.span(), "expected Accounts type"))?;

    let path = match generic_ty {
        syn::Type::Path(ty_path) => &ty_path.path,
        _ => {
            return Err(ParseError::new(
                generic_ty.span(),
                "expected Accounts struct type",
            ));
        }
    };
    if path.leading_colon.is_some() {
        return Err(ParseError::new(
            path.span(),
            "paths with a leading `::` are not supported in `Context<...>`; use a path relative \
             to the crate root",
        ));
    }

    // Strip generic arguments (e.g. `Foo<'info>`) from all segments so the
    // path can be interpolated in non-generic positions (e.g.
    // `#path::try_accounts`), and strip a leading `crate` segment so the path
    // is always relative to the crate root.
    let mut segments = path
        .segments
        .iter()
        .map(|segment| syn::PathSegment::from(segment.ident.clone()))
        .collect::<Vec<_>>();
    if segments
        .first()
        .is_some_and(|segment| segment.ident == "crate")
    {
        segments.remove(0);
    }
    if segments.iter().any(|segment| {
        segment.ident == "self" || segment.ident == "super" || segment.ident == "crate"
    }) {
        return Err(ParseError::new(
            path.span(),
            "`self` and `super` are not supported in `Context<...>` accounts paths; use a path \
             relative to the crate root",
        ));
    }
    if segments.is_empty() {
        return Err(ParseError::new(path.span(), "expected a path segment"));
    }

    Ok(syn::Path {
        leading_colon: None,
        segments: segments.into_iter().collect(),
    })
}

#[cfg(test)]
mod tests {
    #[test]
    fn anchor_path_is_normalized_relative_to_the_crate_root() {
        let program = syn::parse_str::<crate::Program>(
            r#"
            pub mod example {
                pub fn init(ctx: Context<instructions::init::Init>) -> Result<()> {
                    Ok(())
                }

                pub fn update(ctx: Context<crate::instructions::Update<'info>>) -> Result<()> {
                    Ok(())
                }
            }
            "#,
        )
        .unwrap();

        let segments = |path: &syn::Path| {
            path.segments
                .iter()
                .map(|segment| segment.ident.to_string())
                .collect::<Vec<_>>()
        };

        let mut ixs = program.ixs.iter();

        let init = ixs.next().unwrap();
        assert_eq!(init.anchor_ident, "Init");
        assert_eq!(
            segments(&init.anchor_path()),
            ["instructions", "init", "Init"]
        );

        // A leading `crate` segment and generic arguments are stripped.
        let update = ixs.next().unwrap();
        assert_eq!(update.anchor_ident, "Update");
        let update_path = update.anchor_path();
        assert_eq!(segments(&update_path), ["instructions", "Update"]);
        assert!(update_path
            .segments
            .iter()
            .all(|segment| segment.arguments.is_none()));
    }
}
