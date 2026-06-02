use {
    crate::{Error, ErrorArgs, ErrorCode},
    syn::{
        parse::{Parse, Result as ParseResult},
        spanned::Spanned,
        Expr,
    },
};

// Removes any internal #[msg] attributes, as they are inert.
pub fn parse(error_enum: &mut syn::ItemEnum, args: Option<ErrorArgs>) -> Result<Error, syn::Error> {
    let ident = error_enum.ident.clone();
    let mut last_discriminant = 0;
    let codes: Vec<ErrorCode> = error_enum
        .variants
        .iter_mut()
        .map(|variant: &mut syn::Variant| {
            let msg = parse_error_attribute(variant)?;
            let ident = variant.ident.clone();
            let id = match &variant.discriminant {
                None => last_discriminant,
                Some((_, disc)) => match disc {
                    syn::Expr::Lit(expr_lit) => match &expr_lit.lit {
                        syn::Lit::Int(int) => int.base10_parse::<u32>().map_err(|_| {
                            syn::Error::new(int.span(), "error discriminant must be a valid u32")
                        })?,
                        _ => {
                            return Err(syn::Error::new(
                                expr_lit.lit.span(),
                                "error discriminant must be an integer literal",
                            ))
                        }
                    },
                    _ => {
                        return Err(syn::Error::new(
                            disc.span(),
                            "error discriminant must be an integer literal",
                        ))
                    }
                },
            };
            last_discriminant = id + 1;

            // Remove any non-doc attributes on the error variant.
            variant.attrs.retain(|attr| attr.path().is_ident("doc"));

            Ok(ErrorCode { id, ident, msg })
        })
        .collect::<Result<Vec<_>, syn::Error>>()?;
    Ok(Error {
        name: error_enum.ident.to_string(),
        raw_enum: error_enum.clone(),
        ident,
        codes,
        args,
    })
}

fn parse_error_attribute(variant: &syn::Variant) -> Result<Option<String>, syn::Error> {
    let attrs = variant
        .attrs
        .iter()
        .filter(|attr| !attr.path().is_ident("doc"))
        .collect::<Vec<_>>();
    match attrs.len() {
        0 => Ok(None),
        1 => {
            #[allow(
                clippy::indexing_slicing,
                reason = "inside match arm where attrs.len() == 1"
            )]
            let attr = &attrs[0];
            if !attr.path().is_ident("msg") {
                return Err(syn::Error::new(
                    attr.span(),
                    "use `#[msg(\"...\")]` to specify error strings",
                ));
            }

            let g_stream = match &attr.meta {
                syn::Meta::List(list) => list.tokens.clone(),
                _ => {
                    return Err(syn::Error::new(
                        attr.span(),
                        "expected `#[msg(\"message\")]`",
                    ))
                }
            };

            let msg = match g_stream.into_iter().next() {
                None => {
                    return Err(syn::Error::new(
                        attr.span(),
                        "`#[msg]` requires a message string",
                    ))
                }
                Some(msg) => msg.to_string().replace('\"', ""),
            };

            Ok(Some(msg))
        }
        _ => Err(syn::Error::new(
            variant.span(),
            "too many attributes; use `#[msg(\"...\")]` to specify error strings",
        )),
    }
}

pub struct ErrorInput {
    pub error_code: Expr,
}

impl Parse for ErrorInput {
    fn parse(stream: syn::parse::ParseStream) -> ParseResult<Self> {
        let error_code = stream.call(Expr::parse)?;
        Ok(Self { error_code })
    }
}
