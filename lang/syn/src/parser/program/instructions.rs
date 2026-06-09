use {
    crate::{
        parser::{
            docs,
            program::{ctx_accounts_ident, function_type, FunctionType},
        },
        FallbackFn, Ix, IxArg, IxReturn, Overrides,
    },
    syn::{
        parse::{Error as ParseError, Result as ParseResult},
        spanned::Spanned,
        Attribute,
    },
};

// Parse all non-state ix handlers from the program mod definition.
pub fn parse(program_mod: &syn::ItemMod) -> ParseResult<(Vec<Ix>, Option<FallbackFn>)> {
    let mod_content = &program_mod
        .content
        .as_ref()
        .ok_or_else(|| ParseError::new(program_mod.span(), "program content not provided"))?
        .1;

    let mut handlers = Vec::new();
    let mut fallback = Vec::new();
    let mut errors = Vec::new();
    for func in mod_content.iter().filter_map(|item| match item {
        syn::Item::Fn(method) => Some(method),
        _ => None,
    }) {
        match function_type(func) {
            FunctionType::IxHandler => handlers.push(func),
            FunctionType::Fallback => fallback.push(func),
            FunctionType::Error(error) => errors.push(error),
        }
    }

    let ixs = handlers
        .into_iter()
        .map(|method| {
            let (ctx, args) = parse_args(method)?;
            let anchor_ident = ctx_accounts_ident(&ctx.raw_arg)?;
            let overrides = parse_overrides(&method.attrs)?;
            let docs = docs::parse(&method.attrs);
            let cfgs = parse_cfg(method);
            let returns = parse_return(method)?;
            Ok(Some(Ix {
                raw_method: method.clone(),
                ident: method.sig.ident.clone(),
                docs,
                cfgs,
                args,
                anchor_ident,
                returns,
                overrides,
            }))
        })
        .filter_map(|ix| ix.transpose())
        .collect::<ParseResult<Vec<Ix>>>()?;

    let fallback_fn = match *fallback.as_slice() {
        [] => None,
        [f] => Some(FallbackFn {
            raw_method: f.clone(),
        }),
        [_, f2, ..] => {
            return Err(ParseError::new(
                f2.span(),
                "More than one fallback function found",
            ));
        }
    };

    if let Some(e) = errors.pop() {
        return Err(e);
    }

    Ok((ixs, fallback_fn))
}

/// Parse overrides from the `#[instruction]` attribute proc-macro.
fn parse_overrides(attrs: &[syn::Attribute]) -> ParseResult<Option<Overrides>> {
    attrs
        .iter()
        .find(|attr| match attr.path().segments.last() {
            Some(seg) => seg.ident == "instruction",
            _ => false,
        })
        .map(|attr| attr.parse_args())
        .transpose()
}

pub fn parse_args(method: &syn::ItemFn) -> ParseResult<(IxArg, Vec<IxArg>)> {
    let mut args: Vec<IxArg> = method
        .sig
        .inputs
        .iter()
        .map(|arg: &syn::FnArg| match arg {
            syn::FnArg::Typed(arg) => {
                let docs = docs::parse(&arg.attrs);
                let ident = match &*arg.pat {
                    syn::Pat::Ident(ident) => &ident.ident,
                    _ => return Err(ParseError::new(arg.pat.span(), "expected named argument")),
                };
                Ok(IxArg {
                    name: ident.clone(),
                    docs,
                    raw_arg: arg.clone(),
                })
            }
            syn::FnArg::Receiver(_) => Err(ParseError::new(
                arg.span(),
                "expected a typed argument, not self",
            )),
        })
        .collect::<ParseResult<_>>()?;

    // Remove the Context argument
    let ctx = args.remove(0);

    Ok((ctx, args))
}

pub fn parse_return(method: &syn::ItemFn) -> ParseResult<IxReturn> {
    match method.sig.output {
        syn::ReturnType::Type(_, ref ty) => {
            let ty = match ty.as_ref() {
                syn::Type::Path(ty) => ty,
                _ => return Err(ParseError::new(ty.span(), "expected a return type")),
            };
            // Assume unit return by default
            #[allow(
                clippy::unwrap_used,
                reason = "\"()\" is always valid syn::Type syntax"
            )]
            let default_generic_arg = syn::GenericArgument::Type(syn::parse_str("()").unwrap());
            #[allow(
                clippy::unwrap_used,
                reason = "type path always has segments; angle-bracketed args always have at \
                          least one arg"
            )]
            let generic_args = match &ty.path.segments.last().unwrap().arguments {
                syn::PathArguments::AngleBracketed(params) => {
                    params.args.iter().next_back().unwrap()
                }
                _ => &default_generic_arg,
            };
            let ty = match generic_args {
                syn::GenericArgument::Type(ty) => ty.clone(),
                _ => {
                    return Err(ParseError::new(
                        ty.span(),
                        "expected generic return type to be a type",
                    ));
                }
            };
            Ok(IxReturn { ty })
        }
        _ => Err(ParseError::new(
            method.sig.output.span(),
            "expected a return type",
        )),
    }
}

fn parse_cfg(method: &syn::ItemFn) -> Vec<Attribute> {
    method
        .attrs
        .iter()
        .filter_map(|attr| match attr.path().is_ident("cfg") {
            true => Some(attr.to_owned()),
            false => None,
        })
        .collect()
}
