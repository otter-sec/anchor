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

fn ctx_accounts_ident(path_ty: &syn::PatType) -> ParseResult<proc_macro2::Ident> {
    let p = match &*path_ty.ty {
        syn::Type::Path(p) => &p.path,
        _ => return Err(ParseError::new(path_ty.ty.span(), "invalid type")),
    };
    let segment = p
        .segments
        .first()
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
    Ok(path
        .segments
        .first()
        .ok_or_else(|| ParseError::new(path.span(), "expected a path segment"))?
        .ident
        .clone())
}
